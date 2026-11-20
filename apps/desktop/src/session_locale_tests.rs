//! The language of one session, held from its birth: what it says, where it
//! rides, and the two sessions that must not hear each other's.

use std::sync::{Arc, Barrier};

use devpit_core::locale::Locale;

use super::SessionLocale;

/// The assignment is the whole mechanism for a typed line: one variable, one
/// value, in front of the command, as a profile's own variables already are.
#[test]
fn a_session_carries_its_language_as_an_assignment_in_front() {
    assert_eq!(
        SessionLocale::of(Locale::PtBr).assignment(),
        "DEVPIT_LOCALE='pt-BR' "
    );
    assert_eq!(
        SessionLocale::of(Locale::En).assignment(),
        "DEVPIT_LOCALE='en' "
    );
}

/// The variable of the assignment is the variable, name for name: two
/// spellings would leave every session answering English.
#[test]
fn the_assignment_names_the_variable_the_server_reads() {
    let (name, value) = SessionLocale::of(Locale::PtBr).var();
    assert_eq!(name, "DEVPIT_LOCALE");
    assert_eq!(value, "pt-BR");
}

/// A session already running when the person switches the language in
/// Settings keeps the one it was born in: its MCP server keeps answering
/// `tools/list` as it began, and only the sessions born after switch.
#[test]
fn a_live_session_keeps_the_language_it_was_born_in() {
    let _turn = crate::locale_turn::settled(Locale::PtBr);
    let born = SessionLocale::capture();
    let said = born.assignment();
    Locale::En.set();
    assert_eq!(born.assignment(), said);
    Locale::En.set();
}

/// Two sessions born one after another under different settings — the person
/// switched between the births — each carry their own: the barrier holds the
/// first's birth before the second's, as a click that starts one and then
/// the other would space them.
#[test]
fn two_sessions_born_apart_each_carry_their_own_language() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let apart = Arc::new(Barrier::new(2));
    let together = Arc::new(Barrier::new(2));
    let first = {
        let (apart, together) = (apart.clone(), together.clone());
        std::thread::spawn(move || {
            Locale::PtBr.set();
            let born = SessionLocale::capture();
            let said = born.assignment();
            apart.wait();
            together.wait();
            (born, said)
        })
    };
    let second = {
        let (apart, together) = (apart.clone(), together.clone());
        std::thread::spawn(move || {
            apart.wait();
            Locale::En.set();
            let born = SessionLocale::capture();
            let said = born.assignment();
            together.wait();
            (born, said)
        })
    };
    let ((first, first_said), (second, second_said)) =
        (first.join().expect("first"), second.join().expect("second"));
    assert_eq!(first.assignment(), first_said);
    assert_eq!(second.assignment(), second_said);
    assert_ne!(first_said, second_said);
    Locale::En.set();
}

fn profile() -> devpit_rpc::Profile {
    devpit_rpc::Profile {
        id: "codex".to_owned(),
        label: "Codex".to_owned(),
        command: "codex".to_owned(),
        driver: "codex".to_owned(),
        path: None,
        reach: devpit_rpc::Reach::Runnable,
        base: "codex".to_owned(),
        args: Vec::new(),
        env: Vec::new(),
        mine: true,
        models: Vec::new(),
        own_models: Vec::new(),
        offered: Vec::new(),
        efforts: Vec::new(),
        effort_default: None,
        metered: true,
        enabled: true,
    }
}

/// What one turn of a conversation is started with: its variable, its MCP
/// file and the notice heading the person's words.
fn turn_in(locale: SessionLocale, id: &str, root: &std::path::Path) -> (String, String, String) {
    let env = crate::chat_turn::env_of(locale, &profile(), id);
    let var = env
        .iter()
        .rev()
        .find(|(name, _)| name == super::ENV)
        .map(|(_, value)| value.clone())
        .expect("the variable");
    let mcp = crate::agent_reach::chat_mcp_in(
        locale,
        root,
        std::path::Path::new("/opt/devpit"),
        "codex",
        "codex",
    )
    .expect("an MCP file");
    let before = crate::session_notice::Seen::of(&[session("api", "busy")], 1.0);
    let notice = crate::session_notice::notice(locale, Some(&before), &[session("api", "idle")])
        .expect("a notice");
    (var, mcp, notice)
}

fn session(name: &str, status: &str) -> devpit_rpc::LiveSession {
    serde_json::from_value(serde_json::json!({
        "name": name, "pid": 1, "job": null, "status": status, "kind": "interactive", "cwd": "/w",
        "projectId": "prj_a", "projectName": "a", "cardId": null, "since": 0.5, "inDevpit": true,
        "waiting": null, "pane": null, "sessionId": null, "step": null, "draft": null,
        "remote": { "on": false, "url": null },
    }))
    .expect("a session")
}

/// A conversation born in English keeps English through its second turn and
/// the compaction a timer runs, though Settings switched to Portuguese between
/// them; one born after the switch is Portuguese.
#[test]
fn a_conversation_born_in_english_stays_english_through_its_turns_and_compaction() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let home = tempfile::tempdir().expect("tempdir");
    let id = format!("conv_{}", ulid::Ulid::generate());
    let first = turn_in(super::of_conversation(&id, true), &id, home.path());
    Locale::PtBr.set();
    let second = turn_in(super::of_conversation(&id, false), &id, home.path());
    let compacted = turn_in(super::of_conversation(&id, false), &id, home.path());
    assert_eq!(first.0, "en");
    assert!(first.1.ends_with("-en.json"), "{}", first.1);
    assert!(first.2.contains("finished its turn"), "{}", first.2);
    assert_eq!(second, first);
    assert_eq!(compacted, first);
    let later = format!("conv_{}", ulid::Ulid::generate());
    let born_after = super::of_conversation(&later, true);
    assert_eq!(born_after.locale(), Locale::PtBr);
}
