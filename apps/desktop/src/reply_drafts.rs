//! Replies an orchestrator drafts for the person to send as theirs.
//!
//! A message from the orchestrator approves nothing in another session — the
//! CLI treats it as coming from the orchestrator, and rightly. What the person
//! says in the orchestrator's chat used to stop there: to let a session go on,
//! they had to find its tab and type it again. So the orchestrator writes the
//! words and the window shows them beside the session; the person sends them,
//! edits them or drops them, and only their click types anything.
//!
//! Held in memory, one per session: a draft is for now, and one that outlived
//! the app would offer words about a moment that has passed.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The most a draft may be, in characters: what a reply may be.
pub(crate) const LONGEST: usize = 4000;

type Drafts = HashMap<(String, String), String>;

fn drafts() -> &'static Mutex<Drafts> {
    static DRAFTS: OnceLock<Mutex<Drafts>> = OnceLock::new();
    DRAFTS.get_or_init(Mutex::default)
}

/// Keeps `text` as the reply drafted for the session `name` of `profile`,
/// in place of any before it, by the orchestrator of `project`. In a turn
/// from a channel its button goes under the answer, and the notice that
/// would carry the same button does not; answers whether it went there.
pub(crate) fn draft(project: &str, profile: &str, name: &str, text: &str) -> Result<bool, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > LONGEST {
        return Err(format!("a draft is between 1 and {LONGEST} characters"));
    }
    drafts()
        .lock()
        .map_err(|_| "the drafts could not be read".to_owned())?
        .insert((profile.to_owned(), name.to_owned()), text.to_owned());
    crate::tray::refresh_soon();
    let send = crate::channels::Action::SendDraft {
        profile: profile.to_owned(),
        session: name.to_owned(),
    };
    if crate::channel_talk::offer(project, send) {
        return Ok(true);
    }
    crate::channels::emit(crate::channels::Told {
        event: devpit_rpc::ChannelEvent::DraftReady,
        line: format!("A draft for {name}: «{text}»"),
        bare: "A draft waits to be sent.".to_owned(),
        actions: vec![crate::channels::Action::SendDraft {
            profile: profile.to_owned(),
            session: name.to_owned(),
        }],
        asking: None,
        about: Some(crate::channel_escalation::About::Draft {
            profile: profile.to_owned(),
            session: name.to_owned(),
        }),
    });
    Ok(false)
}

/// The draft for `name` moved from `from`'s orchestrator to `to`'s, the same
/// orchestrator after a switch of profile.
pub(crate) fn carried(from: &str, to: &str, name: &str) {
    if let Ok(mut held) = drafts().lock() {
        if let Some(text) = held.remove(&(from.to_owned(), name.to_owned())) {
            held.insert((to.to_owned(), name.to_owned()), text);
        }
    }
}

/// The reply drafted for a session, if one is waiting to be sent.
pub(crate) fn drafted(profile: &str, name: &str) -> Option<String> {
    drafts()
        .lock()
        .ok()?
        .get(&(profile.to_owned(), name.to_owned()))
        .cloned()
}

/// Every draft waiting, as (profile, session, text), in a steady order.
pub(crate) fn all() -> Vec<(String, String, String)> {
    let Ok(held) = drafts().lock() else {
        return Vec::new();
    };
    let mut every: Vec<_> = held
        .iter()
        .map(|((profile, name), text)| (profile.clone(), name.clone(), text.clone()))
        .collect();
    every.sort();
    every
}

/// The line a sent draft leaves in its orchestrator's `sessions` log: who it
/// went to, from where, and the words that went — edited or as drafted.
pub(crate) fn sent_entry(name: &str, drafted: &str, sent: &str, from: &str) -> String {
    let how = if drafted.trim() == sent.trim() {
        "as drafted"
    } else {
        "edited"
    };
    format!(
        "The person sent the draft for {name} from {from}, {how}: {}",
        sent.trim()
    )
}

/// Writes [`sent_entry`] into the log of the orchestrator speaking as `profile`.
pub(crate) fn kept_sent(profile: &str, name: &str, drafted: &str, sent: &str, from: &str) {
    let Ok(projects) = crate::live_sessions::recent_projects() else {
        return;
    };
    let Some(folder) = projects
        .iter()
        .find(|one| one.orchestrator.as_deref() == Some(profile))
        .map(|one| std::path::PathBuf::from(&one.root_path))
    else {
        return;
    };
    let entry = sent_entry(name, drafted, sent, from);
    if let Err(why) = crate::orchestrator_notes::note(
        &folder,
        "sessions",
        &entry,
        crate::orchestrator_notes::Section::Log,
    ) {
        devpit_core::reports::background("draft record", &why);
    }
}

/// The draft waiting for the live session whose id is `session_id`, as
/// (profile, name, text): the island knows a session only by its id.
pub(crate) fn for_session(session_id: &str) -> Option<(String, String, String)> {
    all().into_iter().find(|(profile, name, _)| {
        crate::live_sessions::orchestrator_sessions_now(profile)
            .map(|live| {
                live.sessions
                    .iter()
                    .any(|one| &one.name == name && one.session_id.as_deref() == Some(session_id))
            })
            .unwrap_or(false)
    })
}

/// `island.draft` — the reply drafted for a session the island shows, if any.
#[tauri::command]
#[specta::specta]
pub async fn island_draft(session_id: String) -> Result<Option<String>, devpit_rpc::RpcError> {
    crate::off_main::blocking(move || Ok(for_session(&session_id).map(|(_, _, text)| text))).await
}

/// `island.draft_send` — sends that draft into the session's terminal, as the person.
#[tauri::command]
#[specta::specta]
pub async fn island_draft_send(session_id: String) -> Result<(), devpit_rpc::RpcError> {
    crate::off_main::blocking(move || {
        let (profile, name, text) = for_session(&session_id).ok_or_else(|| {
            devpit_rpc::RpcError::new(
                devpit_rpc::ErrorCode::NotFound,
                "that draft is no longer waiting",
            )
        })?;
        crate::live_sessions::reply_now(&profile, &name, &text, Some("the island"))
    })
    .await
}

/// Lets go of a session's draft: sent, or dropped by the person.
pub(crate) fn forget(profile: &str, name: &str) {
    if let Ok(mut held) = drafts().lock() {
        held.remove(&(profile.to_owned(), name.to_owned()));
    }
    crate::tray::refresh_soon();
}

/// `orchestrator.draft_drop` — the person drops a draft without sending it.
#[tauri::command]
#[specta::specta]
pub fn orchestrator_draft_drop(profile_id: String, name: String) {
    forget(&profile_id, &name);
}

#[cfg(test)]
#[path = "reply_drafts_tests.rs"]
mod tests;
