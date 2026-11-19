use devpit_rpc::{Doing, IslandSession};

use devpit_core::locale::Locale;

use super::*;

fn session(state: Doing, said: Option<&str>) -> IslandSession {
    IslandSession {
        session_id: "s1".to_owned(),
        pane_id: None,
        project_id: None,
        project: Some("api".to_owned()),
        color: None,
        card_id: None,
        card: None,
        root: None,
        state,
        steps: Vec::new(),
        said: said.map(str::to_owned),
        at: 0.0,
    }
}

#[test]
fn stopping_on_a_person_is_told_once() {
    assert_eq!(
        worth_telling(Some(Doing::Working), Doing::Waiting),
        Some(Tell::Waiting)
    );
    assert_eq!(worth_telling(Some(Doing::Waiting), Doing::Waiting), None);
}

/// Working is the island's to show; a notification per tool call would be a
/// notification nobody reads after the first minute.
#[test]
fn working_is_never_told() {
    assert_eq!(worth_telling(Some(Doing::Open), Doing::Working), None);
    assert_eq!(worth_telling(None, Doing::Working), None);
}

#[test]
fn a_turn_is_told_done_only_when_it_was_seen_working() {
    assert_eq!(
        worth_telling(Some(Doing::Working), Doing::Done),
        Some(Tell::Done)
    );
    assert_eq!(worth_telling(None, Doing::Done), None);
    assert_eq!(worth_telling(Some(Doing::Open), Doing::Done), None);
}

#[test]
fn a_failure_is_told_with_what_it_said() {
    let _turn = crate::locale_turn::settled(Locale::En);
    assert_eq!(
        worth_telling(Some(Doing::Working), Doing::Failed),
        Some(Tell::Failed)
    );
    let (who, body) = words(&session(Doing::Failed, Some("rate limited")), Tell::Failed);
    assert_eq!(who, "api");
    assert_eq!(body, "Stopped: rate limited");
}

#[test]
fn a_card_names_the_notification_before_its_project() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let mut on_card = session(Doing::Waiting, None);
    on_card.card = Some("Fix the invoice".to_owned());
    assert_eq!(words(&on_card, Tell::Waiting).0, "Fix the invoice");
}

/// A focus is a door: what comes from another project waits behind it.
#[test]
fn a_focus_holds_back_other_projects_only() {
    let focus = devpit_rpc::HeadsDown {
        project_id: "prj_api".to_owned(),
        since: 0.0,
        until: None,
    };
    assert!(!held_by(Some(&focus), Some("prj_api")));
    assert!(held_by(Some(&focus), Some("prj_web")));
    assert!(held_by(Some(&focus), None));
    assert!(!held_by(None, Some("prj_web")));
}

/// The words are the person's: English says what it always said, Portuguese
/// says it in Portuguese, from the same catalog the tray reads.
#[test]
fn a_notification_is_said_in_the_persons_language() {
    let waiting = session(Doing::Waiting, None);
    let bare = session(Doing::Failed, None);
    let done = session(Doing::Done, None);
    {
        let _turn = crate::locale_turn::settled(Locale::En);
        assert_eq!(words(&waiting, Tell::Waiting).1, "Waiting on you");
        assert_eq!(words(&bare, Tell::Failed).1, "Stopped on an error");
        assert_eq!(words(&done, Tell::Done).1, "Finished its turn");
        let nameless = IslandSession {
            project: None,
            ..session(Doing::Waiting, None)
        };
        assert_eq!(words(&nameless, Tell::Waiting).0, "An agent");
    }
    {
        let _turn = crate::locale_turn::settled(Locale::PtBr);
        assert_eq!(words(&waiting, Tell::Waiting).1, "Esperando você");
        assert_eq!(words(&bare, Tell::Failed).1, "Parou com um erro");
        assert_eq!(
            words(&session(Doing::Failed, Some("limite")), Tell::Failed).1,
            "Parou: limite"
        );
        assert_eq!(words(&done, Tell::Done).1, "Terminou o turno");
        let nameless = IslandSession {
            project: None,
            ..session(Doing::Waiting, None)
        };
        assert_eq!(words(&nameless, Tell::Waiting).0, "Um agente");
    }
}
