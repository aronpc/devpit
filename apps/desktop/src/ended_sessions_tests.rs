use super::*;

const ID: &str = "0c1b7a2e-5d4f-4a3b-9c8d-7e6f5a4b3c2d";

#[test]
fn an_ended_session_resumes_as_the_same_conversation_under_its_name() {
    let line = resumed(
        "claude --settings /s.json",
        "api-a",
        Some("--resume {}"),
        ID,
    )
    .expect("line");
    assert_eq!(
        line,
        format!("claude --settings /s.json --name 'api-a' --resume {ID}")
    );
}

#[test]
fn a_resume_that_would_start_afresh_or_run_something_else_is_refused() {
    assert!(
        resumed("claude", "api-a", None, ID).is_err(),
        "an agent with no resume started afresh"
    );
    assert!(resumed("claude", "api-a", Some("--resume {}"), "abc; rm -rf ~").is_err());
    assert!(resumed("claude", "a\x03b", Some("--resume {}"), ID).is_err());
}

#[test]
fn files_left_uncommitted_in_a_folder_are_counted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .output()
            .expect("git");
    };
    git(&["init", "-q"]);
    assert_eq!(dirty_in(&dir.path().display().to_string()), Some(0));
    std::fs::write(dir.path().join("half.rs"), "fn").expect("write");
    assert_eq!(dirty_in(&dir.path().display().to_string()), Some(1));
    assert_eq!(dirty_in("/no/such/folder"), None);
}

#[test]
fn a_stop_that_would_lose_work_says_why_and_asks() {
    use crate::stopping::would_lose;
    assert_eq!(would_lose("a7", false, Some(0)), None);
    assert_eq!(would_lose("a7", false, None), None);
    assert_eq!(
        would_lose("a7", true, None).as_deref(),
        Some("a7: it is working right now — stop it anyway?")
    );
    assert_eq!(
        would_lose("a7", false, Some(12)).as_deref(),
        Some("a7: its folder has 12 file(s) not committed — stop it anyway?")
    );
}

fn kept(id: &str, first: i64, ended: Option<i64>) -> SeenSessionRow {
    SeenSessionRow {
        session_id: id.to_owned(),
        profile_id: "claude".to_owned(),
        name: format!("api-{id}"),
        cwd: "/w/card".to_owned(),
        project_id: Some("prj_api".to_owned()),
        card_id: Some("card_a".to_owned()),
        first_seen_at: first,
        last_seen_at: ended.unwrap_or(first),
        last_status: Some("idle".to_owned()),
        ended_at: ended,
        ended_by: None,
    }
}

#[test]
fn a_cards_sessions_are_the_running_ones_then_the_latest_to_end() {
    let rows = vec![
        kept("old", 10, Some(20)),
        kept("run-early", 5, None),
        kept("late", 15, Some(50)),
        kept("run-late", 40, None),
    ];
    let order: Vec<_> = on_card(rows, &[], |_| None)
        .into_iter()
        .map(|one| one.session_id)
        .collect();
    assert_eq!(order, ["run-late", "run-early", "late", "old"]);
}

#[test]
fn only_a_running_session_carries_its_terminal_and_remote_control() {
    let live: devpit_rpc::LiveSession = serde_json::from_value(json!({
        "name": "api-s1", "pid": 1, "job": null, "status": "idle", "kind": "interactive",
        "cwd": "/w/card", "projectId": "prj_api", "projectName": "api", "cardId": "card_a",
        "since": null, "inDevpit": true, "waiting": null,
        "pane": { "projectId": "prj_api", "paneId": "leaf_1" }, "sessionId": "s1", "step": null,
        "draft": null, "remote": { "on": true, "url": "https://claude.ai/code/session_1" },
    }))
    .expect("a session");
    // Kept as ended under an id that runs again: the row is history until listed.
    let ended_id = devpit_rpc::LiveSession {
        session_id: Some("s2".into()),
        ..live.clone()
    };
    let both = on_card(
        vec![kept("s1", 10, None), kept("s2", 5, Some(9))],
        &[live, ended_id],
        |cwd| (cwd == "/w/card").then_some(3),
    );
    let running = &both[0];
    assert_eq!(
        running.pane.as_ref().map(|pane| pane.pane_id.as_str()),
        Some("leaf_1")
    );
    assert!(running.remote.on);
    assert_eq!((running.ended_at, running.dirty), (None, Some(3)));
    let ended = &both[1];
    assert_eq!(
        (ended.session_id.as_str(), ended.ended_at),
        ("s2", Some(9000.0))
    );
    assert!(ended.pane.is_none() && !ended.remote.on);
}

/// A conversation a live process runs is not started a second time: the
/// resume is refused, naming the process to message instead.
#[test]
fn a_conversation_a_live_process_runs_is_not_resumed_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).expect("mkdir");
    let me = std::process::id();
    let listing = json!({ "pid": me, "name": "api-a", "sessionId": ID, "cwd": "/w" });
    std::fs::write(sessions.join(format!("{me}.json")), listing.to_string()).expect("listing");

    let refused = not_running(std::slice::from_ref(&sessions), ID).expect_err("refused");
    assert_eq!(refused.code, ErrorCode::Conflict);
    assert!(refused.message.contains("api-a"), "{}", refused.message);
    assert!(not_running(&[sessions], "another").is_ok());
}
