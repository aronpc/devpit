//! What a driver writes to its CLI: the other half of reading it.
//!
//! A chat talks back while the turn runs — the person's next message, a
//! permission answer, a stop — and each CLI wants those in its own shape.
//! A wire is shared by everyone who may write to a running process, so it is
//! handed out behind an `Arc` and keeps whatever state its protocol needs.

use serde_json::Value;

use crate::control::{
    interrupt_request, prompt_of, refusal, steer_line, stop_task_request, user_line, Prompt,
};
use crate::talk::Say;

/// A permission prompt read off the stream.
pub struct Asked {
    /// What it asks, when the line could be read that far.
    pub prompt: Option<Prompt>,
    /// The line that refuses it, written when nobody takes the prompt.
    pub refusal: String,
}

/// How a switch on the running process is taken.
pub enum Switch {
    /// A line for the CLI; its answer comes back on the stream.
    Write(String),
    /// Kept by devpit itself, with nothing for the CLI to hear: taken at once.
    Kept,
}

pub trait Wire: Send + Sync {
    /// The command line a chat turn starts the CLI with.
    fn argv(&self, turn: &Say<'_>) -> Vec<String>;

    /// Written once the process starts, before the first message: a
    /// protocol's handshake. Empty for a CLI that takes messages at once.
    fn hello(&self, _turn: &Say<'_>) -> Vec<String> {
        Vec::new()
    }

    /// The lines that hand it the person's message. Empty while a handshake
    /// is still going: the wire holds the message and sends it as a reply.
    fn prompt(&self, text: &str) -> Vec<String>;

    /// A message for the turn in flight, under `uuid`. `None` when the CLI
    /// takes none mid-turn, so it waits for the next turn instead.
    fn steer(&self, _text: &str, _uuid: &str) -> Option<String> {
        None
    }

    /// Stops the turn in flight and keeps the process. Empty when it cannot.
    fn interrupt(&self) -> Vec<String>;

    /// Stops one background task, by the id the CLI gave it.
    fn stop_task(&self, _task_id: &str) -> Option<String> {
        None
    }

    /// The permission prompt on `line`, if it is one.
    fn asked(&self, line: &str, session_id: Option<&str>) -> Option<Asked>;

    /// devpit's answer to a prompt — `{behavior: "allow"}` or
    /// `{behavior: "deny", message}` — in the CLI's own shape.
    fn answer(&self, request_id: &str, response: Value) -> Option<String>;

    /// A switch on the running process (`set_model`, `set_permission_mode`)
    /// under `id`. `None` when only a new process can take it.
    fn control(&self, id: &str, request: Value) -> Option<Switch>;

    /// The CLI's answer to a switch, when `line` is one: `{subtype, request_id}`.
    fn control_answer(&self, line: &str) -> Option<Value>;

    /// What `line` calls for in reply: the next step of a handshake, or an
    /// answer to a request the CLI made of devpit. A permission request
    /// answered here is not one [`Wire::asked`] reads.
    fn replies(&self, _line: &str) -> Vec<String> {
        Vec::new()
    }
}

/// Claude Code's stream-json input: one JSON object per line on stdin.
pub struct Claude;

impl Wire for Claude {
    fn argv(&self, turn: &Say<'_>) -> Vec<String> {
        crate::talk_args::argv(turn)
    }

    fn prompt(&self, text: &str) -> Vec<String> {
        vec![user_line(text)]
    }

    fn steer(&self, text: &str, uuid: &str) -> Option<String> {
        Some(steer_line(text, uuid))
    }

    fn interrupt(&self) -> Vec<String> {
        vec![interrupt_request()]
    }

    fn stop_task(&self, task_id: &str) -> Option<String> {
        Some(stop_task_request(task_id))
    }

    fn asked(&self, line: &str, session_id: Option<&str>) -> Option<Asked> {
        Some(Asked {
            refusal: refusal(line)?,
            prompt: prompt_of(line, session_id),
        })
    }

    fn answer(&self, request_id: &str, response: Value) -> Option<String> {
        Some(
            serde_json::json!({
                "type": "control_response",
                "response": { "subtype": "success", "request_id": request_id, "response": response },
            })
            .to_string(),
        )
    }

    fn control(&self, id: &str, request: Value) -> Option<Switch> {
        Some(Switch::Write(
            serde_json::json!({ "type": "control_request", "request_id": id, "request": request })
                .to_string(),
        ))
    }

    fn control_answer(&self, line: &str) -> Option<Value> {
        if !line.contains("\"control_response\"") {
            return None;
        }
        let said = serde_json::from_str::<Value>(line).ok()?;
        Some(said["response"].clone())
    }
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
