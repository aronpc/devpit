//! The Agent Client Protocol: JSON-RPC over stdio, which OpenCode, Grok and
//! Antigravity serve.
//!
//! One driver for all of them. devpit greets the process (`initialize`), signs
//! in where the agent asks for it before any session (Cursor), opens
//! a session (`session/new`, or `session/resume` / `session/load` to go on with
//! one) and sends a `session/prompt` per turn. The answer streams as
//! `session/update`s and the turn ends with the prompt's response. Written
//! against the protocol's JSON rather than its Rust SDK: the SDK is async and
//! reshapes between releases, and every CLI here is read a line at a time.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use devpit_rpc::{CallState, Context, OfferedModel, Part, SessionInit};
use serde_json::{json, Value};

use crate::driver::{Driver, Read};
use crate::wire::Wire;

#[path = "acp_wire.rs"]
mod acp_wire;

/// The driver for an agent that serves ACP, started with `args`.
pub struct Acp {
    args: &'static [&'static str],
    session: Arc<Mutex<Session>>,
}

impl Acp {
    pub fn new(args: &'static [&'static str]) -> Self {
        Self {
            args,
            session: Arc::default(),
        }
    }

    /// Asks the agent for `method` after it is greeted: a sign-in that reuses
    /// the CLI's own login, for an agent that opens no session before it.
    pub fn signing_in(self, method: Option<&'static str>) -> Self {
        self.held().sign_in = method;
        self
    }

    fn held(&self) -> MutexGuard<'_, Session> {
        held(&self.session)
    }
}

pub(crate) fn held(session: &Mutex<Session>) -> MutexGuard<'_, Session> {
    session
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What devpit asked the agent, by the id it asked under.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Sent {
    Initialize,
    SignIn,
    Open(Opening),
    /// A model set while the session opens; the person's message waits for it.
    Model,
    /// A session mode set while the session opens, as the chat's mode asks.
    Mode,
    /// A model switched on request, answered under the switch's own id.
    Switch(String),
    Prompt,
}

/// How a session was opened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Opening {
    New,
    Resume,
    Load,
}

/// The session modes an agent offers, and how it takes one: through the
/// config option it named, or `session/set_mode` where it reports `modes`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Modes {
    pub option: Option<String>,
    pub offered: Vec<String>,
    pub current: Option<String>,
}

/// The mode devpit's Plan asks of an agent: its own read-only one.
pub(crate) const PLAN: &str = "plan";

/// How an agent takes a model: through the config option it named, or by
/// the model's id where it reports `models` instead.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Picks {
    Option(String),
    ById,
}

/// A permission request waiting on devpit.
pub(crate) struct Waiting {
    pub id: Value,
    pub options: Vec<Value>,
}

/// One process's conversation, shared by its reader and its writers.
#[derive(Default)]
pub(crate) struct Session {
    next: u64,
    pub sent: HashMap<u64, Sent>,
    pub id: Option<String>,
    /// The response that opened the session: the line that names it.
    pub opened_by: Option<u64>,
    /// Messages from the person written before the session was ready.
    pub held: Vec<String>,
    pub ready: bool,
    /// A handshake request is out: the next message waits for it.
    pub opening: bool,
    /// What the agent offered at `initialize`, once it answered.
    pub offered: Option<Value>,
    /// Stopped before the session opened: its message is dropped, and the
    /// turn ends once the session answers.
    pub cancelled: bool,
    pub stop_now: bool,
    pub cwd: String,
    pub resume: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    /// devpit's MCP servers, as the session request takes them.
    pub servers: Option<Value>,
    /// The session modes the agent offered, once it opened one.
    pub modes: Option<Modes>,
    /// The agent's own mode the session opened in, to go back to from Plan.
    pub mode_before: Option<String>,
    /// A mode asked of the agent and not answered yet.
    pub mode_asked: Option<String>,
    /// The sign-in asked for once the agent is greeted.
    pub sign_in: Option<&'static str>,
    /// History a loaded session replays: not this turn's answer.
    pub replaying: bool,
    /// Said with the turn once the session opens.
    pub notice: Option<String>,
    /// Said before the turn's answer: why the model asked for is not the one
    /// it runs on.
    pub said_first: Option<String>,
    /// Why the session could not open; ends the turn that waits on it.
    pub failed: Option<String>,
    pub waiting: HashMap<String, Waiting>,
    pub kinds: HashMap<String, String>,
    /// Each call's input as the agent sent it, which a permission request
    /// may leave out.
    pub inputs: HashMap<String, Value>,
    shown: HashSet<String>,
    /// How the agent takes a model, once it offered one.
    pub picks: Option<Picks>,
    pub current_model: Option<String>,
    /// The models the session offers, and those it refused with why.
    offers: Vec<OfferedModel>,
    refused: Vec<OfferedModel>,
    pub commands: Vec<String>,
    pub context: Option<Context>,
    /// The session's cost when this process began, and now. Unknown at the
    /// start of a resumed session, and then no turn of this process is priced.
    pub cost_from: Option<f64>,
    pub cost: Option<f64>,
    pub prompted: bool,
    thought: String,
    thinking_since: Option<Instant>,
}

impl Session {
    /// A request under the next id, remembered as `sent`.
    pub fn request(&mut self, method: &str, params: Value, sent: Sent) -> String {
        let id = self.next;
        self.next += 1;
        self.sent.insert(id, sent);
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
    }

    /// The person's message as a prompt, once the session can take it.
    pub fn prompt(&mut self, text: &str) -> String {
        self.prompted = true;
        let params = json!({
            "sessionId": self.id,
            "prompt": [{ "type": "text", "text": text }],
        });
        self.request("session/prompt", params, Sent::Prompt)
    }

    /// Takes the model the agent runs on, the ones it offers and how it
    /// switches one, from a session's `configOptions`, or the `models` an
    /// agent reports instead.
    pub fn options(&mut self, result: &Value) {
        let options = result.get("configOptions").and_then(Value::as_array);
        if let Some(model) =
            options.and_then(|all| all.iter().find(|one| one["category"] == "model"))
        {
            self.picks = model["id"].as_str().map(|id| Picks::Option(id.to_owned()));
            self.current_model = model["currentValue"].as_str().map(str::to_owned);
            self.offers = offers_in(&model["options"], "value");
        } else if let Some(current) = result["models"]["currentModelId"].as_str() {
            self.picks = Some(Picks::ById);
            self.current_model = Some(current.to_owned());
            self.offers = offers_in(&result["models"]["availableModels"], "modelId");
        }
        let values = |all: &Value, key: &str| -> Vec<String> {
            all.as_array()
                .map(|all| {
                    all.iter()
                        .filter_map(|one| one[key].as_str())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let modes = match options.and_then(|all| all.iter().find(|one| one["category"] == "mode")) {
            Some(mode) => Some(Modes {
                option: mode["id"].as_str().map(str::to_owned),
                offered: values(&mode["options"], "value"),
                current: mode["currentValue"].as_str().map(str::to_owned),
            }),
            None => result["modes"].as_object().map(|modes| Modes {
                option: None,
                offered: values(&modes["availableModes"], "id"),
                current: modes["currentModeId"].as_str().map(str::to_owned),
            }),
        };
        if let Some(modes) = modes {
            if self.mode_before.is_none() {
                self.mode_before = modes.current.clone().filter(|one| one != PLAN);
            }
            self.modes = Some(modes);
        }
    }

    /// The agent's mode the chat's mode asks for, when it is not the one the
    /// session is in: its read-only one for Plan, and back from it otherwise.
    pub fn mode_wanted(&self) -> Option<String> {
        let modes = self.modes.as_ref()?;
        let wanted = match self.mode.as_deref() == Some(PLAN) {
            true => PLAN.to_owned(),
            false if modes.current.as_deref() == Some(PLAN) => self
                .mode_before
                .clone()
                .or_else(|| modes.offered.iter().find(|one| *one != PLAN).cloned())?,
            false => return None,
        };
        (modes.current.as_ref() != Some(&wanted)).then_some(wanted)
    }

    /// Plan, on an agent that offers no read-only mode: refused rather than
    /// run as if it were one.
    pub fn plan_refused(&self) -> bool {
        self.mode.as_deref() == Some(PLAN)
            && !self
                .modes
                .as_ref()
                .is_some_and(|modes| modes.offered.iter().any(|one| one == PLAN))
    }

    /// The request that puts the session in the agent's mode `value`.
    pub fn set_mode(&mut self, value: &str, sent: Sent) -> Option<String> {
        let session = self.id.clone()?;
        let modes = self.modes.as_ref()?;
        if !modes.offered.iter().any(|one| one == value) {
            return None;
        }
        let (method, params) = match &modes.option {
            Some(option) => (
                "session/set_config_option",
                json!({ "sessionId": session, "configId": option, "value": value }),
            ),
            None => (
                "session/set_mode",
                json!({ "sessionId": session, "modeId": value }),
            ),
        };
        self.mode_asked = Some(value.to_owned());
        Some(self.request(method, params, sent))
    }

    /// The agent took the mode asked: an answer that does not say so means it did.
    pub fn mode_taken(&mut self, result: &Value) {
        let asked = self.mode_asked.take();
        self.options(result);
        if let (Some(modes), Some(asked)) = (self.modes.as_mut(), asked) {
            modes.current = Some(asked);
        }
    }

    /// The model asked for, refused by the agent: listed as unavailable with
    /// its reason, and said before the turn's answer.
    pub fn refuse(&mut self, why: String) {
        let Some(model) = self.model.take() else {
            return;
        };
        self.said_first = Some(format!("{model} is unavailable here: {why}\n\n"));
        self.refused.retain(|one| one.id != model);
        self.refused.push(OfferedModel {
            name: model.clone(),
            id: model,
            unavailable: Some(why),
        });
    }

    /// Every model the session offers, and each it refused with why.
    fn models(&self) -> Vec<OfferedModel> {
        let refused = |id: &str| self.refused.iter().find(|one| one.id == id);
        let mut all: Vec<OfferedModel> = self
            .offers
            .iter()
            .map(|one| refused(&one.id).cloned().unwrap_or_else(|| one.clone()))
            .collect();
        let unlisted = self
            .refused
            .iter()
            .filter(|one| !self.offers.iter().any(|offer| offer.id == one.id));
        all.extend(unlisted.cloned());
        all
    }

    /// The request that switches the session to `model`, when the agent takes one.
    pub fn pick(&mut self, model: &str, sent: Sent) -> Option<String> {
        let session = self.id.clone()?;
        let (method, params) = match self.picks.clone()? {
            Picks::Option(option) => (
                "session/set_config_option",
                json!({ "sessionId": session, "configId": option, "value": model }),
            ),
            Picks::ById => (
                "session/set_model",
                json!({ "sessionId": session, "modelId": model }),
            ),
        };
        self.model = Some(model.to_owned());
        Some(self.request(method, params, sent))
    }

    fn init(&self) -> SessionInit {
        SessionInit {
            model: self.current_model.clone(),
            models: self.models(),
            slash_commands: self.commands.clone(),
            ..SessionInit::default()
        }
    }

    /// What this process has spent, as a CLI's running total: the resident
    /// takes a turn's share as the difference.
    fn spent(&self) -> Option<f64> {
        Some((self.cost? - self.cost_from?).max(0.0))
    }

    fn thought(&mut self) -> Option<Part> {
        let since = self.thinking_since.take()?;
        let text = std::mem::take(&mut self.thought);
        Some(Part::Thinking {
            text,
            parent: None,
            duration_ms: Some(u32::try_from(since.elapsed().as_millis()).unwrap_or(u32::MAX)),
        })
    }

    fn update(&mut self, update: &Value) -> Read {
        match update["sessionUpdate"].as_str().unwrap_or_default() {
            "usage_update" => {
                self.usage(update);
                Read::Nothing
            }
            _ if self.replaying => Read::Nothing,
            "agent_message_chunk" => match update["content"]["text"].as_str() {
                Some(text) => self.parts(vec![Part::Text {
                    text: text.to_owned(),
                    parent: None,
                }]),
                None => Read::Nothing,
            },
            "agent_thought_chunk" => {
                if let Some(text) = update["content"]["text"].as_str() {
                    self.thinking_since.get_or_insert_with(Instant::now);
                    self.thought.push_str(text);
                }
                Read::Nothing
            }
            "tool_call" | "tool_call_update" => {
                let parts = self.tool(update);
                self.parts(parts)
            }
            "available_commands_update" => {
                self.commands = update["availableCommands"]
                    .as_array()
                    .map(|all| {
                        all.iter()
                            .filter_map(|one| one["name"].as_str().map(|name| format!("/{name}")))
                            .collect()
                    })
                    .unwrap_or_default();
                Read::Init(self.init())
            }
            "config_option_update" => {
                self.options(update);
                Read::Nothing
            }
            _ => Read::Nothing,
        }
    }

    /// `parts`, after what is said first and a thought that ended where they begin.
    fn parts(&mut self, parts: Vec<Part>) -> Read {
        let first = self
            .said_first
            .take()
            .map(|text| Part::Text { text, parent: None });
        let all: Vec<Part> = first
            .into_iter()
            .chain(self.thought())
            .chain(parts)
            .collect();
        if all.is_empty() {
            return Read::Nothing;
        }
        Read::Parts(all)
    }

    fn usage(&mut self, update: &Value) {
        if let (Some(used), Some(window)) = (update["used"].as_u64(), update["size"].as_u64()) {
            self.context = u32::try_from(used)
                .ok()
                .zip(u32::try_from(window).ok())
                .map(|(used, window)| Context { used, window });
        }
        // Only dollars: a total in another currency is not one devpit can add up.
        let Some(amount) = update["cost"]["amount"].as_f64() else {
            return;
        };
        if update["cost"]["currency"] != "USD" {
            return;
        }
        if self.cost_from.is_none() && !self.prompted {
            self.cost_from = Some(amount);
        }
        self.cost = Some(amount);
    }

    /// A tool call is drawn once it has its input or has started, and closed
    /// by its result.
    fn tool(&mut self, update: &Value) -> Vec<Part> {
        let Some(id) = update["toolCallId"].as_str() else {
            return Vec::new();
        };
        if let Some(kind) = update["kind"].as_str() {
            self.kinds.insert(id.to_owned(), kind.to_owned());
        }
        if let Some(input) = update.get("rawInput").filter(|input| !input.is_null()) {
            self.inputs.insert(id.to_owned(), input.clone());
        }
        let status = update["status"].as_str().unwrap_or("pending");
        let mut parts = Vec::new();
        let ready =
            update.get("rawInput").is_some_and(|input| !input.is_null()) || status != "pending";
        if ready && self.shown.insert(id.to_owned()) {
            parts.push(Part::ToolCall {
                id: id.to_owned(),
                name: tool_name(update, self.kinds.get(id)),
                input: input_of(update),
                state: CallState::Running,
                parent: None,
            });
        }
        if matches!(status, "completed" | "failed") {
            parts.push(Part::ToolResult {
                call_id: id.to_owned(),
                output: output_of(update),
                is_error: status == "failed",
                parent: None,
            });
        }
        parts
    }
}

/// The models a session lists under `id`: a select's options, flat or grouped,
/// or the models an agent reports.
fn offers_in(list: &Value, id: &str) -> Vec<OfferedModel> {
    list.as_array()
        .into_iter()
        .flatten()
        .flat_map(|one| match one["options"].as_array() {
            Some(group) => group.clone(),
            None => vec![one.clone()],
        })
        .filter_map(|one| {
            let id = one[id].as_str()?.to_owned();
            let name = one["name"].as_str().unwrap_or(&id).to_owned();
            Some(OfferedModel {
                id,
                name,
                unavailable: None,
            })
        })
        .collect()
}

/// What the screen calls a tool: its programmatic name, else its kind, else
/// what the agent titled the call.
fn tool_name(update: &Value, kind: Option<&String>) -> String {
    update["name"]
        .as_str()
        .or(kind.map(String::as_str))
        .or(update["title"].as_str())
        .unwrap_or("tool")
        .to_owned()
}

fn input_of(update: &Value) -> String {
    match update.get("rawInput") {
        Some(input) if !input.is_null() => input.to_string(),
        _ => update["title"].as_str().unwrap_or_default().to_owned(),
    }
}

/// A call's result as text: what it said, the files it changed, or its raw output.
fn output_of(update: &Value) -> String {
    let said: Vec<String> = update["content"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|one| match one["type"].as_str() {
                    Some("content") => one["content"]["text"].as_str().map(str::to_owned),
                    Some("diff") => one["path"].as_str().map(|path| format!("edited {path}")),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    if !said.is_empty() {
        return said.join("\n");
    }
    match update.get("rawOutput") {
        Some(Value::String(text)) => text.clone(),
        Some(output) if !output.is_null() => output.to_string(),
        _ => String::new(),
    }
}

/// Why this agent's CLI cannot hold a chat: it is older than the first
/// version that serves ACP, and would fail on its first line. A `--version`
/// that says nothing in time is left to try.
pub fn outdated(base: &str, program: &str) -> Option<String> {
    let agent = devpit_pty::agents::known(base)?;
    let since = agent.chat_since?;
    let said = crate::providers::version_of(
        std::path::Path::new(program),
        devpit_pty::login_path::search_path(),
        crate::providers::VERSION_WAIT,
    )?;
    too_old(agent.label, since, &said)
}

/// The refusal for a CLI whose `--version` said `said`, when it is older than
/// `since`. One that says no version is left to try.
pub(crate) fn too_old(label: &str, since: &str, said: &str) -> Option<String> {
    let (found, needed) = (version_in(said)?, version_in(since)?);
    (found < needed).then(|| {
        format!(
            "Update the {label} CLI: a chat needs {since} or newer, and this one is {}.{}.{}",
            found.0, found.1, found.2
        )
    })
}

/// The first `major.minor.patch` in `text`.
fn version_in(text: &str) -> Option<(u32, u32, u32)> {
    text.split(|c: char| !c.is_ascii_digit() && c != '.')
        .find_map(|word| {
            let mut parts = word.split('.').map(str::parse::<u32>);
            Some((
                parts.next()?.ok()?,
                parts.next()?.ok()?,
                parts.next()?.ok()?,
            ))
        })
}

/// The message of a JSON-RPC error, as the agent wrote it: its details' own
/// message where it gave one, which says more than the code's name.
pub(crate) fn error_of(said: &Value) -> Option<String> {
    let error = said.get("error")?;
    Some(
        error["data"]["message"]
            .as_str()
            .or(error["message"].as_str())
            .unwrap_or("the agent refused")
            .to_owned(),
    )
}

impl Driver for Acp {
    fn name(&self) -> &'static str {
        "acp"
    }

    fn wire(&self) -> Arc<dyn Wire> {
        Arc::new(acp_wire::AcpWire::new(self.args, self.session.clone()))
    }

    /// What the agent runs when nobody picked: each serves its own list.
    fn models(&self) -> &'static [&'static str] {
        &["default"]
    }

    /// No hook: every request the mode leaves open goes to the person.
    fn asks(&self) -> bool {
        true
    }

    fn metered(&self) -> bool {
        false
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
        let mut session = self.held();
        if said["method"] == "session/update" {
            return session.update(&said["params"]["update"]);
        }
        if said.get("method").is_some() {
            return Read::Nothing;
        }
        if let Some(why) = session.failed.take() {
            return Read::Failed(why);
        }
        if std::mem::take(&mut session.stop_now) {
            return Read::Ended {
                stop_reason: Some("cancelled".to_owned()),
                cost_usd: None,
                is_error: false,
                context: session.context,
            };
        }
        let Some(id) = said["id"].as_u64() else {
            return Read::Nothing;
        };
        if session.opened_by == Some(id) {
            return match session.notice.take() {
                Some(notice) => Read::Parts(vec![Part::Text {
                    text: notice,
                    parent: None,
                }]),
                None => Read::Init(session.init()),
            };
        }
        if session.sent.get(&id) != Some(&Sent::Prompt) {
            return Read::Nothing;
        }
        session.sent.remove(&id);
        if let Some(why) = error_of(&said) {
            return Read::Failed(why);
        }
        // A thought with no answer after it ends with the turn.
        session.thought();
        Read::Ended {
            stop_reason: said["result"]["stopReason"].as_str().map(str::to_owned),
            cost_usd: session.spent(),
            is_error: false,
            context: session.context,
        }
    }

    fn session(&self, line: &str) -> Option<String> {
        let said = serde_json::from_str::<Value>(line).ok()?;
        if let Some(id) = said["params"]["sessionId"].as_str() {
            return Some(id.to_owned());
        }
        let session = self.held();
        let opened = said["id"].as_u64().is_some() && said["id"].as_u64() == session.opened_by;
        opened.then(|| session.id.clone()).flatten()
    }
}

#[cfg(test)]
#[path = "acp_tests.rs"]
mod tests;
