//! `start` — an orchestrator hands a card's work to a session of its own
//! account.
//!
//! The session runs in the card's terminal tab, in the card's own checkout:
//! the person watches it and types into it like any terminal, the card's
//! Sessions say it is there, and it is named so the orchestrator can message
//! it and ask to hear when it goes idle. Nothing starts out of sight — which is
//! what an orchestrator is allowed on the condition of. It used to start in the
//! background, under the CLI's own supervisor, where nobody could watch it.

use devpit_rpc::Board;
use serde_json::{json, Value};

/// The longest first message handed to a session. A brief, not a document:
/// the card holds the rest.
const LONGEST_PROMPT: usize = 8000;

/// Starts the session and links it to the card. Answers with the name to
/// message it by.
pub(crate) fn hand(
    app: &tauri::AppHandle,
    board: &Board,
    profile_id: &str,
    card_id: &str,
    prompt: &str,
    wanted: crate::opening::Wanted<'_>,
    in_project: Option<&std::path::Path>,
) -> Result<Value, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > LONGEST_PROMPT {
        return Err(format!(
            "a session is handed between 1 and {LONGEST_PROMPT} characters of work"
        ));
    }
    let card = board
        .cards
        .iter()
        .find(|card| card.id == card_id)
        .ok_or("that card is not on this project's board")?;
    let name = unused(
        session_name(wanted.name.unwrap_or(&card.title), card_id),
        &live_names(profile_id),
    );

    let store = crate::projects::store().map_err(|err| err.message)?;
    // The card's own checkout: two sessions at work in one project do not
    // write over each other, and the card's Changes show what this one did.
    // The project's folder only when the person chose it.
    let cwd = match in_project {
        Some(root) => root.to_path_buf(),
        None => crate::checkout::checkout_of(&store, card_id, |_| {})?,
    };
    // The card's own tab, which is how the card finds its sessions; a new one
    // when that tab is already open, rather than typing over what runs there.
    let card_tab = crate::sessions::tab_for_card(card_id);
    let taken = store
        .pane_layout(&board.project_id, &card_tab)
        .map_err(|err| err.to_string())?
        .is_some();
    let tab_id = tab_to_open(card_tab, taken);
    // The last handoff on the card, so a new session starts where one stopped.
    let comments = store.comments(card_id).map_err(|err| err.to_string())?;
    let prompt = crate::handoff::briefed(prompt, crate::handoff::latest(&comments));
    let runs_as = wanted.profile.unwrap_or(profile_id);
    let line = crate::opening::launched(runs_as, &board.project_id, &name, &prompt, wanted.model)
        .map_err(|err| err.message)?;
    let pane_id =
        crate::opening::typed_in(app, &board.project_id, &tab_id, &cwd, &line, Some(runs_as))
            .map_err(|err| err.message)?;
    let project_name = crate::projects::project_list_unread_now()
        .ok()
        .and_then(|list| {
            list.projects
                .into_iter()
                .find(|one| one.id == board.project_id)
                .map(|one| one.name)
        });
    let kept = crate::opening::kept(
        profile_id,
        runs_as,
        crate::pane_sessions::Kept {
            profile: profile_id.to_owned(),
            name: name.clone(),
            project_id: board.project_id.clone(),
            project_name: project_name.clone(),
            card_id: Some(card_id.to_owned()),
            cwd: cwd.display().to_string(),
            pane_id: pane_id.clone(),
        },
    );
    // Looked for from now: before the CLI lists it, it may already be stopped
    // on a question only the person can answer.
    if !kept {
        crate::starting::began(crate::starting::Starting {
            profile: profile_id.to_owned(),
            name: name.clone(),
            project_id: board.project_id.clone(),
            project_name,
            cwd: cwd.display().to_string(),
            pane_id: pane_id.clone(),
            at: std::time::Instant::now(),
        });
    }
    let _ = tauri::Emitter::emit(
        app,
        crate::opening::TAB_OPENED,
        json!({ "projectId": board.project_id, "tabId": tab_id, "paneId": pane_id }),
    );
    // Work on it starts now: a waiting card goes to work in progress.
    if let Some(to) = crate::card_follows::when_started(board, card_id) {
        let _ = crate::agent_api::moved(Some(app), board, card_id, &to.id);
    }

    Ok(json!({
        "name": name,
        "kept": kept,
        "cardId": card_id,
        "cwd": cwd.display().to_string(),
        "next": "It runs in a terminal tab of the project, where the person can watch it. Message it by this name with SendMessage once it is up, and pass notify_when_idle to hear when it is done.",
    }))
}

/// A name no session alive goes by: the same card handed twice is two
/// sessions, and one name for both sends a reply, a screen or a stop to
/// whichever the CLI lists first.
pub(crate) fn unused(name: String, taken: &[String]) -> String {
    if !taken.contains(&name) {
        return name;
    }
    (2..)
        .map(|n| format!("{name}-{n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or(name)
}

/// The names the account's live sessions go by.
pub(crate) fn live_names(profile_id: &str) -> Vec<String> {
    crate::live_sessions::orchestrator_sessions_now(profile_id)
        .map(|live| live.sessions.into_iter().map(|one| one.name).collect())
        .unwrap_or_default()
}

/// The card's own tab, unless something already has it open.
pub(crate) fn tab_to_open(card_tab: String, taken: bool) -> String {
    if taken {
        crate::opening::fresh_tab()
    } else {
        card_tab
    }
}

/// A name another session can address: the card's words, short, plain, and
/// the end of its id so two cards of one title are two names.
pub(crate) fn session_name(title: &str, card_id: &str) -> String {
    let mut out = String::new();
    for ch in title.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let words: String = out.trim_matches('-').chars().take(28).collect();
    let words = words.trim_end_matches('-');
    let tail: String = card_id
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let tail = tail.to_ascii_lowercase();
    if words.is_empty() {
        format!("card-{tail}")
    } else {
        format!("{words}-{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::{session_name, tab_to_open, unused};

    #[test]
    fn a_name_already_alive_is_numbered_until_it_is_free() {
        let taken = ["fix-9xyz".to_owned(), "fix-9xyz-2".to_owned()];
        assert_eq!(unused("fix-9xyz".to_owned(), &taken), "fix-9xyz-3");
        assert_eq!(unused("other".to_owned(), &taken), "other");
    }

    #[test]
    fn a_handed_session_takes_the_card_tab_and_never_types_over_one_open() {
        assert_eq!(tab_to_open("tab_card_x".to_owned(), false), "tab_card_x");
        let fresh = tab_to_open("tab_card_x".to_owned(), true);
        assert_ne!(fresh, "tab_card_x");
        assert!(fresh.starts_with("tab_"));
    }

    #[test]
    fn a_session_is_named_after_its_card_and_told_apart_by_its_id() {
        assert_eq!(
            session_name("Fix the login timeout!", "card_01ABCD9XYZ"),
            "fix-the-login-timeout-9xyz"
        );
        assert_eq!(session_name("   ", "card_01ABCD9XYZ"), "card-9xyz");
        assert_ne!(
            session_name("Same", "card_1111"),
            session_name("Same", "card_2222")
        );
    }
}
