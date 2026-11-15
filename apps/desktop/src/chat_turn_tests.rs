use super::*;

#[test]
fn a_first_turn_accepts_edits_and_starts_now() {
    let head = opening(None, "claude", None, None, None, None, 42.0);
    assert_eq!(head.permission.as_deref(), Some("acceptEdits"));
    assert_eq!(head.created_at, 42.0);
    assert_eq!(head.session_id, None);
}

#[test]
fn a_later_turn_keeps_what_the_conversation_settled() {
    let mut first = opening(
        None,
        "claude",
        None,
        Some(2.0),
        Some("plan".to_owned()),
        None,
        42.0,
    );
    first.session_id = Some("s1".to_owned());
    first.cwd = Some("/w/card".to_owned());
    first.cost_usd = 0.5;

    let later = opening(
        Some(&first),
        "claude",
        Some("haiku".to_owned()),
        None,
        None,
        Some("high".to_owned()),
        99.0,
    );
    assert_eq!(later.created_at, 42.0);
    assert_eq!(later.session_id.as_deref(), Some("s1"));
    assert_eq!(later.cwd.as_deref(), Some("/w/card"));
    assert_eq!(later.cost_usd, 0.5);
    assert_eq!(later.budget_usd, Some(2.0));
    assert_eq!(later.permission.as_deref(), Some("plan"));
    assert_eq!(later.model.as_deref(), Some("haiku"));
    assert_eq!(later.effort.as_deref(), Some("high"));
}

#[test]
fn a_turn_runs_where_the_conversation_was_fixed_and_refuses_a_folder_that_is_gone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let here = dir.path().display().to_string();
    assert_eq!(turn_cwd(None, "/asked").expect("not fixed"), "/asked");
    assert_eq!(turn_cwd(Some(&here), "/asked").expect("fixed"), here);

    let gone = dir.path().join("gone").display().to_string();
    let refused = turn_cwd(Some(&gone), "/asked").expect_err("gone");
    assert_eq!(refused.code, ErrorCode::Conflict);
    assert_eq!(
        refused.message,
        "the folder this conversation ran in is gone"
    );
}

#[test]
fn a_turn_ending_after_a_woken_one_keeps_what_that_one_wrote() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("conv.json");
    let began_with = opening(None, "claude", None, None, None, None, 1.0);
    // A woken turn ended while this one waited: its session and cost are on disk.
    let mut woken = began_with.clone();
    woken.session_id = Some("s_woken".to_owned());
    woken.cost_usd = 0.25;
    devpit_agentcli::head::write_head(&file, &woken).unwrap();

    let end = devpit_rpc::TurnEnd {
        turn_id: String::new(),
        cost_usd: Some(0.5),
        duration_ms: None,
        stop_reason: None,
        is_error: false,
        context: None,
        ..Default::default()
    };
    settle(&file, Some(began_with), "turn_1", &end, None, None);

    let now = devpit_agentcli::head::read_head(&file).unwrap();
    assert_eq!(now.cost_usd, 0.75);
    assert_eq!(now.session_id.as_deref(), Some("s_woken"));
}

/// A turn that failed ends as an error, so the answer stops "working".
#[test]
fn a_failed_turn_ends_as_an_error() {
    let end = failed_end("turn_1");
    assert!(end.is_error);
    assert_eq!(end.turn_id, "turn_1");
    assert_eq!(end.stop_reason.as_deref(), Some("error"));
}

/// A turn that failed after it began keeps what it said, as an answer that
/// is over; one refused before it began writes nothing.
#[test]
fn a_failed_turn_keeps_what_it_said_before_failing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("conversation.jsonl");
    let (_, opened) = messages("turn_1", "hi", 0.0);
    let parts = std::sync::Arc::new(std::sync::Mutex::new(vec![devpit_rpc::Part::Text {
        text: "half an answer".to_owned(),
        parent: None,
    }]));
    let frames = std::sync::Arc::new(std::sync::Mutex::new(0));
    let counted = frames.clone();
    let on_frame = tauri::ipc::Channel::new(move |_| {
        *counted.lock().expect("count") += 1;
        Ok(())
    });
    let refused: Result<(), devpit_rpc::RpcError> = Err(devpit_rpc::RpcError::internal("boom"));

    assert!(or_failed(refused.clone(), "turn_1", &file, None, &opened, &on_frame).is_err());
    assert!(!file.exists(), "a turn that never began wrote an answer");

    assert!(or_failed(refused, "turn_1", &file, Some(&parts), &opened, &on_frame).is_err());
    let (kept, _) = devpit_agentcli::store::read(&file);
    assert_eq!(kept.len(), 1);
    assert!(!kept[0].streaming);
    assert!(format!("{:?}", kept[0].parts).contains("half an answer"));
    // What it failed on stays with it, for a reopened chat to show.
    let ended = kept[0]
        .ended
        .as_ref()
        .expect("the answer keeps how it ended");
    assert!(ended.is_error);
    assert_eq!(ended.error.as_deref(), Some("boom"));
    assert_eq!(
        *frames.lock().expect("count"),
        2,
        "each failure ends its answer"
    );
}

/// The line under a turn reads these, live from the end frame and saved on
/// the answer alike: what it ran on, how long, what it cost.
#[test]
fn a_turn_ends_naming_its_model_effort_time_and_cost() {
    let head = opening(
        None,
        "claude",
        Some("opus".to_owned()),
        None,
        None,
        Some("high".to_owned()),
        1.0,
    );
    let mut said = devpit_agentcli::talk::Said {
        end: devpit_rpc::TurnEnd {
            turn_id: "turn_1".to_owned(),
            cost_usd: Some(0.12),
            duration_ms: Some(81_500.0),
            stop_reason: Some("end_turn".to_owned()),
            context: Some(devpit_rpc::Context {
                used: 45_000,
                window: 200_000,
            }),
            ..Default::default()
        },
        session_id: None,
        init: Some(devpit_rpc::SessionInit {
            model: Some("claude-opus-4-5".to_owned()),
            ..Default::default()
        }),
        anchor: None,
    };
    ran_on(&mut said, &head);
    assert_eq!(
        said.end.model.as_deref(),
        Some("claude-opus-4-5"),
        "the CLI's word over the one asked for"
    );
    let ended = devpit_rpc::Ended::of(&said.end, 9.0);
    assert_eq!(ended.model.as_deref(), Some("claude-opus-4-5"));
    assert_eq!(ended.effort.as_deref(), Some("high"));
    assert_eq!(
        (ended.cost_usd, ended.duration_ms, ended.tokens),
        (Some(0.12), Some(81_500), Some(45_000))
    );

    said.init = None;
    ran_on(&mut said, &head);
    assert_eq!(said.end.model.as_deref(), Some("opus"));
}

/// A chat's process names its conversation, over anything the profile set,
/// and keeps the account's own variables.
#[test]
fn a_chats_process_is_started_naming_its_conversation() {
    let account = vec![
        ("CLAUDE_CONFIG_DIR".to_owned(), "/c".to_owned()),
        (
            devpit_agentapi::hook::CHAT_ENV.to_owned(),
            "conv_other".to_owned(),
        ),
    ];
    let env = with_chat(account, "conv_01A");
    assert_eq!(
        env,
        [
            ("CLAUDE_CONFIG_DIR".to_owned(), "/c".to_owned()),
            (
                devpit_agentapi::hook::CHAT_ENV.to_owned(),
                "conv_01A".to_owned()
            ),
        ]
    );
}
