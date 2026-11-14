//! The project agent's proposals as an orchestrator puts them to the person in
//! its chat. It lists them and asks; devpit reads the person's last message in
//! the orchestrator's own log (`devpit_agentcli::person`) and approves or
//! discards only when that message says so by itself. The orchestrator's own
//! word, another session's or a tool's never counts, and one message decides
//! once. Anything else leaves them waiting on the Project agent page.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use devpit_agentcli::person::{last_said, on_proposals, From, Verdict};
use devpit_rpc::Project;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::project_memory;

pub(crate) const METHOD: &str = "project_agent_proposals";

/// A message that decided: (profile, message id).
type Spent = (String, String);

fn spent() -> &'static Mutex<Vec<Spent>> {
    static SPENT: OnceLock<Mutex<Vec<Spent>>> = OnceLock::new();
    SPENT.get_or_init(Mutex::default)
}

/// `project_agent_proposals` for the orchestrator of `profile` standing in
/// `root`, over the projects it reaches.
pub(crate) fn respond(
    app: Option<&AppHandle>,
    profile: &str,
    root: &Path,
    projects: &[Project],
    params: &Value,
) -> Result<Value, String> {
    let verdict = match params.get("action").and_then(Value::as_str) {
        None | Some("list") => return Ok(listed(projects)),
        Some("approve") => Verdict::Approve,
        Some("discard") => Verdict::Discard,
        Some(other) => {
            return Err(format!(
                "`action` is list, approve or discard, not `{other}`"
            ))
        }
    };
    let ids: Vec<String> = params
        .get("ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let log = crate::draft_sending::log_tail(profile, root)?;
    let mut held = spent()
        .lock()
        .map_err(|_| "the record of decisions could not be read")?;
    let said = settle(
        projects,
        &log,
        verdict,
        &ids,
        &mut held,
        profile,
        &crate::project_agent::today(),
    )?;
    if let (Some(app), true) = (app, said["done"] == true) {
        for project in projects {
            let _ = app.emit("lessons:proposed", &project.id);
        }
    }
    Ok(said)
}

/// Every proposal waiting in `projects`, as the orchestrator puts them to the person.
pub(crate) fn listed(projects: &[Project]) -> Value {
    let proposals: Vec<Value> = projects
        .iter()
        .flat_map(|project| {
            project_memory::proposals(Path::new(&project.root_path))
                .iter()
                .map(|one| {
                    let shown = crate::project_memory_view::proposal_of(project, one);
                    json!({
                        "id": shown.id,
                        "project": shown.project,
                        "kind": kind_of(&shown.file),
                        "by": shown.by,
                        "summary": shown.summary,
                        "text": shown.text,
                        "replaces": shown.replaces,
                        "sources": one.source,
                        "day": shown.day,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    json!({
        "proposals": proposals,
        "note": "Nothing is approved yet. Mention them to the person in one line per project, in their language. Approve or discard only after they say so in this chat — `aprova`, `approve`, ✅ or ❌ alone — by calling again with that action and the ids: devpit reads their last message itself, and anything else leaves them on the project's Project agent page.",
    })
}

fn kind_of(file: &str) -> &'static str {
    match file {
        "facts.md" => "fact",
        "decisions.md" => "decision",
        _ => "lesson",
    }
}

/// Approves or discards the proposals named `ids`, when the person's last
/// message in `log` says `verdict` by itself and has not decided before.
pub(crate) fn settle(
    projects: &[Project],
    log: &str,
    verdict: Verdict,
    ids: &[String],
    held: &mut Vec<Spent>,
    profile: &str,
    day: &str,
) -> Result<Value, String> {
    let asked = match verdict {
        Verdict::Approve => "approve",
        Verdict::Discard => "discard",
    };
    if ids.is_empty() {
        return Err(format!(
            "which proposals? pass their `ids` from the list to {asked}"
        ));
    }
    let Some(said) = last_said(log) else {
        return Ok(waits(
            asked,
            "devpit found nothing the person said in this conversation",
        ));
    };
    if let From::Channel(_) = said.from {
        let why = format!(
            "the person last wrote from {}, where a word decides nothing",
            said.from.label()
        );
        return Ok(waits(asked, &why));
    }
    if on_proposals(&said.text) != Some(verdict) {
        let why = format!(
            "the person's last message («{}») does not say {asked} by itself",
            crate::draft_sending::short(&said.text)
        );
        return Ok(waits(asked, &why));
    }
    let key = (profile.to_owned(), said.id.clone());
    if held.contains(&key) {
        return Err("nothing changed: that message of the person's already decided; one message decides once".to_owned());
    }
    let (mut done, mut gone) = (Vec::new(), Vec::new());
    for id in ids {
        let found = projects.iter().find_map(|project| {
            let root = Path::new(&project.root_path);
            project_memory::proposals(root)
                .into_iter()
                .find(|one| project_memory::id_of(one) == *id)
                .map(|one| (root, one))
        });
        let Some((root, one)) = found else {
            gone.push(id.clone());
            continue;
        };
        let (_, text, replaces) = project_memory::aim(&one.text);
        let kept = (verdict == Verdict::Approve).then_some(text);
        match project_memory::decide(root, day, &one.source, text, replaces, kept) {
            Ok(()) => done.push(project_memory::plain(&one.text)),
            Err(_) => gone.push(id.clone()),
        }
    }
    held.push(key);
    Ok(json!({
        "done": true,
        "action": asked,
        "decided": done,
        "gone": gone,
        "from": said.from.label(),
    }))
}

fn waits(asked: &str, why: &str) -> Value {
    json!({
        "done": false,
        "note": format!("Nothing changed: {why}. The proposals wait on the project's Project agent page, where one click of the person's decides each. Tell them so; do not ask them to type it again."),
        "asked": asked,
    })
}

#[cfg(test)]
#[path = "memory_proposals_tests.rs"]
mod tests;
