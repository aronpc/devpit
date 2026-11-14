//! A plan to approve, put to the person on the CLI's own permission prompt.
//!
//! Not on the hook, like the other questions: only the prompt's answer can
//! take the session out of plan mode, so an approved plan went on planning.
//! A CLI with no hook asks every question on its prompt, and they come here too.

use std::sync::Arc;

use devpit_agentcli::control::{Asker, Control, Prompt};
use serde_json::{json, Value};

use crate::asking::{Answer, Asking, Question, ASKING_MODE};
use crate::asking_picks::Picks;

const PLAN: &str = "ExitPlanMode";

/// What the agent reads when the person refused without a word.
const NOT_YET: &str = "The person did not approve the plan. Keep planning.";

/// Takes a turn's plan prompts to the window, and every prompt when its CLI
/// `asks` there; any other prompt is refused as before.
pub(crate) fn plans(app: &tauri::AppHandle, asks: bool) -> Asker {
    let app = app.clone();
    Arc::new(move |prompt: Prompt, control: &Control| {
        if prompt.tool != PLAN && !asks {
            return false;
        }
        let (app, control) = (app.clone(), control.clone());
        std::thread::spawn(move || put(&app, &control, prompt));
        true
    })
}

/// Waits for the person, as a question the hook holds does, then answers on stdin.
fn put(app: &tauri::AppHandle, control: &Control, prompt: Prompt) {
    use tauri::{Emitter, Manager};
    let question = Question {
        id: prompt.request_id.clone(),
        session_id: prompt.session_id.clone().unwrap_or_default(),
        tool: prompt.tool.clone(),
        input: prompt.input.to_string(),
        cwd: String::new(),
        touch: devpit_agentcli::touch_of(&prompt.tool, &prompt.input),
    };
    let plan = prompt.tool == PLAN;
    // Already allowed for the rest of this conversation, as the hook does.
    let kept = app.state::<crate::asking_kept::Kept>().covers(
        &question.session_id,
        &question.tool,
        &question.input,
    );
    if !plan && kept {
        control.answer(&prompt.request_id, json!({ "behavior": "allow" }));
        return;
    }
    let asking = app.state::<Asking>();
    let hear = asking.opened(&question.id);
    let _ = app.emit("permission:asked", &question);
    crate::island_notify::asked(app, &question.tool, &question.input);
    let answer = asking.wait(&question.id, hear);
    if plan && answer == Answer::Allow {
        // Out of plan mode, the session asks before what it changes.
        asking.ask_from_now(&question.session_id, true);
    }
    let picks = app.state::<Picks>();
    let said = match plan {
        true => plan_reply(&picks, &question.id, prompt.input, answer),
        false => approval_reply(&picks, &question.id, answer),
    };
    control.answer(&prompt.request_id, said);
    let _ = app.emit("permission:settled", &question.id);
}

/// Any other prompt's answer: allowed, or refused with why.
pub(crate) fn approval_reply(picks: &Picks, id: &str, answer: Answer) -> Value {
    if answer == Answer::Allow {
        return json!({ "behavior": "allow" });
    }
    let why = picks.reason(id).filter(|why| !why.is_empty());
    json!({ "behavior": "deny", "message": why.map_or_else(|| "Refused in devpit".to_owned(), |why| format!("Refused in devpit: {why}")) })
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
