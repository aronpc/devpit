//! One driver per CLI, behind one trait.
//!
//! Each vendor prints its own shape and changes it without asking. A driver
//! turns those lines into the typed parts the contract carries, so a new CLI
//! is a new file here and nothing else.

use std::sync::Arc;

use devpit_rpc::{Context, Part, SessionInit};

use crate::claude_lines::{assistant_parts, context_of, system, tool_results, Streamed};
use crate::wire::Wire;

/// What one line of a CLI's output turned into.
#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    /// Parts to append to the message being built.
    Parts(Vec<Part>),
    /// The turn ended, with the CLI's own reason.
    Ended {
        stop_reason: Option<String>,
        cost_usd: Option<f64>,
        is_error: bool,
        context: Option<Context>,
    },
    /// The CLI described itself: its commands, skills and model.
    Init(SessionInit),
    /// Nothing to show — a keepalive, a blank line.
    Nothing,
}

/// One CLI, both ways: its lines read here, and what is written back to it
/// through its [`Wire`].
pub trait Driver: Send + Sync {
    /// The name this driver answers to, and what a conversation records.
    fn name(&self) -> &'static str;

    /// How this CLI is written to, shared by every writer of one process.
    fn wire(&self) -> Arc<dyn Wire>;

    /// The models it offers. The composer picks from these; it never picks a
    /// driver, because that is fixed for the conversation's life.
    fn models(&self) -> &'static [&'static str];

    /// How hard it may be asked to think, and what it does by default.
    ///
    /// Empty for a CLI with no such control — the composer draws no chip
    /// rather than a chip with one option in it.
    fn efforts(&self) -> &'static [&'static str] {
        &[]
    }

    fn effort_default(&self) -> Option<&'static str> {
        None
    }

    /// Reads one line of output.
    fn read(&self, line: &str) -> Read;

    /// The CLI's own id for this conversation, if the line carries it.
    ///
    /// Kept so the next turn resumes the same thread: without it the agent
    /// starts over and the transcript on screen is the only memory left.
    fn session(&self, line: &str) -> Option<String>;

    /// The id the CLI's transcript gives this line, when it is the agent's own
    /// message — what a rewind names to fork at. None for a CLI with no such id.
    fn anchor(&self, _line: &str) -> Option<String> {
        None
    }

    /// The uuid of a message from stdin the CLI echoed back, having taken it in.
    fn echoed(&self, _line: &str) -> Option<String> {
        None
    }
}

/// The Claude Code CLI, which prints one JSON object per line.
#[derive(Default)]
pub struct Claude {
    streamed: Streamed,
}

impl Driver for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn wire(&self) -> Arc<dyn Wire> {
        Arc::new(crate::wire::Claude)
    }

    /// The CLI's own aliases rather than dated ids: an alias follows the
    /// provider (Bedrock, a gateway, `ANTHROPIC_DEFAULT_OPUS_MODEL`), and an id
    /// pinned here would be wrong on every one of them. `default` leads because
    /// it is what the account would run anyway.
    fn models(&self) -> &'static [&'static str] {
        &["default", "opus", "sonnet", "haiku", "fable", "opusplan"]
    }

    /// `--effort`, as the CLI's own help lists it.
    fn efforts(&self) -> &'static [&'static str] {
        &["low", "medium", "high", "xhigh", "max"]
    }

    fn effort_default(&self) -> Option<&'static str> {
        Some("high")
    }

    fn read(&self, line: &str) -> Read {
        let line = line.trim();
        if line.is_empty() {
            return Read::Nothing;
        }

        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            // A line we cannot parse is still output. Dropping it would hide
            // exactly the errors a CLI prints plainly.
            return Read::Parts(vec![Part::Unknown {
                text: line.to_owned(),
            }]);
        };

        match value.get("type").and_then(|kind| kind.as_str()) {
            Some("result") => Read::Ended {
                stop_reason: value
                    .get("subtype")
                    .and_then(|reason| reason.as_str())
                    .map(str::to_owned),
                cost_usd: value.get("total_cost_usd").and_then(|cost| cost.as_f64()),
                is_error: value
                    .get("is_error")
                    .and_then(|flag| flag.as_bool())
                    .unwrap_or(false),
                context: context_of(&value),
            },
            Some("assistant") => {
                Read::Parts(assistant_parts(&value, self.streamed.already(&value)))
            }
            Some("stream_event") => self.streamed.event(&value),
            Some("user") => Read::Parts(tool_results(&value)),
            Some("system") => system(&value),
            _ => Read::Nothing,
        }
    }

    fn session(&self, line: &str) -> Option<String> {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()?
            .get("session_id")?
            .as_str()
            .map(str::to_owned)
    }

    /// Measured on 2.1.323: `{"type":"user","isReplay":true,"uuid":…}`, once
    /// the message is part of the turn, not when it is written.
    fn echoed(&self, line: &str) -> Option<String> {
        if !line.contains("\"isReplay\"") {
            return None;
        }
        let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
        if value.get("isReplay")?.as_bool()? {
            return value.get("uuid")?.as_str().map(str::to_owned);
        }
        None
    }

    /// Measured: a stream-json assistant line's `uuid` is the transcript's.
    fn anchor(&self, line: &str) -> Option<String> {
        let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
        // A subagent's messages live in its own transcript, not this one.
        let own = value
            .get("parent_tool_use_id")
            .is_none_or(|parent| parent.is_null());
        if value.get("type")?.as_str()? != "assistant" || !own {
            return None;
        }
        value.get("uuid")?.as_str().map(str::to_owned)
    }
}

/// The driver a conversation named, or nothing when it is not installed.
pub fn driver(name: &str) -> Option<Box<dyn Driver>> {
    match name {
        "claude" => Some(Box::<Claude>::default()),
        _ => None,
    }
}
