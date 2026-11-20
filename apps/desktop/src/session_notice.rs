//! What an orchestrator's sessions did since its last turn, said at the top
//! of its next one.
//!
//! A session that finishes does not wake the orchestrator: a turn it did not
//! ask for spends the person's money and talks over them. devpit remembers
//! how the sessions stood when the orchestrator last spoke, and the next turn
//! the person starts begins with the difference — "since your last message:
//! api-worker finished its turn" — so the orchestrator is never the last to
//! know and never speaks unasked.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use devpit_core::i18n::t_for;
use devpit_rpc::{LiveSession, TurnFrom};

use crate::session_locale::SessionLocale;

/// How the sessions stood at a turn.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Seen {
    /// Milliseconds since the epoch.
    pub at: f64,
    pub names: BTreeSet<String>,
    pub busy: BTreeSet<String>,
    pub waiting: BTreeSet<String>,
}

impl Seen {
    pub(crate) fn of(sessions: &[LiveSession], at: f64) -> Seen {
        let named = |keep: &dyn Fn(&LiveSession) -> bool| {
            sessions
                .iter()
                .filter(|one| keep(one))
                .map(|one| one.name.clone())
                .collect()
        };
        Seen {
            at,
            names: named(&|_| true),
            busy: named(&|one| one.status == "busy"),
            waiting: named(&|one| one.waiting.is_some()),
        }
    }
}

fn seen() -> &'static Mutex<HashMap<String, Seen>> {
    static SEEN: OnceLock<Mutex<HashMap<String, Seen>>> = OnceLock::new();
    SEEN.get_or_init(Mutex::default)
}

/// What changed between `before` and `now`, in a line, or nothing.
pub(crate) fn notice(
    locale: SessionLocale,
    before: Option<&Seen>,
    now: &[LiveSession],
) -> Option<String> {
    // The first turn has nothing to compare with.
    let before = before?;
    let t = |key: &str, args: &[(&str, &str)]| t_for(locale.locale(), key, args);
    let mut said = Vec::new();
    for one in now {
        let new = !before.names.contains(&one.name);
        if one.waiting.is_some() && !before.waiting.contains(&one.name) {
            said.push(t("sinceNotice.waiting", &[("name", &one.name)]));
        } else if one.status == "idle"
            && (before.busy.contains(&one.name)
                || (new && one.since.is_some_and(|since| since > before.at)))
        {
            said.push(t("sinceNotice.finished", &[("name", &one.name)]));
        }
    }
    let here: BTreeSet<&String> = now.iter().map(|one| &one.name).collect();
    for gone in before.names.iter().filter(|name| !here.contains(name)) {
        said.push(t("sinceNotice.ended", &[("name", gone)]));
    }
    (!said.is_empty()).then(|| t("sinceNotice.line", &[("said", &said.join("; "))]))
}

/// A turn's project, its account, and where the person sent it from when
/// not the composer.
pub(crate) type Noticed = (String, String, Option<TurnFrom>);

/// `prompt`, with devpit's notices above it: where the person sent it from,
/// and in an orchestrator's chat what changed since the last turn, under the
/// person's preferences for a CLI no hook hands them to.
pub(crate) fn with_notice(
    locale: SessionLocale,
    noticed: &Noticed,
    prompt: &str,
    now_ms: f64,
) -> String {
    let (project_id, profile_id, from) = noticed;
    let root = crate::projects::store()
        .ok()
        .and_then(|store| crate::projects::locate(&store, project_id).ok())
        .map(|(_, root)| root);
    let handoff = root
        .as_deref()
        .and_then(|root| crate::orchestrator_handoff::take(root, profile_id));
    let since = since(locale, project_id, profile_id, now_ms);
    let devpits: Vec<String> = handoff.into_iter().chain(since).collect();
    let devpits = (!devpits.is_empty()).then(|| devpits.join("\n\n"));
    let said = notices(from.as_ref(), devpits.as_deref(), prompt);
    if let Some(root) = &root {
        crate::draft_sending::hear(profile_id, root, &said);
    }
    crate::orchestrator_brief::headed(project_id, profile_id, &said)
}

/// `prompt` under its notices, where it came from first: each its own
/// paragraph, which is how the person's words are told apart from them.
pub(crate) fn notices(from: Option<&TurnFrom>, since: Option<&str>, prompt: &str) -> String {
    let origin = from.and_then(|from| match from {
        TurnFrom::Remote => Some(devpit_agentcli::person::FROM_THE_REMOTE.to_owned()),
        TurnFrom::Channel { id } => devpit_agentcli::person::from_channel(id),
    });
    origin
        .as_deref()
        .into_iter()
        .chain(since)
        .chain([prompt])
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// What changed since the orchestrator's last turn, when the chat is an
/// orchestrator's and something did.
fn since(locale: SessionLocale, project_id: &str, profile_id: &str, now_ms: f64) -> Option<String> {
    let here = crate::projects::project_list_now()
        .ok()?
        .projects
        .into_iter()
        .find(|one| one.id == project_id && one.orchestrator.is_some())?;
    let live = crate::live_sessions::orchestrator_sessions_now(profile_id).ok()?;
    let sessions = crate::orchestrator_links::within(
        &here.id,
        &crate::orchestrator_links::linked(Path::new(&here.root_path)),
        live.sessions,
    );
    // The person speaks: the round the orchestrator reads after a compaction.
    crate::round_state::keep(Path::new(&here.root_path), &here.id, profile_id, &sessions);
    let mut held = seen().lock().ok()?;
    let said = notice(locale, held.get(project_id), &sessions);
    held.insert(project_id.to_owned(), Seen::of(&sessions, now_ms));
    said
}

#[cfg(test)]
#[path = "session_notice_tests.rs"]
mod tests;
