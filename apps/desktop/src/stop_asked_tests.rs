use devpit_agentcli::person::{From, Said};
use devpit_rpc::StopAsked;

use super::{ask, confirm, requested, swapped, take, waiting, Request};
use crate::stopping::{as_seen, Clicked};

fn said(text: &str, from: From) -> Said {
    Said {
        id: "u1".into(),
        text: text.into(),
        from,
    }
}

fn live(name: &str, pid: i32, card: Option<&str>) -> devpit_rpc::LiveSession {
    devpit_rpc::LiveSession {
        name: name.to_owned(),
        pid,
        job: None,
        status: "busy".to_owned(),
        kind: "interactive".to_owned(),
        cwd: "/w".to_owned(),
        project_id: Some("prj_a".to_owned()),
        project_name: None,
        card_id: card.map(str::to_owned),
        since: None,
        in_devpit: true,
        waiting: None,
        pane: None,
        session_id: None,
        step: None,
        draft: None,
        remote: Default::default(),
    }
}

/// The sessions running, as the guard reads a message against them.
const LIVE: [&str; 2] = ["fix-login", "review-x"];

/// A stop of `name` asked on `said`, nothing else in the way.
fn request<'a>(said: Option<&'a Said>, name: &'a str) -> Request<'a> {
    Request {
        said,
        name,
        pid: None,
        running: &LIVE,
        swapped: false,
        losing: None,
        force: false,
        drafted: false,
    }
}

fn names(stops: &[StopAsked]) -> Vec<&str> {
    stops.iter().map(|one| one.name.as_str()).collect()
}

fn stop(name: &str, losing: Option<&str>) -> StopAsked {
    StopAsked {
        name: name.to_owned(),
        pid: None,
        losing: losing.map(str::to_owned),
    }
}

#[test]
fn a_stop_nobody_asked_for_is_refused() {
    let now = |said: Option<&Said>| super::allowed(&request(said, "fix-login"));
    assert!(now(Some(&said("pare fix-login", From::Composer))).is_ok());
    assert!(now(Some(&said("stop review-x fix-login", From::Composer))).is_ok());
    let refused = now(Some(&said("como está a fix-login?", From::Composer)));
    assert!(refused.is_err_and(|why| why.contains("fix-login")));
    assert!(now(None).is_err(), "nothing said");
    assert!(
        now(Some(&said("pare fix-login", From::Channel("telegram")))).is_err(),
        "a channel's yes is its button"
    );
}

#[test]
fn a_session_relieved_on_its_card_is_stopped_without_a_word() {
    let ok = said("ok", From::Composer);
    for said in [Some(&ok), None] {
        let relieved = Request {
            swapped: true,
            ..request(said, "fix-login")
        };
        assert!(super::allowed(&relieved).is_ok());
    }
}

/// The handler's own path: a stop the words do not cover is held, and the
/// orchestrator is answered that nothing stopped.
#[test]
fn a_stop_the_words_do_not_cover_waits_for_the_persons_click() {
    let words = said("se sim pode encerrar as sessões", From::Composer);
    let answer =
        requested("p-click", &request(Some(&words), "fix-login")).expect_err("not on these words");
    assert_eq!(answer["done"], false);
    assert!(answer["note"]
        .as_str()
        .is_some_and(|note| note.contains("click")));
    assert_eq!(names(&waiting("p-click")), ["fix-login"]);
    // Asked again, it is still one stop to click.
    let _ = requested("p-click", &request(Some(&words), "fix-login"));
    assert_eq!(names(&waiting("p-click")), ["fix-login"]);
    // On the person's own words it goes ahead, and nothing more waits.
    let word = said("stop review-x", From::Composer);
    assert!(requested("p-click", &request(Some(&word), "review-x")).is_ok());
    assert_eq!(names(&waiting("p-click")), ["fix-login"]);
}

/// The Claude app shows no devpit button: the orchestrator is told the one
/// message that stops every stop waiting.
#[test]
fn from_the_claude_app_the_orchestrator_is_told_the_typed_form() {
    let words = said("pode encerrar as duas", From::RemoteControl);
    let _ = requested("p-typed", &request(Some(&words), "fix-login"));
    let answer = requested("p-typed", &request(Some(&words), "review-x")).expect_err("held");
    let note = answer["note"].as_str().unwrap_or_default();
    assert!(note.contains("`stop fix-login review-x`"), "{note}");
    let composer = said("pode encerrar", From::Composer);
    let answer = requested("p-typed-2", &request(Some(&composer), "fix-login")).expect_err("held");
    let note = answer["note"].as_str().unwrap_or_default();
    assert!(note.contains("Remote"), "{note}");
    assert!(!note.contains("`stop "), "{note}");
}

#[test]
fn one_click_stops_every_session_asked_and_only_those() {
    for name in ["a", "b", "c"] {
        ask("p-batch", stop(name, None), "asked", None);
    }
    let clicked = ["a", "c", "never-asked"].map(str::to_owned);
    assert_eq!(names(&take("p-batch", &clicked)), ["a", "c"]);
    assert_eq!(names(&waiting("p-batch")), ["b"]);
    assert!(waiting("p-other").is_empty(), "another orchestrator's");
}

#[test]
fn what_a_stop_would_lose_goes_inside_its_confirmation() {
    let word = said("stop fix-login", From::Composer);
    let losing = "its folder has 3 file(s) not committed";
    let asked = Request {
        losing: Some(losing.to_owned()),
        ..request(Some(&word), "fix-login")
    };
    let why = super::allowed(&asked).expect_err("not without the person seeing it");
    assert!(why.contains(losing));
    let forced = Request {
        force: true,
        ..request(Some(&word), "fix-login")
    };
    assert!(
        super::allowed(&Request {
            losing: Some(losing.to_owned()),
            ..forced
        })
        .is_ok(),
        "forced on the person's word"
    );
    let _ = requested("p-lose", &asked);
    assert_eq!(waiting("p-lose")[0].losing.as_deref(), Some(losing));
}

/// A ✅ beside a session with a draft waiting is the draft's yes: a stop
/// asked on it waits for the click instead.
#[test]
fn a_yes_mark_for_a_draft_does_not_stop_its_session() {
    let mark = said("✅ fix-login", From::Composer);
    assert!(super::allowed(&request(Some(&mark), "fix-login")).is_ok());
    let drafted = Request {
        drafted: true,
        ..request(Some(&mark), "fix-login")
    };
    assert!(super::allowed(&drafted).is_err_and(|why| why.contains("draft")));
    let word = said("stop fix-login", From::Composer);
    assert!(
        super::allowed(&Request {
            drafted: true,
            ..request(Some(&word), "fix-login")
        })
        .is_ok(),
        "the stop word still stops"
    );
}

/// The loss is read again at the click: one the click did not show stops
/// nothing, and waits again with what it would lose now.
#[test]
fn a_click_stops_only_on_the_loss_it_showed() {
    let dirty = "its folder has 2 file(s) not committed";
    assert_eq!(as_seen(None, None), Ok(()));
    assert_eq!(as_seen(Some(dirty), Some(dirty.to_owned())), Ok(()));
    assert_eq!(as_seen(Some(dirty), None), Ok(()), "less to lose");
    assert_eq!(as_seen(None, Some(dirty.to_owned())), Err(dirty.to_owned()));
    assert_eq!(
        as_seen(Some("it is working right now"), Some(dirty.to_owned())),
        Err(dirty.to_owned())
    );

    let busy = "it is working right now";
    for one in [stop("a", None), stop("b", None), stop("c", Some(busy))] {
        ask("p-fresh", one, "asked", None);
    }
    let shown = waiting("p-fresh");
    // Asked again meanwhile, with more to lose than the click showed.
    ask("p-fresh", stop("c", Some(dirty)), "asked", None);
    let now = |name: &str| match name {
        "b" => Some(dirty.to_owned()),
        "c" => Some(dirty.to_owned()),
        _ => None,
    };
    let mut seen = Vec::new();
    let done = confirm("p-fresh", &shown, |one, shown| {
        seen.push((one.name.clone(), shown.map(str::to_owned)));
        Ok(match as_seen(shown, now(&one.name)) {
            Ok(()) => Clicked::Stopped,
            Err(now) => Clicked::Changed(now),
        })
    });
    assert_eq!(done.stopped, ["a"]);
    assert_eq!(done.changed, [format!("b: {dirty}"), format!("c: {dirty}")]);
    assert!(done.failed.is_empty());
    assert_eq!(
        seen[2],
        ("c".to_owned(), Some(busy.to_owned())),
        "what the click showed"
    );
    let again = waiting("p-fresh");
    assert_eq!(names(&again), ["b", "c"]);
    assert!(again.iter().all(|one| one.losing.as_deref() == Some(dirty)));
}

#[test]
fn a_swap_is_another_session_running_on_the_same_card() {
    let old = live("fix-login", 10, Some("card_a"));
    let relief = live("fix-login-2", 11, Some("card_a"));
    let elsewhere = live("other", 12, Some("card_b"));
    let free = live("free", 13, None);
    assert!(swapped(&[old.clone(), relief.clone()], "fix-login", None));
    assert!(
        !swapped(&[old.clone(), elsewhere.clone()], "fix-login", None),
        "no relief"
    );
    assert!(
        !swapped(&[free.clone(), live("free", 14, None)], "free", None),
        "no card"
    );
    let same_name = live("fix-login", 11, Some("card_a"));
    assert!(
        swapped(&[old.clone(), same_name.clone()], "fix-login", Some(10)),
        "the old one by pid"
    );
    assert!(
        !swapped(&[old, same_name], "fix-login", None),
        "both would stop"
    );
    assert!(!swapped(&[], "fix-login", None), "nothing running");
}

/// One message, `stop a b c`, stops the three through three calls: each call
/// reads the sessions running at that moment, so the ones stopped before are
/// gone from the list, and the message must still read as only the request.
#[test]
fn a_batch_stop_goes_through_one_call_per_name() {
    let message = said("stop sess-a sess-b sess-c", From::Composer);
    let mut running = vec!["sess-a", "sess-b", "sess-c"];
    for name in ["sess-a", "sess-b", "sess-c"] {
        let stopped = super::stopped_on("p-batch", &message.id);
        let mut known: Vec<&str> = running.clone();
        known.extend(stopped.iter().map(String::as_str));
        let mut ask = request(Some(&message), name);
        ask.running = &known;
        assert_eq!(super::allowed(&ask), Ok(()), "{name}");
        super::stopped_now("p-batch", &message.id, name);
        running.retain(|one| *one != name);
    }
}

/// A name the message does not carry stays refused, even after others of it
/// stopped, and the refusal says which name the message lacks.
#[test]
fn a_name_outside_the_message_stays_refused_in_a_batch() {
    let message = said("stop sess-a sess-b", From::Composer);
    super::stopped_now("p-outside", &message.id, "sess-a");
    let stopped = super::stopped_on("p-outside", &message.id);
    let mut known: Vec<&str> = vec!["sess-b", "sess-c"];
    known.extend(stopped.iter().map(String::as_str));
    let mut ask = request(Some(&message), "sess-c");
    ask.running = &known;
    let why = super::allowed(&ask).expect_err("not in the message");
    assert!(
        why.contains("sess-c") && why.contains("does not name"),
        "{why}"
    );
    // Another message does not inherit the names stopped on this one.
    assert!(super::stopped_on("p-outside", "u2").is_empty());
}
