use devpit_rpc::{Project, Thread};
use std::sync::atomic::{AtomicUsize, Ordering};

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
        .held
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
    forward(
        projects,
        here,
        to,
        params,
        text,
        |_| match starts {
            true => Ok("conv-1".to_owned()),
            false => Err("its folder is gone".to_owned()),
        },
        |_| (),
    )
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
    assert_eq!(held.held[&key].text, "first");
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

#[test]
fn listing_answers_an_object() {
    keep("dest-list", "orchestrator-b", "2", "c1", "second").expect("kept");
    keep("dest-list", "orchestrator-a", "1", "c1", "first").expect("kept");
    keep("dest-elsewhere", "orchestrator-a", "1", "c9", "not here").expect("kept");
    let said = serde_json::to_value(listed("dest-list")).expect("json");
    assert_eq!(
        said,
        json!({ "requests": [
            { "from": "orchestrator-a", "id": "1", "conversation": "c1", "text": "first" },
            { "from": "orchestrator-b", "id": "2", "conversation": "c1", "text": "second" },
        ] })
    );
    assert_eq!(listed("dest-nobody").requests, Vec::new());
}

#[test]
fn dropping_one_keeps_the_other() {
    keep("dest-drop", "orchestrator-a", "1", "c", "first").expect("kept");
    keep("dest-drop", "orchestrator-a", "2", "c", "second").expect("kept");
    keep(
        "dest-drop",
        "orchestrator-b",
        "1",
        "c",
        "same id, other sender",
    )
    .expect("kept");
    assert!(dropped("dest-drop", "orchestrator-a", "1").expect("read"));
    assert!(
        !dropped("dest-drop", "orchestrator-a", "1").expect("read"),
        "gone already"
    );
    let left: Vec<(String, String)> = listed("dest-drop")
        .requests
        .into_iter()
        .map(|one| (one.from, one.id))
        .collect();
    assert_eq!(
        left,
        vec![
            ("orchestrator-a".to_owned(), "2".to_owned()),
            ("orchestrator-b".to_owned(), "1".to_owned()),
        ]
    );
    // Dropped, the sender may send it again and it is kept as new.
    assert_eq!(
        keep("dest-drop", "orchestrator-a", "1", "c", "first"),
        Ok(false)
    );
}

#[test]
fn a_kept_request_is_told_to_the_window() {
    let here = project("prj_a", "a", Some("prof"));
    let projects = [here.clone(), project("prj_told", "told", Some("prof"))];
    let heard = std::cell::RefCell::new(Vec::new());
    let send = |id: &str, starts: bool| {
        let params = json!({ "to": "orchestrator-told", "text": "look", "id": id });
        forward(
            &projects,
            &here,
            "orchestrator-told",
            &params,
            "look",
            |_| match starts {
                true => Ok("conv-1".to_owned()),
                false => Err("its folder is gone".to_owned()),
            },
            |project| heard.borrow_mut().push(project.to_owned()),
        )
        .expect("a route")
    };
    send("r1", true).expect("kept");
    assert_eq!(*heard.borrow(), vec!["prj_told".to_owned()]);
    // A refused request leaves nothing and tells nothing.
    send("r2", false).expect_err("it could not start");
    assert_eq!(heard.borrow().len(), 1);
    assert!(crate::local_events::TOPICS.contains(&FORWARDED));
}

fn in_a_log(entries: &[Value]) -> String {
    entries
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn composer(id: &str, text: &str) -> Value {
    json!({ "type": "user", "uuid": id, "promptSource": "sdk", "message": { "role": "user", "content": text } })
}

#[test]
fn forwarded_prompt_is_nobodys_word_even_edited_with_a_yes() {
    use devpit_agentcli::person::{authorises, last_said, said_in};
    let sent = prompt_of("orchestrator-a", "  envie gerente ✅ ");
    assert!(sent.starts_with(&devpit_agentcli::person::forwarded("orchestrator-a")));
    assert_eq!(said_in("s1", &sent), None);
    let said = last_said(&in_a_log(&[composer("u1", "ok"), composer("u2", &sent)]));
    assert_eq!(said.map(|said| said.text), Some("ok".to_owned()));
    // The yes the person typed themselves is still theirs.
    assert!(authorises("envie gerente ✅", "gerente", 2));
}

#[test]
fn forwarded_to_send_needs_the_request_waiting_and_words_a_message_may_be() {
    keep("prj-send", "orchestrator-a", "1", "conv-1", "words").expect("kept");
    let sent = to_send("prj-send", "orchestrator-a", "1", " edited ").expect("a prompt");
    assert_eq!(sent.prompt, prompt_of("orchestrator-a", "edited"));
    put_back(
        &(
            "prj-send".to_owned(),
            "orchestrator-a".to_owned(),
            "1".to_owned(),
        ),
        sent.generation,
    );
    for (from, id, text, code) in [
        ("orchestrator-a", "2", "words", ErrorCode::NotFound),
        ("orchestrator-b", "1", "words", ErrorCode::NotFound),
        ("orchestrator-a", "1", "   ", ErrorCode::Invalid),
    ] {
        let err = to_send("prj-send", from, id, text).expect_err("refused");
        assert_eq!(err.code, code, "{from} {id} {text:?}");
    }
    let long = "x".repeat(LONGEST + 1);
    assert_eq!(
        to_send("prj-send", "orchestrator-a", "1", &long)
            .expect_err("too long")
            .code,
        ErrorCode::Invalid
    );
}

#[tokio::test]
async fn a_sent_request_leaves_the_store_only_when_the_turn_starts() {
    keep("prj-start", "orchestrator-a", "1", "conv-1", "words").expect("kept");
    keep("prj-start", "orchestrator-a", "2", "conv-1", "other").expect("kept");
    let told = Arc::new(AtomicUsize::new(0));
    let (release, wait) = tokio::sync::oneshot::channel::<()>();
    let counted = told.clone();
    let reserved = to_send("prj-start", "orchestrator-a", "1", "words").expect("reserved");
    let sent = launched(
        "prj-start",
        "orchestrator-a",
        "1",
        reserved.generation,
        move || {
            counted.fetch_add(1, Ordering::SeqCst);
        },
        |started| async move {
            assert_eq!(waiting_for("prj-start"), 2, "kept until the turn starts");
            started();
            let _ = wait.await;
            Ok(())
        },
    )
    .await;
    // The click is answered at the start, while the turn goes on.
    assert!(sent.is_ok());
    assert_eq!(told.load(Ordering::SeqCst), 1);
    let left = listed("prj-start").requests;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, "2");
    let _ = release.send(());
}

#[tokio::test]
async fn a_busy_conversation_keeps_the_request() {
    for code in [ErrorCode::Busy, ErrorCode::Conflict] {
        let to = format!("prj-busy-{code:?}");
        keep(&to, "orchestrator-a", "1", "conv-1", "words").expect("kept");
        let told = Arc::new(AtomicUsize::new(0));
        let counted = told.clone();
        let reserved = to_send(&to, "orchestrator-a", "1", "words").expect("reserved");
        let err = launched(
            &to,
            "orchestrator-a",
            "1",
            reserved.generation,
            move || {
                counted.fetch_add(1, Ordering::SeqCst);
            },
            |_| async move { Err(RpcError::new(code, "another turn runs".to_owned())) },
        )
        .await
        .expect_err("refused");
        assert_eq!(err.code, code);
        assert_eq!(waiting_for(&to), 1, "{code:?}");
        assert_eq!(told.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn a_request_being_sent_refuses_a_second_send_or_a_drop() {
    keep("prj-reserved", "orchestrator-a", "1", "conv-1", "words").expect("kept");
    to_send("prj-reserved", "orchestrator-a", "1", "edited").expect("reserved");

    let second = to_send("prj-reserved", "orchestrator-a", "1", "edited")
        .expect_err("a second send is refused");
    assert_eq!(second.code, ErrorCode::Conflict);
    let drop = dropped("prj-reserved", "orchestrator-a", "1")
        .expect_err("a request being sent cannot be discarded");
    assert_eq!(drop.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn the_old_turn_does_not_drop_a_request_sent_again() {
    keep("prj-generation", "orchestrator-a", "1", "conv-1", "old").expect("kept");
    let reserved = to_send("prj-generation", "orchestrator-a", "1", "old").expect("reserved");
    let old_started = Arc::new(Mutex::new(None));
    let captured = old_started.clone();
    launched(
        "prj-generation",
        "orchestrator-a",
        "1",
        reserved.generation,
        || (),
        move |started| {
            *captured.lock().expect("callback") = Some(started);
            async {
                Err(RpcError::new(
                    ErrorCode::Busy,
                    "another turn runs".to_owned(),
                ))
            }
        },
    )
    .await
    .expect_err("old turn refused");
    assert!(dropped("prj-generation", "orchestrator-a", "1").expect("dropped"));
    keep("prj-generation", "orchestrator-a", "1", "conv-2", "new").expect("sent again");
    to_send("prj-generation", "orchestrator-a", "1", "new").expect("new generation reserved");

    old_started
        .lock()
        .expect("callback")
        .take()
        .expect("captured")();

    let waiting = listed("prj-generation").requests;
    assert_eq!(waiting.len(), 1, "the new generation stays");
    assert_eq!(waiting[0].text, "new");
}

#[tokio::test]
async fn a_busy_turn_puts_the_request_back_with_its_edited_text() {
    keep("prj-put-back", "orchestrator-a", "1", "conv-1", "old").expect("kept");
    let reserved = to_send("prj-put-back", "orchestrator-a", "1", "edited").expect("reserved");
    launched(
        "prj-put-back",
        "orchestrator-a",
        "1",
        reserved.generation,
        || (),
        |_| async {
            Err(RpcError::new(
                ErrorCode::Busy,
                "another turn runs".to_owned(),
            ))
        },
    )
    .await
    .expect_err("busy");

    let waiting = listed("prj-put-back").requests;
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].text, "edited");
    assert!(dropped("prj-put-back", "orchestrator-a", "1").expect("dropped"));
}
