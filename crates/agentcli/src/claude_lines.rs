//! Claude Code's stream-json, line by line.
//!
//! Apart from `driver.rs` because that file is the contract every CLI meets,
//! and this is one vendor's grammar — the part with cases in it, and the part
//! that changes when the vendor does. Shapes were read off a recorded turn of
//! 2.1.270 (`tests/fixtures/`), not off documentation.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use devpit_rpc::{CallState, Context, Part, SessionInit};

use crate::driver::Read;

fn content_of(value: &serde_json::Value) -> &[serde_json::Value] {
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// The `Agent` call a subagent's line belongs to. Top-level on the line, not
/// inside the message — measured against Claude Code 2.1.270.
fn parent_of(value: &serde_json::Value) -> Option<String> {
    value
        .get("parent_tool_use_id")
        .and_then(|parent| parent.as_str())
        .map(str::to_owned)
}

fn strings_at(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|found| found.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|one| one.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn optional_at(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|found| found.as_str())
        .map(str::to_owned)
}

/// The `system` lines worth keeping: the session's self-description, and the
/// lifecycle of work the CLI runs in the background. Hook chatter, token
/// estimates and rate limits stay out of the conversation.
pub(crate) fn system(value: &serde_json::Value) -> Read {
    let task = |status: String| {
        Read::Parts(vec![Part::Task {
            task_id: string_at(value, "task_id"),
            call_id: optional_at(value, "tool_use_id"),
            task_kind: optional_at(value, "task_type"),
            description: optional_at(value, "description"),
            status,
            summary: optional_at(value, "summary"),
            duration_ms: value
                .pointer("/usage/duration_ms")
                .and_then(|ms| ms.as_u64())
                .and_then(|ms| u32::try_from(ms).ok()),
        }])
    };
    match value.get("subtype").and_then(|kind| kind.as_str()) {
        Some("init") => Read::Init(SessionInit {
            model: optional_at(value, "model"),
            slash_commands: strings_at(value, "slash_commands"),
            terminal_slash_commands: strings_at(value, "terminal_slash_commands"),
            skills: strings_at(value, "skills"),
            agents: strings_at(value, "agents"),
        }),
        // A slash command's own answer (`/compact`, `/clear`), which the CLI
        // prints as a system line rather than as the agent speaking.
        Some("local_command") => Read::Parts(vec![Part::Command {
            content: string_at(value, "content"),
        }]),
        Some("task_started") => task("started".to_owned()),
        Some("task_progress") => task("running".to_owned()),
        Some("task_updated") => match value
            .get("patch")
            .and_then(|patch| patch.get("status"))
            .and_then(|status| status.as_str())
        {
            Some(status) => task(status.to_owned()),
            None => Read::Nothing,
        },
        Some("task_notification") => {
            task(optional_at(value, "status").unwrap_or_else(|| "completed".to_owned()))
        }
        _ => Read::Nothing,
    }
}

/// Which message each speaker is streaming, and what of it came as events: the
/// whole block its `assistant` line repeats is then not drawn twice.
#[derive(Default)]
pub struct Streamed(Mutex<HashMap<Option<String>, (String, Seen)>>);

/// What a message's stream already drew.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Seen {
    pub text: bool,
    /// When a thinking block began. Its text is withheld from the stream, so
    /// the block the `assistant` line repeats says when it ended.
    pub thinking_since: Option<Instant>,
}

impl Streamed {
    /// A `stream_event`: text deltas become text; the rest is the whole line's.
    pub(crate) fn event(&self, value: &serde_json::Value) -> Read {
        let parent = parent_of(value);
        let Some(event) = value.get("event") else {
            return Read::Nothing;
        };
        let Ok(mut open) = self.0.lock() else {
            return Read::Nothing;
        };
        match event.get("type").and_then(|kind| kind.as_str()) {
            Some("message_start") => {
                let id = event.get("message").map(|message| string_at(message, "id"));
                open.insert(parent, (id.unwrap_or_default(), Seen::default()));
                Read::Nothing
            }
            // Said when it begins, not when it ends: seconds can pass in it.
            Some("content_block_start")
                if event
                    .pointer("/content_block/type")
                    .and_then(|kind| kind.as_str())
                    == Some("thinking") =>
            {
                if let Some(streaming) = open.get_mut(&parent) {
                    streaming.1.thinking_since = Some(Instant::now());
                }
                Read::Parts(vec![Part::Thinking {
                    text: String::new(),
                    parent,
                    duration_ms: None,
                }])
            }
            Some("content_block_delta") => {
                let delta = event.get("delta");
                let text = delta
                    .filter(|delta| {
                        delta.get("type").and_then(|kind| kind.as_str()) == Some("text_delta")
                    })
                    .and_then(|delta| delta.get("text")?.as_str());
                let Some(text) = text.filter(|text| !text.is_empty()) else {
                    return Read::Nothing;
                };
                if let Some(streaming) = open.get_mut(&parent) {
                    streaming.1.text = true;
                }
                Read::Parts(vec![Part::Text {
                    text: text.to_owned(),
                    parent,
                }])
            }
            _ => Read::Nothing,
        }
    }

    /// What of this `assistant` line's message already arrived as events.
    pub(crate) fn already(&self, value: &serde_json::Value) -> Seen {
        let id = value.get("message").map(|message| string_at(message, "id"));
        self.0
            .lock()
            .ok()
            .and_then(|open| {
                open.get(&parent_of(value))
                    .filter(|(streaming, _)| Some(streaming) == id.as_ref())
                    .map(|(_, seen)| *seen)
            })
            .unwrap_or_default()
    }
}

pub(crate) fn assistant_parts(value: &serde_json::Value, streamed: Seen) -> Vec<Part> {
    let parent = parent_of(value);
    content_of(value)
        .iter()
        .filter_map(
            |block| match block.get("type").and_then(|kind| kind.as_str()) {
                Some("text") if streamed.text => None,
                Some("text") => block
                    .get("text")
                    .and_then(|text| text.as_str())
                    .map(|text| Part::Text {
                        text: text.to_owned(),
                        parent: parent.clone(),
                    }),
                // Streamed, it began as its own part; this one closes it.
                Some("thinking") => {
                    block
                        .get("thinking")
                        .and_then(|text| text.as_str())
                        .map(|text| Part::Thinking {
                            text: text.to_owned(),
                            parent: parent.clone(),
                            duration_ms: streamed.thinking_since.map(|since| {
                                u32::try_from(since.elapsed().as_millis()).unwrap_or(u32::MAX)
                            }),
                        })
                }
                Some("tool_use") => Some(Part::ToolCall {
                    id: string_at(block, "id"),
                    name: string_at(block, "name"),
                    input: block
                        .get("input")
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    state: CallState::Running,
                    parent: parent.clone(),
                }),
                _ => None,
            },
        )
        .collect()
}

pub(crate) fn tool_results(value: &serde_json::Value) -> Vec<Part> {
    let parent = parent_of(value);
    content_of(value)
        .iter()
        .filter(|block| block.get("type").and_then(|kind| kind.as_str()) == Some("tool_result"))
        .map(|block| Part::ToolResult {
            call_id: string_at(block, "tool_use_id"),
            output: block
                .get("content")
                .map(|content| match content.as_str() {
                    Some(text) => text.to_owned(),
                    None => content.to_string(),
                })
                .unwrap_or_default(),
            is_error: block
                .get("is_error")
                .and_then(|flag| flag.as_bool())
                .unwrap_or(false),
            parent: parent.clone(),
        })
        .collect()
}

pub(crate) fn string_at(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|found| found.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// A line saying the provider refused the turn, kept with what was heard
/// before it: the reset comes on its own line, ahead of the error.
pub(crate) fn limit(line: &str, was: Option<devpit_rpc::Limit>) -> Option<devpit_rpc::Limit> {
    use devpit_rpc::{Limit, LimitKind};
    // Most lines are neither: only these are worth parsing.
    if !line.contains("rate_limit_event") && !line.contains("\"error\"") {
        return was;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return was;
    };
    let resets_at = was.and_then(|limit| limit.resets_at);
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("rate_limit_event") => {
            let info = &value["rate_limit_info"];
            if info["status"] != "rejected" {
                return was;
            }
            Some(Limit {
                kind: LimitKind::RateLimit,
                resets_at: info["resetsAt"].as_f64().or(resets_at),
            })
        }
        Some("assistant") => match value["error"].as_str() {
            Some("rate_limit") => Some(Limit {
                kind: LimitKind::RateLimit,
                resets_at,
            }),
            Some("server_error") => Some(Limit {
                kind: LimitKind::Outage,
                resets_at: None,
            }),
            _ => was,
        },
        _ => was,
    }
}

/// Measured on 2.1.270: `usage.iterations` holds one entry per request the
/// turn made, and the last one is what the context held when it ended — the
/// top-level `usage` sums them, which overcounts any turn that used a tool.
/// The window is the main model's, from `modelUsage`.
pub(crate) fn context_of(result: &serde_json::Value) -> Option<Context> {
    let last = result.get("usage")?.get("iterations")?.as_array()?.last()?;
    let tokens = |key: &str| last.get(key).and_then(|n| n.as_u64()).unwrap_or(0);
    let used = tokens("input_tokens")
        + tokens("cache_read_input_tokens")
        + tokens("cache_creation_input_tokens")
        + tokens("output_tokens");
    let window = result
        .get("modelUsage")?
        .as_object()?
        .values()
        .filter_map(|model| model.get("contextWindow")?.as_u64())
        .max()?;
    Some(Context {
        used: u32::try_from(used).ok()?,
        window: u32::try_from(window).ok()?,
    })
}
