//! Hearing what an agent is doing, from the agent itself.
//!
//! `claude agents --json` answers "is it working" once per poll. A hook answers
//! "it is about to run a command called X" at the moment it happens, and it is
//! the agent telling us rather than us inferring it from a terminal title or a
//! regex over output — which is guesswork that breaks the first time the output
//! changes.
//!
//! The shape is taken from what the CLI actually sends, recorded rather than
//! read from documentation:
//!
//! ```text
//! PreToolUse -> cwd, hook_event_name, permission_mode, prompt_id, session_id,
//!               tool_input, tool_name, tool_use_id, transcript_path
//! Stop       -> ..., last_assistant_message, stop_hook_active
//! ```
//!
//! Two rules the receiver keeps, both learned from how these fail elsewhere:
//!
//! 1. **The hook runs on the agent's critical path.** It gets a short timeout
//!    and gives up rather than holding the agent while a dead app is waited on.
//! 2. **The endpoint lives on disk and is read on every invocation.** A pty
//!    that outlived a restart would otherwise keep posting to a port nothing is
//!    listening on.

use std::path::{Path, PathBuf};

use devpit_rpc::Touch;
use serde::Deserialize;

use crate::touch::{target_of, touch_of};

/// What a hook told us.
#[derive(Debug, Clone, PartialEq)]
pub struct Happening {
    pub session_id: String,
    pub event: Event,
    pub cwd: String,
    /// The session's transcript. Its folder names the installation that wrote
    /// it, which decides who may resume the session somewhere else.
    pub transcript_path: Option<String>,
}

/// The events worth reacting to.
///
/// Deliberately fewer than the CLI sends: an event the board cannot draw is an
/// event nobody asked for, and every one of these changes what a card shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// About to run a tool, named, with what it is run on when its input says.
    Using {
        tool: String,
        target: Option<String>,
        touch: Option<Touch>,
    },
    /// Finished a tool.
    Used { tool: String },
    /// A tool that failed, which sends no `Used`, with what it said.
    UseFailed { tool: String, error: Option<String> },
    /// A person sent a prompt, so a turn has begun — the only word a turn that
    /// answers in text alone ever sends before its `Stop`.
    Prompted,
    /// The turn ended, with the last thing it said.
    Stopped { said: Option<String> },
    /// The turn ended on an error — an API failure, a rate limit — with the
    /// CLI's word for it. No `Stop` follows one, so without it the card would
    /// say `working` until the next prompt.
    Failed { error: Option<String> },
    /// A subagent began, known so far only by its id and type.
    SubagentStarted { agent: String, kind: Option<String> },
    /// The `Agent` call that launched a subagent returned, naming it. In 2.1.270
    /// the call returns at launch (`async_launched`), so this is not its end.
    Delegated {
        agent: String,
        description: Option<String>,
        model: Option<String>,
        ended: bool,
    },
    /// A subagent it started finished. The agent itself is still working —
    /// reading this as `Stopped` told the sidebar the whole agent was done.
    /// Recorded arriving many times for one subagent, so ending one is idempotent.
    SubagentDone { agent: Option<String> },
    /// Waiting on a person — the state that matters most, because nothing
    /// moves until someone comes back.
    Waiting,
    /// A session began, at startup, on resume, or after `/clear`.
    SessionStarted,
    /// The session ended, with the CLI's reason (`prompt_input_exit` on `/exit`).
    SessionEnded { reason: Option<String> },
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    hook_event_name: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_input: Option<serde_json::Value>,
    #[serde(default)]
    last_assistant_message: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    agent_type: Option<String>,
    #[serde(default)]
    transcript_path: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    notification_type: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// What an `Agent` call returns, read apart from [`Raw`]: other tools answer
/// with strings and arrays, and one strict shape would drop their `Used`.
#[derive(Deserialize)]
struct Launched {
    tool_response: LaunchedAgent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchedAgent {
    agent_id: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    resolved_model: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// Reads one hook payload.
///
/// An event this build has no use for is `None`, not an error: the CLI is free
/// to send more of them, and a receiver that fails on an unknown name is a
/// receiver that breaks on an upgrade.
pub fn read(payload: &str) -> Option<Happening> {
    let raw: Raw = serde_json::from_str(payload).ok()?;
    if raw.session_id.is_empty() {
        return None;
    }

    let event = match raw.hook_event_name.as_str() {
        "PreToolUse" => {
            let tool = raw.tool_name?;
            let input = raw.tool_input.unwrap_or_default();
            Event::Using {
                target: target_of(&input),
                touch: touch_of(&tool, &input),
                tool,
            }
        }
        "PostToolUse" => {
            let tool = raw.tool_name?;
            // Parsed a second time only for `Agent`, not for every tool's output.
            match (tool == "Agent").then(|| launched(payload)).flatten() {
                Some(agent) => Event::Delegated {
                    // Any status but a launch is a call that ran to its end.
                    ended: agent.status.as_deref() != Some("async_launched"),
                    agent: agent.agent_id,
                    description: agent.description,
                    model: agent.resolved_model,
                },
                None => Event::Used { tool },
            }
        }
        "PostToolUseFailure" => Event::UseFailed {
            tool: raw.tool_name?,
            error: raw.error,
        },
        "UserPromptSubmit" => Event::Prompted,
        "Stop" => Event::Stopped {
            said: raw.last_assistant_message,
        },
        "StopFailure" => Event::Failed { error: raw.error },
        "SubagentStart" => Event::SubagentStarted {
            agent: raw.agent_id?,
            kind: raw.agent_type,
        },
        "SubagentStop" => Event::SubagentDone {
            agent: raw.agent_id,
        },
        "Notification" if waits_on_someone(raw.notification_type.as_deref()) => Event::Waiting,
        // A compaction starts the same session over, often in the middle of a
        // turn; read as a start, it told a working card it was only open.
        "SessionStart" if raw.source.as_deref() == Some("compact") => return None,
        "SessionStart" => Event::SessionStarted,
        "SessionEnd" => Event::SessionEnded { reason: raw.reason },
        _ => return None,
    };

    Some(Happening {
        session_id: raw.session_id,
        event,
        cwd: raw.cwd,
        transcript_path: raw.transcript_path,
    })
}

/// Whether a notification is the agent stopped on a person.
///
/// Not all of them are: `idle_prompt` comes a minute after a turn that has
/// already ended, and `auth_success` is news, not a question. Both rang the
/// bell as though something were blocked. A CLI too old to send a type only
/// ever notified to ask, so no type still reads as waiting.
fn waits_on_someone(notification_type: Option<&str>) -> bool {
    matches!(
        notification_type,
        None | Some("permission_prompt") | Some("elicitation_dialog")
    )
}

fn launched(payload: &str) -> Option<LaunchedAgent> {
    serde_json::from_str::<Launched>(payload)
        .ok()
        .map(|launched| launched.tool_response)
}

/// Where the endpoint is written, and read back from.
pub fn endpoint_file(root: &Path) -> PathBuf {
    root.join("hook-endpoint")
}

/// The header a hook carries to say it is one of ours.
///
/// Named here, next to the file that holds it, so the side that writes the
/// header and the side that checks it cannot drift apart.
pub const HOOK_HEADER: &str = "x-devpit-hook";

/// The variable a chat's process is started with, naming its conversation;
/// `devpit_agentapi::hook` names it alike for the Windows hook.
pub(crate) const CHAT_ENV: &str = "DEVPIT_CHAT";

/// The header a hook carries [`CHAT_ENV`] in: a session is not one
/// conversation's, so a question is tied to the chat whose process asked.
pub(crate) const CHAT_HEADER: &str = "x-devpit-chat";

/// Where the secret that header carries is kept.
///
/// A file rather than an argument: the hook command lives in a settings file
/// the agent CLI reads, and everything in that line is visible to anyone who
/// can list processes. `curl -H @file` reads the header from disk instead, and
/// the file is owner-only.
pub fn auth_file(root: &Path) -> PathBuf {
    root.join("hook-auth")
}

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
