//! Codex, through `codex app-server`: JSON-RPC over stdio, one thread per
//! conversation and one turn per message. Every shape here was measured on
//! codex-cli 0.160, and the recorded transcripts under `tests/fixtures/codex`
//! are what the tests hold it to.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use devpit_rpc::{CallState, Context, Part, SessionInit};
use serde_json::Value;

use crate::driver::{Driver, Read};
use crate::wire::Wire;

#[path = "codex_wire.rs"]
mod codex_wire;

/// The Codex CLI's app-server, read and written through one shared thread.
#[derive(Default)]
pub struct Codex {
    thread: Arc<Mutex<Thread>>,
}

/// A request devpit sent and has not heard back on.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sent {
    Initialize,
    Open { resuming: bool },
    Turn,
    Other,
}

/// Tokens as the app-server counts them: `input` includes `cached`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Tokens {
    input: u64,
    cached: u64,
    cache_write: u64,
    output: u64,
}

impl Tokens {
    fn of(said: &Value) -> Self {
        let count = |key: &str| said[key].as_u64().unwrap_or_default();
        Self {
            input: count("inputTokens"),
            cached: count("cachedInputTokens"),
            cache_write: count("cacheWriteInputTokens"),
            output: count("outputTokens"),
        }
    }

    fn less(self, other: Self) -> Self {
        Self {
            input: self.input.saturating_sub(other.input),
            cached: self.cached.saturating_sub(other.cached),
            cache_write: self.cache_write.saturating_sub(other.cache_write),
            output: self.output.saturating_sub(other.output),
        }
    }
}

/// The thread's usage as the turn went: where it stood before the turn's
/// first request, and where it stands now.
#[derive(Debug, Default)]
struct Usage {
    from: Option<Tokens>,
    total: Tokens,
    context: Option<Context>,
}

/// One process's thread, shared by its reader and everyone who writes to it.
#[derive(Default)]
struct Thread {
    cwd: String,
    resume: Option<String>,
    model: Option<String>,
    mode: Option<String>,
    effort: Option<String>,
    next_id: u64,
    sent: HashMap<u64, Sent>,
    initialized: bool,
    opening: bool,
    id: Option<String>,
    /// Messages waiting for the thread to open.
    held: Vec<String>,
    /// A model or mode switched since the thread opened, sent with the next turn.
    switched_model: bool,
    switched_mode: bool,
    turn: Option<String>,
    /// Approvals put to the person: devpit's key, and the request's id.
    waiting: HashMap<String, Value>,
    /// Changes the agent proposed, by item, for the approval that follows.
    edits: HashMap<String, Value>,
    ran_on: Option<String>,
    started_at: HashMap<String, u64>,
    streamed: HashSet<String>,
    last_was_text: bool,
    usage: Usage,
    /// What this process's priced turns have spent: a resident turn's cost is
    /// what it adds to this.
    spent: f64,
    failed: Option<String>,
    notice: Option<String>,
}

fn held(thread: &Mutex<Thread>) -> MutexGuard<'_, Thread> {
    thread
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn text(text: impl Into<String>) -> Part {
    Part::Text {
        text: text.into(),
        parent: None,
    }
}

/// The command exactly as it will run. `commandActions` is a summary for
/// display that drops pipeline stages, so it never stands in for it.
fn command_of(item: &Value) -> String {
    item["command"].as_str().unwrap_or_default().to_owned()
}

/// A tool call's name and input, as devpit's acts and questions read them.
fn call_of(item: &Value) -> Option<(String, Value)> {
    match item["type"].as_str()? {
        "commandExecution" => Some((
            "Bash".to_owned(),
            serde_json::json!({ "command": command_of(item), "cwd": item["cwd"] }),
        )),
        "fileChange" => Some((
            "Edit".to_owned(),
            serde_json::json!({
                "file_path": item["changes"][0]["path"],
                "changes": item["changes"],
            }),
        )),
        "mcpToolCall" => Some((
            format!(
                "mcp__{}__{}",
                item["server"].as_str()?,
                item["tool"].as_str()?
            ),
            item["arguments"].clone(),
        )),
        _ => None,
    }
}

/// What a finished tool call said, and whether it failed.
fn result_of(item: &Value) -> (String, bool) {
    let status = item["status"].as_str().unwrap_or_default();
    let failed = status != "completed";
    let output = match item["type"].as_str().unwrap_or_default() {
        "commandExecution" => {
            let said = item["aggregatedOutput"].as_str().unwrap_or_default();
            let code = item["exitCode"].as_i64();
            let failed = failed || code.is_some_and(|code| code != 0);
            return (
                if said.is_empty() { status } else { said }.to_owned(),
                failed,
            );
        }
        "fileChange" => item["changes"]
            .as_array()
            .map(|changes| {
                changes
                    .iter()
                    .map(|change| {
                        let kind = change["kind"]["type"].as_str().unwrap_or("update");
                        let path = change["path"].as_str().unwrap_or_default();
                        let diff = change["diff"].as_str().unwrap_or_default();
                        format!("{kind} {path}\n{diff}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        _ => match item["error"]["message"].as_str() {
            Some(why) => why.to_owned(),
            None => item["result"]["content"]
                .as_array()
                .map(|content| {
                    content
                        .iter()
                        .map(|one| {
                            one["text"]
                                .as_str()
                                .map_or_else(|| one.to_string(), str::to_owned)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default(),
        },
    };
    (
        if output.is_empty() {
            status.to_owned()
        } else {
            output
        },
        failed,
    )
}

/// What a turn's tokens cost at the model's price, or `None` when the price
/// table does not know the model.
fn cost_of(model: &str, tokens: Tokens) -> Option<f64> {
    let uncached = tokens
        .input
        .saturating_sub(tokens.cached)
        .saturating_sub(tokens.cache_write);
    crate::spend_prices::cost_of(&crate::spend_scan::Record {
        key: String::new(),
        at: 0,
        session_id: String::new(),
        cwd: String::new(),
        model: model.to_owned(),
        input: uncached,
        output: tokens.output,
        cache_read: tokens.cached,
        cache_write_5m: tokens.cache_write,
        cache_write_1h: 0,
    })
}

impl Thread {
    fn heard(&mut self, method: &str, params: &Value) -> Read {
        let item = &params["item"];
        let id = item["id"].as_str().unwrap_or_default().to_owned();
        match method {
            "turn/started" => {
                self.turn = params["turn"]["id"].as_str().map(str::to_owned);
                self.usage = Usage::default();
                self.last_was_text = false;
                Read::Init(SessionInit {
                    model: self.ran_on.clone(),
                    ..SessionInit::default()
                })
            }
            "item/agentMessage/delta" => {
                let delta = params["delta"].as_str().unwrap_or_default();
                let first = self
                    .streamed
                    .insert(params["itemId"].as_str().unwrap_or_default().to_owned());
                let gap = if first && self.last_was_text {
                    "\n\n"
                } else {
                    ""
                };
                self.last_was_text = true;
                Read::Parts(vec![text(format!("{gap}{delta}"))])
            }
            "item/started" => self.started(id, item, params),
            "item/completed" => self.completed(id, item, params),
            "thread/tokenUsage/updated" => {
                let usage = &params["tokenUsage"];
                let (total, last) = (Tokens::of(&usage["total"]), Tokens::of(&usage["last"]));
                self.usage.from.get_or_insert(total.less(last));
                self.usage.total = total;
                self.usage.context = usage["modelContextWindow"].as_u64().map(|window| Context {
                    used: usage["last"]["totalTokens"].as_u64().unwrap_or_default() as u32,
                    window: window as u32,
                });
                Read::Nothing
            }
            "error" if !params["willRetry"].as_bool().unwrap_or(false) => {
                let why = params["error"]["message"]
                    .as_str()
                    .unwrap_or("the turn failed");
                Read::Parts(vec![text(format!("\n\n{why}"))])
            }
            "serverRequest/resolved" => {
                let request = &params["requestId"];
                self.waiting.retain(|_, waiting| waiting != request);
                Read::Nothing
            }
            "turn/completed" => self.ended(&params["turn"]),
            _ => Read::Nothing,
        }
    }

    fn started(&mut self, id: String, item: &Value, params: &Value) -> Read {
        if let Some(at) = params["startedAtMs"].as_u64() {
            self.started_at.insert(id.clone(), at);
        }
        match item["type"].as_str().unwrap_or_default() {
            "reasoning" => {
                self.last_was_text = false;
                Read::Parts(vec![Part::Thinking {
                    text: String::new(),
                    parent: None,
                    duration_ms: None,
                }])
            }
            "fileChange" => {
                self.edits.insert(id.clone(), item.clone());
                self.call(id, item)
            }
            _ => self.call(id, item),
        }
    }

    fn call(&mut self, id: String, item: &Value) -> Read {
        let Some((name, input)) = call_of(item) else {
            return Read::Nothing;
        };
        self.last_was_text = false;
        Read::Parts(vec![Part::ToolCall {
            id,
            name,
            input: input.to_string(),
            state: CallState::Running,
            parent: None,
        }])
    }

    fn completed(&mut self, id: String, item: &Value, params: &Value) -> Read {
        let began = self.started_at.remove(&id);
        match item["type"].as_str().unwrap_or_default() {
            "agentMessage" if !self.streamed.contains(&id) => {
                let gap = if self.last_was_text { "\n\n" } else { "" };
                self.last_was_text = true;
                let said = item["text"].as_str().unwrap_or_default();
                Read::Parts(vec![text(format!("{gap}{said}"))])
            }
            "reasoning" => {
                let lines = |key: &str| -> Vec<String> {
                    item[key]
                        .as_array()
                        .map(|all| {
                            all.iter()
                                .filter_map(|one| one.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let mut said = lines("summary");
                if said.is_empty() {
                    said = lines("content");
                }
                let took = began
                    .zip(params["completedAtMs"].as_u64())
                    .map(|(from, to)| to.saturating_sub(from) as u32);
                Read::Parts(vec![Part::Thinking {
                    text: said.join("\n\n"),
                    parent: None,
                    duration_ms: Some(took.unwrap_or_default()),
                }])
            }
            "commandExecution" | "fileChange" | "mcpToolCall" => {
                self.edits.remove(&id);
                let (output, is_error) = result_of(item);
                self.last_was_text = false;
                Read::Parts(vec![Part::ToolResult {
                    call_id: id,
                    output,
                    is_error,
                    parent: None,
                }])
            }
            _ => Read::Nothing,
        }
    }

    /// The turn is over: what it cost on top of what the process spent, and
    /// how full the context was.
    fn ended(&mut self, turn: &Value) -> Read {
        self.turn = None;
        self.streamed.clear();
        // What a stop left open is not coming back.
        self.started_at.clear();
        self.edits.clear();
        let status = turn["status"].as_str().unwrap_or("completed");
        let tokens = self
            .usage
            .from
            .map(|from| self.usage.total.less(from))
            .unwrap_or_default();
        let priced = self
            .ran_on
            .as_deref()
            .and_then(|model| cost_of(model, tokens));
        let cost_usd = priced.map(|cost| {
            self.spent += cost;
            self.spent
        });
        Read::Ended {
            stop_reason: Some(status.to_owned()),
            cost_usd,
            is_error: status == "failed",
            context: self.usage.context,
        }
    }
}

impl Driver for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn wire(&self) -> Arc<dyn Wire> {
        Arc::new(codex_wire::CodexWire::new(self.thread.clone()))
    }

    /// The account's own model by default: which ones a plan offers is the
    /// app-server's to say, and a profile names the ones it wants.
    fn models(&self) -> &'static [&'static str] {
        &["default"]
    }

    /// The efforts every model `model/list` offered on 0.160.
    fn efforts(&self) -> &'static [&'static str] {
        &["low", "medium", "high", "xhigh", "max"]
    }

    fn asks(&self) -> bool {
        true
    }

    fn read(&self, line: &str) -> Read {
        let line = line.trim();
        if line.is_empty() {
            return Read::Nothing;
        }
        let Ok(said) = serde_json::from_str::<Value>(line) else {
            return Read::Parts(vec![Part::Unknown {
                text: line.to_owned(),
            }]);
        };
        let mut thread = held(&self.thread);
        if let Some(why) = thread.failed.take() {
            return Read::Failed(why);
        }
        if let Some(notice) = thread.notice.take() {
            return Read::Parts(vec![text(notice)]);
        }
        match said["method"].as_str() {
            Some(method) => thread.heard(method, &said["params"]),
            // The answer that opened the thread names the model it runs on.
            None => match said["result"]["model"].as_str() {
                Some(model) if said["result"].get("thread").is_some() => {
                    thread.ran_on = Some(model.to_owned());
                    Read::Init(SessionInit {
                        model: Some(model.to_owned()),
                        ..SessionInit::default()
                    })
                }
                _ => Read::Nothing,
            },
        }
    }

    fn session(&self, line: &str) -> Option<String> {
        if !line.contains("\"thread\"") {
            return None;
        }
        let said: Value = serde_json::from_str(line).ok()?;
        let thread = if said["method"] == "thread/started" {
            &said["params"]["thread"]
        } else {
            &said["result"]["thread"]
        };
        thread["id"].as_str().map(str::to_owned)
    }

    /// A message steered in comes back as the turn's user message, under the
    /// id devpit gave it.
    fn echoed(&self, line: &str) -> Option<String> {
        if !line.contains("\"userMessage\"") {
            return None;
        }
        let said: Value = serde_json::from_str(line).ok()?;
        let item = &said["params"]["item"];
        if said["method"] != "item/started" || item["type"] != "userMessage" {
            return None;
        }
        item["clientId"].as_str().map(str::to_owned)
    }
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
