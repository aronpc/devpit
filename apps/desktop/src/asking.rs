//! Permission questions, from the agent to the person and back.
//!
//! A `PreToolUse` hook can answer with a decision, which is the round trip
//! this needs: the agent asks, the hook holds, the window shows it, the person
//! answers. The hook that reports what a card is doing already runs on this
//! path — the same event, asked one more question.
//!
//! **The critical path stays fast by default.** Only a session that has said
//! it wants to be asked is held; every other one is answered at once with no
//! decision, which leaves the CLI's own permission mode in charge.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use devpit_rpc::{ErrorCode, RpcError};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::asking_wait::Ends;

/// What the agent wants to do, as the screen puts it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// This question, so the answer can name it.
    pub id: String,
    /// The CLI's session, which is what ties it to a conversation.
    pub session_id: String,
    pub tool: String,
    /// The tool's input, verbatim. Parsing it here would be guessing at a
    /// shape the provider changes without asking.
    pub input: String,
    pub cwd: String,
    /// What it touches, as the island reads it: the choices, the command.
    #[serde(default)]
    pub touch: Option<devpit_rpc::Touch>,
    /// The chat whose process asked; none for one started outside a chat.
    #[serde(default)]
    pub conversation_id: Option<String>,
}

/// The chat mode that stops to ask — the screen's `ASKS`. A turn started in
/// it is held from the moment its session is named, so a new conversation's
/// first turn asks too, not only the ones after it.
pub const ASKING_MODE: &str = "manual";

/// What a chat turn does with its session id once the stream names it: tells
/// the screen, marks it a chat's, and in the asking mode holds its tools —
/// so the screen that shows the question already knows whose it is.
pub fn holding(
    mode: Option<&str>,
    app: &tauri::AppHandle,
    frames: tauri::ipc::Channel<devpit_rpc::Frame>,
) -> impl Fn(&str) + Sync + Send {
    let (app, asks) = (app.clone(), mode == Some(ASKING_MODE));
    move |session: &str| {
        use tauri::Manager;
        if asks {
            app.state::<Asking>().ask_from_now(session, true);
        }
        let _ = frames.send(devpit_rpc::Frame::Session {
            session_id: session.to_owned(),
        });
    }
}

/// What a person answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Allow,
    /// Refused. The refusal goes back to the agent as a tool result rather
    /// than killing the turn — an agent told "no" can try something else.
    Deny,
}

/// The questions in flight, and the sessions that want to be asked.
#[derive(Default)]
pub struct Asking {
    waiting: Arc<Mutex<HashMap<String, Sender<Answer>>>>,
    /// Sessions whose turns should stop and ask. Everything else is answered
    /// at once, so a board step never waits on a window nobody is watching.
    asked: Arc<Mutex<Vec<String>>>,
}

impl Asking {
    /// Whether this session wants its tools held for an answer.
    pub fn asks(&self, session_id: &str) -> bool {
        self.asked
            .lock()
            .map(|held| held.iter().any(|one| one == session_id))
            .unwrap_or(false)
    }

    pub fn ask_from_now(&self, session_id: &str, on: bool) {
        let Ok(mut held) = self.asked.lock() else {
            return;
        };
        held.retain(|one| one != session_id);
        if on {
            held.push(session_id.to_owned());
        }
    }

    /// Registers a question and hands back the end that waits on it.
    pub fn opened(&self, id: &str) -> Receiver<Answer> {
        let (tell, hear) = channel();
        if let Ok(mut held) = self.waiting.lock() {
            held.insert(id.to_owned(), tell);
        }
        hear
    }

    /// Waits for the person as long as the question waits (`asking_wait`).
    pub fn wait_for(&self, question: &Question, hear: Receiver<Answer>, ends: &Ends) -> Answer {
        let answer = crate::asking_wait::waits(&hear, question, ends, &crate::asking_wait::Wall);
        drop(self.forget(&question.id));
        answer
    }

    /// The wait for a question that changes something, for a test that has
    /// already answered it or dropped the sender.
    #[cfg(test)]
    pub fn wait(&self, id: &str, hear: Receiver<Answer>) -> Answer {
        let answer = crate::asking_wait::within(&hear, &crate::asking_wait::Wall);
        drop(self.forget(id));
        answer
    }

    fn forget(&self, id: &str) -> Option<Sender<Answer>> {
        self.waiting.lock().ok()?.remove(id)
    }

    /// Answers a question that is waiting. False when none was.
    pub fn answer(&self, id: &str, answer: Answer) -> bool {
        self.forget(id)
            .map(|tell| tell.send(answer).is_ok())
            .unwrap_or(false)
    }

    pub(crate) fn end(&self, id: &str) -> bool {
        self.answer(id, Answer::Deny)
    }
}

/// The hook's reply, in the shape the CLI reads it.
///
/// `ask` is deliberately never sent: this *is* the asking, and handing the
/// question back would loop.
pub fn decision(answer: Answer) -> String {
    let (verdict, why) = match answer {
        Answer::Allow => ("allow", "allowed in devpit"),
        Answer::Deny => ("deny", "not allowed in devpit"),
    };
    format!(
        r#"{{"hookSpecificOutput":{{"hookEventName":"PreToolUse","permissionDecision":"{verdict}","permissionDecisionReason":"{why}"}}}}"#
    )
}

/// `permission.answer` — what the person said.
#[tauri::command]
#[specta::specta]
pub fn permission_answer(
    state: tauri::State<'_, Asking>,
    id: String,
    answer: Answer,
) -> Result<(), RpcError> {
    if state.answer(&id, answer) {
        return Ok(());
    }
    Err(RpcError::new(
        ErrorCode::NotFound,
        "that question is no longer waiting — it timed out, or the turn ended",
    ))
}

/// `permission.ask_from_now` — whether this conversation stops to ask.
///
/// Per session rather than global: a board step running unattended must not
/// start waiting because a chat window asked to be consulted.
#[tauri::command]
#[specta::specta]
pub fn permission_ask_from_now(
    state: tauri::State<'_, Asking>,
    session_id: String,
    on: bool,
) -> Result<(), RpcError> {
    state.ask_from_now(&session_id, on);
    Ok(())
}

#[cfg(test)]
#[path = "asking_tests.rs"]
mod tests;
