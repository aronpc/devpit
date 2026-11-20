use devpit_rpc::{Project, Thread};
use serde_json::{json, Value};

use super::*;

fn project(id: &str, folder: &str, orchestrator: Option<&str>) -> Project {
    serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": format!("/tmp/forwarded-tests/{folder}"), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": orchestrator,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project")
}

fn thread(id: &str, profile: &str) -> Thread {
    serde_json::from_value(json!({
        "id": id, "title": id, "profile": profile, "model": null, "costUsd": 0.0,
        "lastAt": 1.0, "cwd": null, "marks": [],
    }))
    .expect("a thread")
}

fn waiting_for(destination: &str) -> usize {
    requests()
        .lock()
        .expect("requests")
        .keys()
        .filter(|(to, ..)| to == destination)
        .count()
}

/// Sends through `message()`, the way an orchestrator does.
fn sent(projects: &[Project], here: &Project, params: Value) -> Result<Value, String> {
    crate::session_bus::message(projects, here, &params, "", "")
}

/// Forwards with a destination that starts, or one that does not.
fn forwarded(
    projects: &[Project],
    here: &Project,
    params: &Value,
    starts: bool,
) -> Option<Result<Value, String>> {
    let to = params["to"].as_str().expect("to");
    let text = params["text"].as_str().expect("text");
    forward(projects, here, to, params, text, |_| match starts {
        true => Ok("conv-1".to_owned()),
        false => Err("its folder is gone".to_owned()),
    })
}

#[test]
fn two_ids_stay_two_requests() {
    assert_eq!(
        keep("dest-two", "orchestrator-a", "one", "c", "first"),
        Ok(false)
    );
    assert_eq!(
        keep("dest-two", "orchestrator-a", "two", "c", "second"),
        Ok(false)
    );
    assert_eq!(waiting_for("dest-two"), 2);
}

#[test]
fn the_same_id_and_text_again_leaves_one_request() {
    assert_eq!(
        keep("dest-again", "orchestrator-a", "x", "c", "do it"),
        Ok(false)
    );
    assert_eq!(
        keep("dest-again", "orchestrator-a", "x", "c", " do it "),
        Ok(true)
    );
    assert_eq!(waiting_for("dest-again"), 1);
    // Another sender's id is its own.
    assert_eq!(
        keep("dest-again", "orchestrator-b", "x", "c", "do it"),
        Ok(false)
    );
    assert_eq!(waiting_for("dest-again"), 2);
}

#[test]
fn same_id_other_text_is_a_conflict() {
    keep("dest-conflict", "orchestrator-a", "x", "c", "first").expect("kept");
    let err = keep("dest-conflict", "orchestrator-a", "x", "c", "other").expect_err("conflict");
    assert!(err.contains("other words"), "{err}");
    let held = requests().lock().expect("requests");
    let key = (
        "dest-conflict".to_owned(),
        "orchestrator-a".to_owned(),
        "x".to_owned(),
    );
    assert_eq!(held[&key].1, "first");
}

#[test]
fn an_empty_long_or_surplus_request_is_refused_before_it_is_kept() {
    assert!(keep("dest-refused", "o", "x", "c", "   ").is_err());
    assert!(keep("dest-refused", "o", "x", "c", &"x".repeat(LONGEST + 1)).is_err());
    assert!(keep("dest-refused", "o", "  ", "c", "text").is_err());
    assert!(keep(
        "dest-refused",
        "o",
        &"i".repeat(LONGEST_ID + 1),
        "c",
        "text"
    )
    .is_err());
    assert_eq!(waiting_for("dest-refused"), 0);
    for n in 0..MOST {
        keep("dest-full", "o", &format!("id{n}"), "c", "text").expect("room");
    }
    assert!(keep("dest-full", "o", "one-more", "c", "text").is_err());
    assert_eq!(waiting_for("dest-full"), MOST);
    // What is kept already can still be sent again.
    assert_eq!(keep("dest-full", "o", "id0", "c", "text"), Ok(true));
}

#[test]
fn listening_conversation_wins() {
    let threads = vec![thread("newest", "p1"), thread("older", "p1")];
    assert_eq!(
        chosen(&["live".to_owned()], threads, Some("p1")).as_deref(),
        Some("live")
    );
}

#[test]
fn the_latest_conversation_on_the_profile_is_the_one_without_a_listener() {
    let threads = vec![
        thread("other-account", "p2"),
        thread("mine", "p1"),
        thread("older", "p1"),
    ];
    assert_eq!(chosen(&[], threads, Some("p1")).as_deref(), Some("mine"));
    assert_eq!(chosen(&[], Vec::new(), Some("p1")), None);
}

#[test]
fn an_unknown_orchestrator_is_refused_with_the_valid_ones() {
    let here = project("prj_a", "a", Some("prof"));
    let projects = [here.clone(), project("prj_b", "b", Some("prof"))];
    let err = sent(
        &projects,
        &here,
        json!({ "to": "orchestrator-zzz", "text": "hi", "id": "1" }),
    )
    .expect_err("unknown");
    assert!(err.contains("orchestrator-b"), "{err}");
    assert!(!err.contains("orchestrator-a"), "{err}");
}

#[test]
fn an_orchestrator_never_forwards_to_itself() {
    let here = project("prj_a", "a", Some("prof"));
    let projects = [here.clone(), project("prj_b", "b", Some("prof"))];
    let err = sent(
        &projects,
        &here,
        json!({ "to": "orchestrator-a", "text": "hi", "id": "1" }),
    )
    .expect_err("itself");
    assert!(err.contains("itself"), "{err}");
}

#[test]
fn a_forwarded_request_needs_its_id() {
    let here = project("prj_a", "a", Some("prof"));
    let projects = [here.clone(), project("prj_id", "id", Some("prof"))];
    let err = forwarded(
        &projects,
        &here,
        &json!({ "to": "orchestrator-id", "text": "hi" }),
        true,
    )
    .expect("a route")
    .expect_err("no id");
    assert!(err.contains("`id`"), "{err}");
    assert_eq!(waiting_for("prj_id"), 0);
}

#[test]
fn a_forwarded_request_is_kept_without_a_letter_or_a_turn() {
    let here = project("prj_a", "a", Some("prof"));
    let projects = [here.clone(), project("prj_kept", "kept", Some("prof"))];
    let params = json!({ "to": "orchestrator-kept", "text": "please look", "id": "r1" });
    let said = forwarded(&projects, &here, &params, true)
        .expect("a route")
        .expect("kept");
    assert_eq!(said["again"], json!(false));
    assert_eq!(said["forwarded"], json!("orchestrator-kept"));
    assert_eq!(waiting_for("prj_kept"), 1);
    assert!(
        !crate::session_bus::waits_in_chat("prj_kept"),
        "no letter for its chat, so no turn"
    );
    let again = forwarded(&projects, &here, &params, true)
        .expect("a route")
        .expect("again");
    assert_eq!(again["again"], json!(true));
    assert_eq!(waiting_for("prj_kept"), 1);
}

#[test]
fn a_destination_that_does_not_start_keeps_nothing_and_the_same_id_goes_again() {
    let here = project("prj_a", "a", Some("prof"));
    let projects = [here.clone(), project("prj_down", "down", Some("prof"))];
    let params = json!({ "to": "orchestrator-down", "text": "please look", "id": "r1" });
    let err = forwarded(&projects, &here, &params, false)
        .expect("a route")
        .expect_err("down");
    assert!(err.contains("orchestrator-down"), "{err}");
    assert!(err.contains("its folder is gone"), "{err}");
    assert!(err.contains("send again with the same id"), "{err}");
    assert_eq!(waiting_for("prj_down"), 0);
    let kept = forwarded(&projects, &here, &params, true)
        .expect("a route")
        .expect("kept");
    assert_eq!(kept["again"], json!(false));
    assert_eq!(waiting_for("prj_down"), 1);
}

#[test]
fn through_message_a_destination_unknown_to_the_store_does_not_start() {
    let here = project("prj_a", "a", Some("prof"));
    let projects = [
        here.clone(),
        project("prj_nostore", "nostore", Some("prof")),
    ];
    let err = sent(
        &projects,
        &here,
        json!({ "to": "orchestrator-nostore", "text": "hi", "id": "1" }),
    )
    .expect_err("does not start");
    assert!(err.contains("send again with the same id"), "{err}");
    assert_eq!(waiting_for("prj_nostore"), 0);
}
