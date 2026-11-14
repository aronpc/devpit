//! An orchestrator switched to another provider: its folder, notes and links
//! stay where they are, and its first turn on the new CLI starts from a short
//! handoff written from its own notes, since no conversation crosses CLIs.

use std::path::Path;

use devpit_core::Store;
use serde_json::{json, Value};

/// Its key among the orchestrator's settings.
const KEY: &str = "handoff";
/// The most of `context/now.md` a handoff carries.
const MOST_NOW: usize = 3000;
/// The latest lines of `context/sessions.md` a handoff carries.
const LAST_SESSIONS: usize = 12;

/// Notes that the orchestrator in `folder` now speaks as `to`, having spoken
/// as `from`: its next turn as `to` is handed over.
pub(crate) fn switched(store: &Store, folder: &Path, from: &str, to: &str) {
    if from != to {
        let _ = crate::orchestrator_links::set_in(
            store,
            folder,
            KEY,
            json!({ "from": from, "to": to }),
        );
    }
}

/// The handoff for the orchestrator in `root`'s turn as `profile`, once:
/// nothing when it was not switched to `profile`.
pub(crate) fn take(root: &Path, profile: &str) -> Option<String> {
    let store = crate::projects::store().ok()?;
    let waiting = store.orchestrator_settings(root).ok()?[KEY].clone();
    if waiting["to"].as_str() != Some(profile) {
        return None;
    }
    let from = waiting["from"].as_str().unwrap_or_default().to_owned();
    let _ = crate::orchestrator_links::set_in(&store, root, KEY, Value::Null);
    let read =
        |file: &str| std::fs::read_to_string(root.join("context").join(file)).unwrap_or_default();
    Some(handoff(
        &from,
        profile,
        &read("now.md"),
        &read("sessions.md"),
    ))
}

/// The handoff as one notice: one paragraph, so the person's words under it
/// are still told apart from it.
pub(crate) fn handoff(from: &str, to: &str, now: &str, sessions: &str) -> String {
    let lines = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect()
    };
    let mut now: String = lines(now).join("\n");
    if now.chars().count() > MOST_NOW {
        now = now.chars().take(MOST_NOW).collect::<String>() + "…";
    }
    let sessions = lines(sessions);
    let latest = sessions[sessions.len().saturating_sub(LAST_SESSIONS)..].join("\n");
    let mut said = format!(
        "[devpit, not the person] This orchestrator was switched from {from} to {to}: this is its first conversation on {to}, and the earlier ones stay with {from}. Its folder, notes, linked projects and settings are as they were; read context/now.md, context/sessions.md and context/preferences.md before going on."
    );
    if !now.is_empty() {
        said.push_str("\nWhere the round stood, from context/now.md:\n");
        said.push_str(&now);
    }
    if !latest.is_empty() {
        said.push_str("\nThe latest of context/sessions.md:\n");
        said.push_str(&latest);
    }
    said
}

#[cfg(test)]
#[path = "orchestrator_handoff_tests.rs"]
mod tests;
