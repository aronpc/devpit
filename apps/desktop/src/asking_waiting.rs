//! The questions still waiting, for a window that was not there to be asked.
//!
//! A woken resident may ask with no chat open, after the event has nobody to
//! hear it. The chat reads these when it opens; settled questions leave.

use std::sync::Mutex;

use devpit_rpc::RpcError;
use tauri::State;

use crate::asking::Question;

/// The questions in flight, in the order they were asked.
#[derive(Default)]
pub struct Waiting(Mutex<Vec<Question>>);

/// The questions still waiting on one session.
#[derive(serde::Serialize, specta::Type)]
pub struct Questions {
    pub questions: Vec<Question>,
}

impl Waiting {
    /// A question the person has not answered yet.
    pub fn opened(&self, question: &Question) {
        if let Ok(mut held) = self.0.lock() {
            held.push(question.clone());
        }
    }

    /// The question is answered, or timed out: no window is offered it again.
    pub fn settled(&self, id: &str) {
        if let Ok(mut held) = self.0.lock() {
            held.retain(|one| one.id != id);
        }
    }

    /// The questions waiting on this session, oldest first.
    pub fn on_session(&self, session_id: &str) -> Vec<Question> {
        self.0
            .lock()
            .map(|held| {
                held.iter()
                    .filter(|one| one.session_id == session_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// `permission.questions` — the questions still waiting on a session, drawn
/// by a chat that opens after they were asked.
#[tauri::command]
#[specta::specta]
pub fn permission_questions(
    state: State<'_, Waiting>,
    session_id: String,
) -> Result<Questions, RpcError> {
    Ok(Questions {
        questions: state.on_session(&session_id),
    })
}

#[cfg(test)]
#[path = "asking_waiting_tests.rs"]
mod tests;
