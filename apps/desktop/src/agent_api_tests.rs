use super::*;

fn project(id: &str, root: &Path, worktrees: &[&Path]) -> Project {
    serde_json::from_value(json!({
        "id": id,
        "name": id,
        "rootPath": root.display().to_string(),
        "group": null,
        "accent": "#000000",
        "worktrees": worktrees.iter().map(|tree| json!({
            "id": tree.display().to_string(),
            "branch": "main",
            // As git lists it: the last segment here, the whole path apart.
            "folder": tree.file_name().map(|name| name.to_string_lossy().into_owned()),
            "path": tree.display().to_string(),
            "ahead": 0,
            "behind": 0,
            "dirtyFiles": null,
            "origin": "devpit",
            "current": false,
        })).collect::<Vec<_>>(),
        "unreadable": null,
        "origin": null,
        "lastOpenedAt": null,
        "icon": null,
        "color": null,
    }))
    .expect("a project")
}

#[test]
fn the_project_is_the_one_whose_checkout_holds_the_agent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app = dir.path().join("app");
    let tree = dir.path().join("worktrees").join("card-1");
    let deep = app.join("src").join("lib");
    for path in [&app, &tree, &deep] {
        std::fs::create_dir_all(path).expect("mkdir");
    }
    let projects = vec![
        project("app", &app, &[&tree]),
        project("other", &dir.path().join("x"), &[]),
    ];

    assert_eq!(
        project_at(&projects, &deep).map(|one| one.id.as_str()),
        Some("app")
    );
    // A card's worktree lives outside the root and is still the project's.
    assert_eq!(
        project_at(&projects, &tree).map(|one| one.id.as_str()),
        Some("app")
    );
    assert!(project_at(&projects, dir.path()).is_none());
}

/// A folder named like the root but beside it is not inside it.
#[test]
fn a_sibling_with_the_same_prefix_is_not_the_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app = dir.path().join("app");
    let sibling = dir.path().join("app-old");
    std::fs::create_dir_all(&app).expect("mkdir");
    std::fs::create_dir_all(&sibling).expect("mkdir");
    assert!(project_at(&[project("app", &app, &[])], &sibling).is_none());
}

#[test]
fn a_question_it_cannot_read_or_answer_says_so() {
    assert!(answer(None, "not json").contains("\"error\""));
    let unknown = answer(None, r#"{"method":"card_move","cwd":"/"}"#);
    assert!(unknown.contains("does not answer `card_move`"), "{unknown}");
}

#[test]
fn it_says_what_it_answers() {
    let said: Value = serde_json::from_str(&answer(None, r#"{"method":"methods"}"#)).expect("json");
    assert_eq!(said["ok"], json!(METHODS.as_slice()));
}

#[test]
fn the_terminals_status_is_answered_from_anywhere() {
    let said: Value =
        serde_json::from_str(&answer(None, r#"{"method":"status","cwd":"/"}"#)).expect("json");
    assert!(said["ok"]["running"].is_u64(), "{said}");
    assert!(said["ok"]["waiting"].is_u64(), "{said}");
}

#[test]
fn the_terminals_pairing_is_asked_of_no_project() {
    // Remote is off in the tests' home, unless another test turned it on.
    let said = answer(None, r#"{"method":"pair","cwd":"/"}"#);
    assert!(
        !said.contains("does not answer") && !said.contains("no devpit project contains"),
        "{said}"
    );
    assert!(
        said.contains("turn Remote on first") || said.contains("\"code\""),
        "{said}"
    );
}

#[test]
fn a_comment_is_signed_by_an_agent_devpit_knows_or_by_nobody_in_particular() {
    assert_eq!(signed("claude"), "claude");
    assert_eq!(signed("codex"), "codex");
    assert_eq!(signed("root"), "agent");
    assert_eq!(signed(""), "agent");
}

/// A lane that runs a step starts work when a card enters it: not an agent's
/// move to make.
#[test]
fn a_lane_with_a_step_is_behind_the_gate() {
    let lane = |step: Value| -> devpit_rpc::Column {
        serde_json::from_value(json!({
            "id": "col_1", "name": "Review", "position": 1, "step": step,
            "onPass": null, "autonomy": "manual"
        }))
        .expect("a column")
    };
    assert!(gate(&lane(Value::Null)).is_ok());
    let step = json!({ "id": "stp_1", "kind": "command", "name": "Tests", "config": "{}", "irreversible": false });
    let refused = gate(&lane(step)).expect_err("gated");
    assert!(refused.contains("`Review` runs a step"), "{refused}");
}

fn orchestrator(id: &str, root: &Path) -> Project {
    let mut one = project(id, root, &[]);
    one.orchestrator = Some("claude".to_owned());
    one
}

#[test]
fn only_an_orchestrator_reaches_another_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let projects = vec![
        project("prj_app", &dir.path().join("app"), &[]),
        project("prj_api", &dir.path().join("api"), &[]),
        orchestrator("prj_orch", &dir.path().join("orch")),
    ];
    let (app, orch) = (&projects[0], &projects[2]);

    assert_eq!(
        reached(&projects, orch, Some("prj_api")).map(|one| one.id.as_str()),
        Ok("prj_api")
    );
    assert_eq!(
        reached(&projects, orch, Some("PRJ_API")).map(|one| one.id.as_str()),
        Ok("prj_api")
    );
    assert_eq!(
        reached(&projects, app, None).map(|one| one.id.as_str()),
        Ok("prj_app")
    );
    assert!(
        reached(&projects, app, Some("prj_api")).is_err(),
        "a project agent reached another project"
    );
    assert!(
        reached(&projects, orch, Some("prj_orch")).is_err(),
        "an orchestrator was reachable as a project"
    );
    // Naming its own project asks for nothing a project's session lacks.
    assert_eq!(
        reached(&projects, app, Some("prj_app")).map(|one| one.id.as_str()),
        Ok("prj_app")
    );
}

#[test]
fn a_project_agent_reads_no_session_and_nothing_answers_a_prompt() {
    let dir = tempfile::tempdir().expect("tempdir");
    let projects = vec![project("prj_app", &dir.path().join("app"), &[])];
    for method in ["projects", "sessions", "screen"] {
        let asked: Asked = serde_json::from_value(json!({
            "method": method, "params": { "name": "anyone" }, "cwd": "",
        }))
        .expect("asked");
        let refused = respond_in(None, &projects, &projects[0], &asked).expect_err(method);
        assert!(
            refused.contains("only an orchestrator"),
            "{method}: {refused}"
        );
    }
    // A session's question is the person's to answer: no agent method presses.
    assert!(!METHODS
        .iter()
        .any(|method| method.contains("answer") || method.contains("press")));
}

/// A question stuck behind something slow is answered before the agent gives
/// up, and the answer says it may still be applied.
#[test]
fn a_slow_answer_is_cut_short_with_an_error() {
    let started = std::time::Instant::now();
    let said = super::within(std::time::Duration::from_millis(50), "board", || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        "late".to_owned()
    });
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    let said: serde_json::Value = serde_json::from_str(&said).expect("json");
    assert!(said["error"]
        .as_str()
        .is_some_and(|why| why.contains("`board`")));
}

/// Inside the deadline the answer is the work's own.
#[test]
fn a_quick_answer_passes_through() {
    let said = super::within(std::time::Duration::from_secs(2), "board", || {
        "{\"ok\":1}".to_owned()
    });
    assert_eq!(said, "{\"ok\":1}");
}

/// The app gives up before the client does, so its error is the one read.
#[test]
fn the_app_stops_waiting_before_the_agent_does() {
    for method in ["board", "start"] {
        assert!(super::patience(method) < devpit_agentapi::client::wait_for(method));
    }
}

/// A write queued behind another writer waits out the store's busy timeout
/// and then lands; told "still working" before that, the agent wrote twice.
#[test]
fn a_write_is_waited_for_longer_than_the_store_waits_for_its_lock() {
    let busy = std::time::Duration::from_millis(devpit_core::store::BUSY_WAIT_MS.into());
    for method in WRITES {
        assert!(super::patience(method) > busy * 2, "{method}");
    }
}

/// Hooks give up on their own clock: the answer has to come inside it, or
/// the hook fails instead of hearing why.
#[test]
fn a_hooks_question_is_answered_inside_the_hooks_own_timeout() {
    use devflow_engine::plugin::launch::{CONTEXT_WAITS, GATE_WAITS};
    use devpit_agentapi::client::wait_for;
    use std::time::Duration;
    assert!(wait_for("devflow_gate") < Duration::from_secs(GATE_WAITS));
    assert!(wait_for("devflow_session") < Duration::from_secs(CONTEXT_WAITS));
}

/// Decisions reads the state it is handed and changes nothing on the board.
#[test]
fn decisions_are_answered_and_write_nothing() {
    for method in crate::agent_decide::METHODS {
        assert!(METHODS.contains(&method), "{method}");
        assert!(!WRITES.contains(&method), "{method}");
    }
}

/// A card id from another project's board does not open that project's
/// checkout: its git state is out of this session's reach.
#[test]
fn a_cards_checkout_is_read_only_from_its_own_project() {
    let store = crate::projects::store().expect("store");
    let (mine, theirs) = (
        tempfile::tempdir().expect("tempdir"),
        tempfile::tempdir().expect("tempdir"),
    );
    let ids: Vec<String> = [&mine, &theirs]
        .iter()
        .map(|dir| {
            let id = store.add_project(dir.path(), None).expect("project");
            store.ensure_board(&id).expect("board");
            id
        })
        .collect();
    let column = store.columns(&ids[1]).expect("columns")[0].id.clone();
    let card = store
        .create_card(&ids[1], &column, "Theirs", "")
        .expect("card");
    let refused = checkout_of(&project(&ids[0], mine.path(), &[]), &card).expect_err("refused");
    assert!(refused.contains("no card"), "{refused}");
    assert_eq!(
        checkout_of(&project(&ids[1], theirs.path(), &[]), &card),
        Ok(None)
    );
}

#[test]
fn a_comment_lands_on_its_own_projects_card_and_on_no_other() {
    let store = crate::projects::store().expect("store");
    let (mine, theirs) = (
        tempfile::tempdir().expect("tempdir"),
        tempfile::tempdir().expect("tempdir"),
    );
    let ids: Vec<String> = [&mine, &theirs]
        .iter()
        .map(|dir| {
            let id = store.add_project(dir.path(), None).expect("project");
            store.ensure_board(&id).expect("board");
            id
        })
        .collect();
    let card_in = |id: &str| {
        let column = store.columns(id).expect("columns")[0].id.clone();
        store.create_card(id, &column, "A card", "").expect("card")
    };
    let (ours, other) = (card_in(&ids[0]), card_in(&ids[1]));
    let projects = vec![project(&ids[0], mine.path(), &[])];
    let comment = |card: &str| -> Asked {
        serde_json::from_value(json!({
            "method": "comment", "params": { "cardId": card, "body": "seen" }, "cwd": "",
        }))
        .expect("asked")
    };

    let refused = respond_in(None, &projects, &projects[0], &comment(&other)).expect_err("theirs");
    assert!(refused.contains("no card"), "{refused}");
    assert!(store.comments(&other).expect("comments").is_empty());

    let answered = respond_in(None, &projects, &projects[0], &comment(&ours)).expect("ours");
    assert_eq!(answered["card"]["id"], ours.as_str());
    assert_eq!(store.comments(&ours).expect("comments").len(), 1);
}

/// A move, a start or a finish waited on every agent CLI listing its live
/// sessions, and the agent heard "devpit is still working" (B12).
#[test]
fn only_the_board_and_the_context_ask_what_each_cards_sessions_are_doing() {
    for method in WRITES {
        assert!(!shows_activity(method), "{method}");
    }
    assert!(shows_activity("board"));
    assert!(shows_activity("context"));
}

#[test]
fn an_agent_changes_only_the_comments_it_wrote() {
    assert!(may_change_comment("claude", "claude").is_ok());
    let theirs = may_change_comment(crate::cards::YOU, "claude").expect_err("the person's");
    assert!(theirs.contains("person"), "{theirs}");
    assert!(may_change_comment("codex", "claude").is_err());
}

/// `devpit_log` writes a preference to the log, a rule only when asked, and
/// a project's rule under that project's own heading.
#[test]
fn a_logged_preference_goes_where_the_call_says() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("orch");
    std::fs::create_dir_all(root.join("context")).expect("context");
    let rules_last = "## Rules for sessions\n- Never push to main.\n";
    std::fs::write(root.join("context/preferences.md"), rules_last).expect("prefs");
    let projects = vec![
        project("prj_billing", &dir.path().join("billing"), &[]),
        orchestrator("prj_orch", &root),
    ];
    let log = |params: Value| {
        let asked: Asked = serde_json::from_value(json!({
            "method": "log", "params": params, "cwd": "",
        }))
        .expect("asked");
        respond_in(None, &projects, &projects[1], &asked)
    };
    log(json!({ "log": "preferences", "text": "likes short reports" })).expect("log");
    log(
        json!({ "log": "preferences", "text": "Deploy after the gate.", "project": "PRJ_BILLING" }),
    )
    .expect("project");
    log(json!({ "log": "preferences", "text": "Write in English.", "section": "rules" }))
        .expect("rules");
    let written = std::fs::read_to_string(root.join("context/preferences.md")).expect("read");
    let rules = crate::orchestrator_notes::rules_in(&written).expect("rules");
    assert!(rules.starts_with("- Never push to main.\n- "), "{written}");
    assert!(rules.ends_with("— Write in English."), "{written}");
    assert!(!rules.contains("likes short reports"), "{written}");
    assert!(
        written.contains("## Rules for sessions: prj_billing\n\n- "),
        "{written}"
    );
    assert!(log(json!({ "log": "preferences", "text": "x", "project": "nowhere" })).is_err());
    assert!(log(json!({ "log": "preferences", "text": "x", "section": "other" })).is_err());
}

/// The assistant's tools answer only an orchestrator whose Assistant mode is
/// on: a project's session asking by name is refused, and writes nothing.
#[test]
fn the_assistants_methods_refuse_without_assistant_mode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let projects = vec![project("prj_app", &dir.path().join("app"), &[])];
    for method in crate::assistant::METHODS {
        assert!(METHODS.contains(&method), "{method}");
        assert!(!WRITES.contains(&method), "{method}");
        let asked: Asked = serde_json::from_value(json!({
            "method": method, "params": { "why": "x", "routine": "ci", "time": "08:00" }, "cwd": "",
        }))
        .expect("asked");
        let refused = respond_in(None, &projects, &projects[0], &asked).expect_err(method);
        assert!(
            refused.contains("Assistant mode is off"),
            "{method}: {refused}"
        );
    }
}

/// A project's session reaches its own project's agent and no other one; an
/// orchestrator asks with its own tool, and a project never learned is off,
/// and consulting it reads nothing.
#[test]
fn a_session_consults_its_own_projects_agent_and_no_other() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (app, other) = (dir.path().join("app"), dir.path().join("other"));
    for root in [&app, &other] {
        std::fs::create_dir_all(root.join(".devpit/project-agent")).expect("memory");
        // Learned before projects were turned on one by one, so it is on.
        crate::project_memory::onboarded(root, "2026-11-14").expect("learned");
    }
    let projects = vec![project("app", &app, &[]), project("other", &other, &[])];
    let consult = |params: Value| -> Asked {
        serde_json::from_value(json!({
            "method": "project_agent_consult", "params": params, "cwd": "", "author": "agt-1",
        }))
        .expect("asked")
    };
    let question = json!({ "question": "Where are the guards?" });

    let refused = respond_in(
        None,
        &projects,
        &projects[0],
        &consult(json!({ "project": "other", "question": "Where are the guards?" })),
    )
    .expect_err("another project");
    assert!(refused.contains("only an orchestrator"), "{refused}");
    // Its own passes every guard and goes to the agent, which needs the window.
    let own = respond_in(None, &projects, &projects[0], &consult(question.clone()))
        .expect_err("no window in a test");
    assert!(own.contains("window is not running"), "{own}");

    let orchestrating = orchestrator("prj_orch", &dir.path().join("orch"));
    let refused = respond_in(None, &projects, &orchestrating, &consult(question.clone()))
        .expect_err("an orchestrator");
    assert!(refused.contains("devpit_project_agent_ask"), "{refused}");

    std::fs::remove_dir_all(app.join(".devpit")).expect("forgotten");
    let none =
        respond_in(None, &projects, &projects[0], &consult(question)).expect_err("never asked");
    assert_eq!(none, LABS_OFF);
    assert!(!app.join(".devpit").exists(), "consulting made a memory");
}

#[test]
fn labs_turns_off_every_way_to_the_project_agent_and_nothing_else() {
    for method in [
        "project_agent_ask",
        "project_agent_brief",
        "project_agent_consult",
        "project_agent_propose",
    ] {
        assert!(asks_the_project_agent(method), "{method}");
    }
    for method in ["board", "card", "comment", "health", "status"] {
        assert!(!asks_the_project_agent(method), "{method}");
    }
}
