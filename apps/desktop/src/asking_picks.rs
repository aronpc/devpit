//! What an agent asks the person rather than asks to do: `AskUserQuestion`,
//! a plan to approve. A chat's question goes to the CLI's own prompt
//! (`asking_plan`) — the hook never holds it — and the picks kept here are
//! the person's answers.

use std::collections::HashMap;
use std::sync::Mutex;

use devpit_rpc::{ErrorCode, RpcError};

use crate::asking::{decision, Answer, Asking};

/// The tools that put something to the person. A plan is too, on the CLI's
/// own prompt (`asking_plan`).
const TO_THE_PERSON: [&str; 1] = ["AskUserQuestion"];

pub fn to_the_person(tool: &str) -> bool {
    TO_THE_PERSON.contains(&tool)
}

/// The picks waiting for a reply.
#[derive(Default)]
pub struct Picks {
    chosen: Mutex<HashMap<String, Vec<Vec<String>>>>,
    expected: Mutex<HashMap<String, usize>>,
    /// Why the person refused, for the agent to read.
    reasons: Mutex<HashMap<String, String>>,
    decisions: crate::asking_wait::Decisions,
}

impl Picks {
    pub(crate) fn opened(&self, question: &crate::asking::Question) {
        let Some(devpit_rpc::Touch::Ask { questions }) = question.touch.as_ref() else {
            return;
        };
        if let Ok(mut expected) = self.expected.lock() {
            expected.insert(question.id.clone(), questions.len());
        }
    }

    pub(crate) fn picked(&self, id: &str, chosen: Vec<Vec<String>>) {
        if let Ok(mut all) = self.chosen.lock() {
            all.insert(id.to_owned(), chosen);
        }
    }

    pub(crate) fn taken(&self, id: &str) -> Option<Vec<Vec<String>>> {
        self.chosen.lock().ok()?.remove(id)
    }

    /// Why the person refused, taken once.
    pub(crate) fn reason(&self, id: &str) -> Option<String> {
        self.reasons.lock().ok()?.remove(id)
    }
}

/// Keeps the picks, then allows the question; nothing is kept for one that
/// no longer waits.
pub(crate) fn pick(asking: &Asking, picks: &Picks, id: &str, chosen: Vec<Vec<String>>) -> bool {
    if picks
        .expected
        .lock()
        .ok()
        .and_then(|expected| expected.get(id).copied())
        .is_some_and(|expected| validate_choice_count(expected, &chosen).is_err())
    {
        return false;
    }
    picks.decisions.answer(id, || {
        picks.picked(id, chosen);
        let answered = asking.answer(id, Answer::Allow);
        if !answered {
            picks.taken(id);
        }
        answered
    })
}

/// The longest reason kept: the agent reads a sentence, not a document.
const MOST_REASON: usize = 2000;

/// Keeps why, then refuses the question; nothing is kept for one that no
/// longer waits.
pub(crate) fn refuse(asking: &Asking, picks: &Picks, id: &str, reason: &str) -> bool {
    let why: String = reason.trim().chars().take(MOST_REASON).collect();
    picks.decisions.answer(id, || {
        if let Ok(mut all) = picks.reasons.lock() {
            all.insert(id.to_owned(), why);
        }
        let answered = asking.answer(id, Answer::Deny);
        if let (false, Ok(mut all)) = (answered, picks.reasons.lock()) {
            all.remove(id);
        }
        answered
    })
}

/// Settles a question the CLI answered elsewhere, without replying on stdin.
pub(crate) fn cancel(asking: &Asking, picks: &Picks, id: &str) -> bool {
    picks
        .decisions
        .cancel(id, || asking.answer(id, Answer::Deny))
}

pub(crate) fn cancelled(picks: &Picks, id: &str) -> bool {
    let cancelled = picks.decisions.cancelled(id);
    if cancelled {
        if let Ok(mut expected) = picks.expected.lock() {
            expected.remove(id);
        }
    }
    cancelled
}

pub(crate) fn finished(picks: &Picks, id: &str) {
    picks.decisions.finished(id);
    if let Ok(mut expected) = picks.expected.lock() {
        expected.remove(id);
    }
}

/// A refusal in the person's words, which the CLI hands the agent as the
/// tool's result.
fn refused_because(why: &str) -> String {
    serde_json::json!({ "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": format!("Refused in devpit: {why}"),
    }})
    .to_string()
}

/// The tool's input as the CLI sent it, with the person's answers in: keyed
/// by each question's own words, several choices joined by ", ". Invalid input
/// or a mismatched number of choices is an error. The hook's reply and the
/// prompt's answer both build on it, so the two never drift apart.
pub(crate) fn answered_input(
    input: &str,
    chosen: Vec<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let mut input = serde_json::from_str::<serde_json::Value>(input)
        .map_err(|_| "that question's input is not valid JSON".to_owned())?;
    let asked: Vec<&str> = input
        .get("questions")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|one| one.get("question")?.as_str())
        .collect();
    validate_choice_count(asked.len(), &chosen)?;
    let answers: serde_json::Map<String, serde_json::Value> = asked
        .into_iter()
        .zip(chosen)
        .map(|(question, labels)| (question.to_owned(), labels.join(", ").into()))
        .collect();
    input["answers"] = answers.into();
    Ok(input)
}

pub(crate) fn validate_choice_count(expected: usize, chosen: &[Vec<String>]) -> Result<(), String> {
    if chosen.len() == expected {
        return Ok(());
    }
    Err(format!(
        "this question needs {expected} answers, but {} were sent",
        chosen.len()
    ))
}

/// The hook's reply. Allowed with picks, they go in as the tool's `answers`,
/// keyed by each question as it was asked, several choices joined by ", ".
pub fn reply_for(picks: &Picks, id: &str, input: &str, answer: Answer) -> String {
    let why = picks.reason(id);
    if let (Answer::Deny, Some(why)) = (answer, why.filter(|why| !why.is_empty())) {
        picks.taken(id);
        let reply = refused_because(&why);
        finished(picks, id);
        return reply;
    }
    // The picks go out with the answer, whichever it is: kept for no one.
    let chosen = picks.taken(id);
    let Some(updated) = chosen
        .filter(|_| answer == Answer::Allow)
        .and_then(|chosen| answered_input(input, chosen).ok())
    else {
        finished(picks, id);
        return decision(answer);
    };
    let reply = serde_json::json!({ "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "allow",
        "permissionDecisionReason": "answered in devpit",
        "updatedInput": updated,
    }})
    .to_string();
    finished(picks, id);
    reply
}

/// `permission.pick` — the person's choices to an `AskUserQuestion`: one
/// list per question, in the order asked.
#[tauri::command]
#[specta::specta]
pub async fn permission_pick(
    app: tauri::AppHandle,
    id: String,
    chosen: Vec<Vec<String>>,
) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        use tauri::Manager;
        if pick(&app.state::<Asking>(), &app.state::<Picks>(), &id, chosen) {
            return Ok(());
        }
        Err(RpcError::new(
            ErrorCode::NotFound,
            "that question is no longer waiting — it timed out, or the turn ended",
        ))
    })
    .await
}

/// `permission.refuse` — a refusal with the person's reason, which the agent
/// reads where it would have read the tool's result.
#[tauri::command]
#[specta::specta]
pub async fn permission_refuse(
    app: tauri::AppHandle,
    id: String,
    reason: String,
) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        use tauri::Manager;
        if refuse(&app.state::<Asking>(), &app.state::<Picks>(), &id, &reason) {
            return Ok(());
        }
        Err(RpcError::new(
            ErrorCode::NotFound,
            "that question is no longer waiting — it timed out, or the turn ended",
        ))
    })
    .await
}

#[cfg(test)]
#[path = "asking_picks_tests.rs"]
mod tests;
