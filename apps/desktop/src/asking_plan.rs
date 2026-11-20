//! A plan to approve, put to the person on the CLI's own permission prompt.
//!
//! Not on the hook, like the other questions: only the prompt's answer can
//! take the session out of plan mode, so an approved plan went on planning.
//! A question to the person rides the same prompt — it is where a remote
//! control answers from, so it is held for whoever is there. A CLI with no
//! hook asks every question on its prompt, and they come here too.

use std::sync::Arc;

use devpit_agentcli::control::{Asker, Control, Prompt};
use serde_json::{json, Value};

use crate::asking::{Answer, Asking, Question, ASKING_MODE};
use crate::asking_picks::{answered_input, to_the_person, Picks};
use crate::asking_waiting::Waiting;

const PLAN: &str = "ExitPlanMode";

/// What the agent reads when the person refused without a word.
const NOT_YET: &str = "The person did not approve the plan. Keep planning.";

/// Whether this prompt is answered here rather than refused: a plan, whose
/// only way out of plan mode is its own prompt; a question to the person;
/// and anything when its CLI asks every question here.
fn takes(tool: &str, asks: bool) -> bool {
    tool == PLAN || to_the_person(tool) || asks
}

/// Takes a turn's plan prompts and its questions to the person to the
/// window, and every prompt when its CLI `asks` there; any other prompt is
/// refused as before.
pub(crate) fn plans(app: &tauri::AppHandle, asks: bool, conversation_id: &str) -> Asker {
    let (app, conversation) = (app.clone(), conversation_id.to_owned());
    Arc::new(move |prompt: Prompt, control: &Control| {
        if !takes(&prompt.tool, asks) {
            return false;
        }
        let (app, control, conversation) = (app.clone(), control.clone(), conversation.clone());
        std::thread::spawn(move || put(&app, &control, prompt, conversation));
        true
    })
}

/// Waits for the person, as a question the hook holds does, then answers on stdin.
fn put(app: &tauri::AppHandle, control: &Control, prompt: Prompt, conversation: String) {
    use tauri::{Emitter, Manager};
    let question = Question {
        id: prompt.request_id.clone(),
        session_id: prompt.session_id.clone().unwrap_or_default(),
        tool: prompt.tool.clone(),
        input: prompt.input.to_string(),
        cwd: String::new(),
        touch: devpit_agentcli::touch_of(&prompt.tool, &prompt.input),
        conversation_id: Some(conversation),
    };
    let plan = prompt.tool == PLAN;
    let person = to_the_person(&prompt.tool);
    if already_allowed(&app.state::<crate::asking_kept::Kept>(), &question) {
        control.answer(&prompt.request_id, json!({ "behavior": "allow" }));
        return;
    }
    let asking = app.state::<Asking>();
    let hear = asking.opened(&question.id);
    held_for_a_late_window(
        &app.state::<Waiting>(),
        &question,
        |question| {
            let _ = app.emit("permission:asked", question);
            crate::island_notify::asked(app, &question.tool, &question.input);
        },
        || asking.wait_for(&question, hear, &app.state::<crate::asking_wait::Ends>()),
        |answer| {
            if plan && answer == Answer::Allow {
                // Out of plan mode, the session asks before what it changes.
                asking.ask_from_now(&question.session_id, true);
            }
            let picks = app.state::<Picks>();
            let said = if plan {
                plan_reply(&picks, &question.id, prompt.input, answer)
            } else if person {
                person_reply(&picks, &question.id, &question.input, answer)
            } else {
                approval_reply(&picks, &question.id, answer)
            };
            control.answer(&prompt.request_id, said);
        },
        |id| {
            let _ = app.emit("permission:settled", id);
        },
    );
}

/// One question on the CLI's own prompt, kept for the Waiting the hook holds
/// its own in: in before the window hears it asked, so a chat that opens or
/// reloads then — the event only reaches windows already listening — still
/// draws it, and out before the window hears it settled, whichever way it
/// settled.
fn held_for_a_late_window(
    waiting: &Waiting,
    question: &Question,
    asked: impl FnOnce(&Question),
    waited: impl FnOnce() -> Answer,
    answered: impl FnOnce(Answer),
    settled: impl FnOnce(&str),
) {
    waiting.opened(question);
    asked(question);
    let answer = waited();
    answered(answer);
    waiting.settled(&question.id);
    settled(&question.id);
}

/// Allowed for the rest of this conversation, as the hook does: never a plan,
/// never a question to the person, never a request for more reach than the
/// call, and never one whose CLI says it asks every time (`"always": false`).
pub(crate) fn already_allowed(kept: &crate::asking_kept::Kept, question: &Question) -> bool {
    let never_kept = question.tool == PLAN || to_the_person(&question.tool);
    let each_time = serde_json::from_str::<Value>(&question.input)
        .is_ok_and(|input| input.get("grants").is_some() || input["always"] == false);
    !never_kept && !each_time && kept.covers(&question.session_id, &question.tool, &question.input)
}

/// Any other prompt's answer: allowed, or refused with why.
pub(crate) fn approval_reply(picks: &Picks, id: &str, answer: Answer) -> Value {
    if answer == Answer::Allow {
        return json!({ "behavior": "allow" });
    }
    let why = picks.reason(id).filter(|why| !why.is_empty());
    json!({ "behavior": "deny", "message": why.map_or_else(|| "Refused in devpit".to_owned(), |why| format!("Refused in devpit: {why}")) })
}

/// The prompt's answer to a question put to the person: allowed, their
/// picks in as the tool's `answers`; or refused, with why. An allow that
/// chose nothing still answers in the shape the CLI's prompt reads, every
/// question in it answered with nothing.
pub(crate) fn person_reply(picks: &Picks, id: &str, input: &str, answer: Answer) -> Value {
    let why = picks.reason(id).filter(|why| !why.is_empty());
    if answer == Answer::Deny {
        picks.taken(id);
        let message = why.map_or_else(
            || "Refused in devpit".to_owned(),
            |why| format!("Refused in devpit: {why}"),
        );
        return json!({ "behavior": "deny", "message": message });
    }
    let chosen = picks.taken(id).unwrap_or_default();
    let answered = answered_input(input, chosen).unwrap_or_else(|| json!({}));
    json!({ "behavior": "allow", "updatedInput": answered })
}

/// The prompt's answer: the plan approved as written or as the person edited
/// it, leaving plan mode for the one that asks; or refused, with why.
pub(crate) fn plan_reply(picks: &Picks, id: &str, mut input: Value, answer: Answer) -> Value {
    let why = picks.reason(id).filter(|why| !why.is_empty());
    let edited = picks
        .taken(id)
        .and_then(|chosen| chosen.into_iter().next()?.into_iter().next());
    if answer == Answer::Deny {
        let message = why.map_or_else(
            || NOT_YET.to_owned(),
            |why| format!("Refused in devpit: {why}"),
        );
        return json!({ "behavior": "deny", "message": message });
    }
    if let Some(plan) = edited {
        input["plan"] = plan.into();
    }
    json!({
        "behavior": "allow",
        "updatedInput": input,
        "updatedPermissions": [{ "type": "setMode", "mode": ASKING_MODE, "destination": "session" }],
    })
}

#[cfg(test)]
#[path = "asking_plan_tests.rs"]
mod tests;
