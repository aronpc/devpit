use devpit_core::locale::Locale;
use devpit_rpc::{LiveSession, PendingPrompt};
use serde_json::json;

use super::{notice, Seen};
use crate::session_locale::SessionLocale;

const EN: SessionLocale = SessionLocale::of(Locale::En);
const PT: SessionLocale = SessionLocale::of(Locale::PtBr);

fn session(name: &str, status: &str, since: f64, waiting: bool) -> LiveSession {
    let mut one: LiveSession = serde_json::from_value(json!({
        "name": name, "pid": 1, "job": null, "status": status, "kind": "interactive", "cwd": "/w",
        "projectId": "prj_a", "projectName": "a", "cardId": null, "since": since, "inDevpit": true,
        "waiting": null, "pane": null, "sessionId": null, "step": null, "draft": null,
        "remote": { "on": false, "url": null },
    }))
    .expect("a session");
    if waiting {
        one.waiting = Some(PendingPrompt {
            question: "Allow?".into(),
            options: Vec::new(),
            cursor: 0,
        });
    }
    one
}

#[test]
fn the_first_turn_says_nothing_and_a_quiet_one_nothing_new() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let now = [session("api", "busy", 10.0, false)];
    assert_eq!(notice(EN, None, &now), None);
    assert_eq!(notice(EN, Some(&Seen::of(&now, 20.0)), &now), None);
}

#[test]
fn what_finished_asks_or_ended_since_the_last_turn_is_said() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let before = Seen::of(
        &[
            session("api", "busy", 10.0, false),
            session("web", "busy", 10.0, false),
            session("old", "idle", 5.0, false),
        ],
        20.0,
    );
    let now = [
        session("api", "idle", 30.0, false),
        session("web", "busy", 10.0, true),
        session("quick", "idle", 25.0, false),
        session("stale", "idle", 15.0, false),
    ];
    assert_eq!(
        notice(EN, Some(&before), &now).as_deref(),
        Some("[devpit, not the person] Since your last message: api finished its turn; web is waiting on a question; quick finished its turn; old ended.")
    );
}

/// The line the orchestrator reads is in the person's language too.
#[test]
fn the_notice_is_said_in_portuguese_when_the_person_speaks_it() {
    let _turn = crate::locale_turn::settled(Locale::PtBr);
    let before = Seen::of(
        &[
            session("api", "busy", 10.0, false),
            session("old", "idle", 5.0, false),
        ],
        20.0,
    );
    let now = [session("api", "idle", 30.0, false)];
    assert_eq!(
        notice(PT, Some(&before), &now).as_deref(),
        Some("[devpit, não a pessoa] Desde a sua última mensagem: api terminou o turno; old terminou.")
    );
}

/// The line is said in the language of the session it heads, and a switch in
/// Settings after that session was born does not reach it.
#[test]
fn the_notice_keeps_the_sessions_language_and_not_the_processs() {
    let _turn = crate::locale_turn::settled(Locale::PtBr);
    let before = Seen::of(&[session("api", "busy", 10.0, false)], 20.0);
    let now = [session("api", "idle", 30.0, false)];
    assert_eq!(
        notice(EN, Some(&before), &now).as_deref(),
        Some("[devpit, not the person] Since your last message: api finished its turn.")
    );
}
