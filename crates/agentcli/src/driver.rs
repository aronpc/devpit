//! One driver per CLI, behind one trait. Each vendor prints its own shape and
//! changes it without asking; a driver turns those lines into the contract's
//! typed parts, so a new CLI is a new file here and nothing else.

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
    /// The turn ended in an error the CLI described.
    Failed(String),
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

    /// Whether a permission prompt on its stream is the person's to answer.
    /// Claude Code asks the person through its hook, and only a plan comes here.
    fn asks(&self) -> bool {
        false
    }

    /// Whether every turn says what it cost and how full the context is.
    fn metered(&self) -> bool {
        true
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

    fn echoed(&self, line: &str) -> Option<String> {
        crate::claude_lines::echoed(line)
    }

    fn anchor(&self, line: &str) -> Option<String> {
        crate::claude_lines::anchor(line)
    }
}

/// The driver a conversation named, or nothing when it is not installed.
pub fn driver(name: &str) -> Option<Box<dyn Driver>> {
    driver_for(name, "")
}

/// [`driver`], started the way the profile's base agent serves a chat.
pub fn driver_for(name: &str, base: &str) -> Option<Box<dyn Driver>> {
    let agent = devpit_pty::agents::known(base);
    let args = agent.map_or(&[][..], |agent| agent.chat_args);
    let sign_in = agent.and_then(|agent| agent.chat_sign_in);
    match name {
        "claude" => Some(Box::<Claude>::default()),
        "codex" => Some(Box::<crate::codex::Codex>::default()),
        "acp" => Some(Box::new(crate::acp::Acp::new(args).signing_in(sign_in))),
        _ => None,
    }
}
