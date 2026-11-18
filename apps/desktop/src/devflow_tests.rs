use devflow_engine::config::parse;
use devflow_engine::locate::DEVPIT as FILE;
use devflow_engine::Invalid;

use super::*;

fn valid(text: &str) -> State {
    State::Valid(text.to_owned(), Box::new(parse(text).expect("valid")))
}

#[test]
fn devflow_is_offered_only_where_the_file_and_the_click_both_say_so() {
    let on = valid("version: 1\nenabled: true\nstages:\n  context: { enabled: false }\n");
    assert_eq!(
        offered(true, &on, true),
        Offered::Active(vec![
            "briefing",
            "plan",
            "execution",
            "review",
            "completion"
        ])
    );
    // The file alone is a hand edit or a clone: the person has not clicked.
    assert_eq!(offered(true, &on, false), Offered::Setup);
    // Off until activated: no file, or a file that says enabled: false.
    assert_eq!(
        offered(
            true,
            &valid(
                "version: 1
enabled: false
"
            ),
            true
        ),
        Offered::Off
    );
    assert_eq!(offered(true, &State::Missing, false), Offered::Off);
    let invalid = State::Invalid(None, parse("version: 9\n").expect_err("invalid"));
    assert_eq!(offered(true, &invalid, true), Offered::Setup);
    // Off in Settings: what devpit offered before Devflow, file or not.
    assert_eq!(offered(false, &on, true), Offered::Off);
    assert_eq!(offered(false, &State::Missing, false), Offered::Off);
}

#[test]
fn the_state_is_said_in_the_skills_words() {
    let invalid = State::Invalid(
        None,
        Invalid {
            line: Some(2),
            field: "enabled".into(),
            message: "expected true or false".into(),
        },
    );
    assert_eq!(state_word(&State::Missing, false), "no file");
    assert_eq!(state_word(&invalid, false), "invalid");
    assert_eq!(state_word(&valid("version: 1\n"), false), "disabled");
    assert_eq!(
        state_word(&valid("version: 1\nenabled: true\n"), true),
        "active"
    );
}

fn project(id: &str, root: &Path) -> Project {
    serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": root.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": null,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project")
}

/// Sabotage: put the release checks back among each card's, and the preview
/// says `e2e` is run for every card's evidence.
#[test]
fn the_recommended_setup_runs_a_detected_e2e_at_release_and_judges_no_brief() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("Makefile"), "test:\n\ttrue\ne2e:\n\ttrue\n").expect("makefile");
    let here = project("prj_devflow_release", dir.path());
    let ask = |params: Value| {
        respond_from(None, "devflow_setup", &here, dir.path(), None, &params).expect("setup")
    };
    let names = |said: &Value, key: &str| -> Vec<String> {
        said[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|check| check["name"].as_str().map(str::to_owned))
            .collect()
    };
    let preview = ask(json!({ "preset": "recommended" }));
    assert_eq!(names(&preview, "checks"), ["unit"], "{preview}");
    assert_eq!(names(&preview, "release"), ["e2e"], "{preview}");
    let yaml = preview["yaml"].as_str().expect("yaml");
    assert!(yaml.contains("consolidate: none"), "{yaml}");
    assert!(!yaml.contains("devflow-brief"), "{yaml}");

    ask(json!({ "preset": "recommended", "apply": true }));
    let written = std::fs::read_to_string(dir.path().join(FILE)).expect("yaml");
    let config = parse(&written).expect("valid");
    let execution = &config.stages.execution;
    assert_eq!(
        execution
            .checks
            .iter()
            .map(|one| one.name.as_str())
            .collect::<Vec<_>>(),
        ["unit"]
    );
    assert_eq!(
        execution
            .release
            .iter()
            .map(|one| one.name.as_str())
            .collect::<Vec<_>>(),
        ["e2e"]
    );
    assert_eq!(
        config.review.consolidate,
        devflow_engine::config::Consolidate::Nothing
    );
    assert!(config.review.scale.is_some(), "{written}");
    assert_eq!(devflow_engine::jev::rubric_for(&config, "briefing"), None);
}

#[test]
fn a_setup_writes_only_when_applied_and_leaves_switching_on_to_the_click() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("Makefile"), "test:\n\ttrue\n").expect("makefile");
    let here = project("prj_devflow_setup", dir.path());
    let ask = |method: &str, params: Value| {
        respond_from(None, method, &here, dir.path(), None, &params).expect(method)
    };

    let status = ask("devflow_status", json!({}));
    assert_eq!(status["state"], "no file");
    assert_eq!(status["next"], "devpit_devflow_setup previews one");
    assert_eq!(status["recommended_checks"][0]["run"], "make test");
    assert_eq!(
        ask("devflow_offer", json!({}))["offer"],
        "off",
        "a project without Devflow gets none of its tools"
    );

    let preview = ask("devflow_setup", json!({ "preset": "recommended" }));
    assert_eq!(preview["applied"], false);
    assert!(!dir.path().join(".devpit").exists(), "a preview wrote");

    let applied = ask(
        "devflow_setup",
        json!({ "preset": "recommended", "apply": true }),
    );
    assert_eq!(applied["applied"], true);
    for file in ["devflow.yaml", ".gitignore", "scripts/check-unit.sh"] {
        assert!(
            dir.path().join(".devpit/devflow").join(file).is_file(),
            "{file}"
        );
    }
    for devpits in ["reviewers", "devflow-findings.schema.json"] {
        assert!(
            !dir.path().join(".devpit/devflow").join(devpits).exists(),
            "{devpits} copied"
        );
    }
    assert_eq!(applied["waiting_for_click"], json!(["enable"]));
    let status = ask("devflow_status", json!({}));
    assert_eq!(status["state"], "disabled");
    assert_eq!(status["next"], "waiting for Switch on in the app");

    // An agent cannot switch it on: the file stays as it was.
    let text = || std::fs::read_to_string(dir.path().join(FILE)).expect("yaml");
    let before = text();
    let refused = ask(
        "devflow_set",
        json!({ "path": "enabled", "value": true, "apply": true }),
    );
    assert_eq!(refused["applied"], false);
    assert_eq!(text(), before);
    assert_eq!(ask("devflow_offer", json!({}))["offer"], "off");

    let changed = ask(
        "devflow_set",
        json!({ "path": "review.block_on", "value": ["critical"], "apply": true }),
    );
    assert_eq!(changed["applied"], true);
    assert!(text().contains("block_on: [critical]"), "{}", text());

    // A check taken out leaves its script: the next preview says it goes.
    let script = dir.path().join(".devpit/devflow/scripts/check-unit.sh");
    let emptied = ask(
        "devflow_set",
        json!({ "path": "stages.execution.checks", "value": [] }),
    );
    let said: Vec<(&str, &str)> = emptied["files"]
        .as_array()
        .expect("files")
        .iter()
        .filter_map(|one| Some((one["path"].as_str()?, one["change"].as_str()?)))
        .collect();
    assert!(
        said.contains(&(".devpit/devflow/scripts/check-unit.sh", "remove")),
        "{said:?}"
    );
    assert!(script.is_file(), "a preview removed it");
}

/// The server lists from this answer: an orchestrator gets its own tools and
/// no flow to set up in its notes; a project, neither of an orchestrator's.
#[test]
fn the_offer_says_who_asks_and_an_orchestrator_is_offered_no_flow() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut here = project("prj_offer_seat", dir.path());
    let offer = |here: &Project| {
        respond_from(None, "devflow_offer", here, dir.path(), None, &json!({})).expect("offer")
    };
    let plain = offer(&here);
    assert_eq!(plain["offer"], "off");
    assert_eq!(plain["orchestrator"], false);
    assert!(plain["decisions"].is_boolean());
    here.orchestrator = Some("claude".to_owned());
    let orchestrating = offer(&here);
    assert_eq!(orchestrating["offer"], "off");
    assert_eq!(orchestrating["orchestrator"], true);
}

#[test]
fn every_devflow_method_is_answered_by_the_agent_api() {
    for method in METHODS {
        assert!(crate::agent_api::METHODS.contains(&method), "{method}");
    }
}

/// A team's layer as a bare repository, at tag v1, locking its reviewer.
fn team_layer(base: &Path) -> String {
    let run = |dir: &Path, args: &[&str]| {
        let ran = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.email=team@example.invalid",
                "-c",
                "user.name=Team",
            ])
            .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
            .args(args)
            .output()
            .expect("git");
        assert!(ran.status.success(), "git {args:?}");
    };
    let work = base.join("work");
    std::fs::create_dir_all(&work).expect("work");
    run(&work, &["init", "-q"]);
    std::fs::write(
        work.join("devflow.yaml"),
        "version: 2\nreview:\n  reviewers:\n    - agent: claude\n      persona: devflow-security\nlocked: [review.reviewers.devflow-security]\n",
    )
    .expect("layer");
    run(&work, &["add", "-A"]);
    run(&work, &["commit", "-qm", "one"]);
    run(&work, &["tag", "v1"]);
    let bare = base.join("acme").join("flow.git");
    run(base, &["clone", "-q", "--bare", "work", "acme/flow.git"]);
    bare.display().to_string()
}

#[test]
fn a_change_that_weakens_what_the_team_locked_is_refused_before_it_is_written() {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = team_layer(dir.path());
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join(".devpit/devflow")).expect("folder");
    let text = format!("version: 2\nextends:\n  - git: {url}\n    ref: v1\n");
    std::fs::write(root.join(FILE), &text).expect("yaml");
    let layer = devflow_engine::config::Extend {
        git: url,
        reference: "v1".into(),
    };
    let shown = devflow_engine::pinning::fetched(&root, &layer, &home()).expect("fetched");
    devflow_engine::pinning::approve(&root, &layer, &shown.to_commit, &home()).expect("approved");
    let here = project("prj_devflow_locked", &root);

    let refused = respond_from(
        None,
        "devflow_set",
        &here,
        &root,
        None,
        &json!({ "path": "review.reviewers", "value": [], "apply": true }),
    )
    .expect_err("the team locked it");
    assert!(refused.contains("locked by acme"), "{refused}");
    assert_eq!(
        std::fs::read_to_string(root.join(FILE)).expect("yaml"),
        text
    );
    let State::Valid(_, config) = read(&root) else {
        panic!("the layered file reads")
    };
    assert_eq!(config.review.reviewers[0].persona, "devflow-security");
}

#[test]
fn a_field_this_devpit_does_not_read_is_said_in_the_status_note() {
    let unread = devflow_engine::config::unread("version: 2\nreview:\n  quorum: 2\n");
    let note = unread_note(&unread).expect("a note");
    assert!(note.contains("review.quorum"), "{note}");
    assert!(note.contains("line 3"), "{note}");
    assert_eq!(unread_note(&[]), None);
}

#[test]
fn the_status_says_the_next_step_in_each_state() {
    let off = valid("version: 1\nenabled: false\n");
    let on = valid("version: 1\nenabled: true\n");
    let invalid = State::Invalid(None, parse("version: 9\n").expect_err("invalid"));
    let next = |state: &State, switched_on: bool, click_waits: bool| {
        let active = matches!(offered(true, state, switched_on), Offered::Active(_));
        next_step(state, true, switched_on, click_waits, active)
    };
    assert_eq!(
        next(&State::Missing, false, false),
        "devpit_devflow_setup previews one"
    );
    assert_eq!(next(&off, false, true), "waiting for Switch on in the app");
    assert_eq!(
        next(&off, false, false),
        "the person activates it on the Devflow screen"
    );
    assert_eq!(
        next(&on, true, false),
        "start with devpit_devflow_skill devflow-briefing"
    );
    // The click was made where devflow.yaml lives only on a feature branch.
    assert_eq!(
        next(&State::Missing, true, false),
        "commit .devpit/devflow/ on the default branch"
    );
    assert!(next(&invalid, false, false).contains("devpit_devflow_set"));
    assert!(next_step(&on, false, true, false, false).contains("Settings"));
}

/// The person clicked Switch on between an agent's preview and its apply:
/// the apply writes nothing and shows the setup as it now is.
#[test]
fn a_setup_apply_writes_nothing_when_the_setup_changed_since_the_preview() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("Makefile"), "test:\n\ttrue\n").expect("makefile");
    let here = project("prj_devflow_setup_moved", dir.path());
    let ask = |method: &str, params: Value| {
        respond_from(None, method, &here, dir.path(), None, &params).expect(method)
    };
    let setup = |apply: bool| {
        ask(
            "devflow_setup",
            json!({ "preset": "recommended", "apply": apply }),
        )
    };

    let unshown = setup(true);
    assert_eq!(unshown["applied"], false, "an apply with no preview");
    assert!(!dir.path().join(FILE).exists());
    assert_eq!(setup(true)["applied"], true, "the preview it just showed");

    let shown = setup(false);
    let store = crate::projects::store().expect("store");
    let clicked =
        devflow_engine::proposal::switched(&read(dir.path()), Ask::Enable).expect("clicked");
    devflow_engine::setup::write(dir.path(), &clicked.files).expect("written");
    asks::remember(&store, &here.id, Ask::Enable, true).expect("kept");
    let text = std::fs::read_to_string(dir.path().join(FILE)).expect("yaml");

    let moved = setup(true);
    assert_eq!(moved["applied"], false, "{moved}");
    assert_ne!(moved["yaml"], shown["yaml"]);
    let note = moved["note"].as_str().expect("a note");
    assert!(note.contains("enabled"), "{note}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join(FILE)).expect("yaml"),
        text
    );

    let applied = setup(true);
    assert_eq!(applied["applied"], true, "{applied}");
    let note = applied["note"].as_str().expect("a note");
    assert!(note.contains("Devflow is on here"), "{note}");
    assert!(!note.contains("still off"), "{note}");
    asks::remember(&store, &here.id, Ask::Enable, false).expect("forgotten");
}

#[test]
fn an_applied_setup_says_what_it_wrote() {
    assert!(applied_note(true, true, &[]).contains("Devflow is on here."));
    assert!(applied_note(false, false, &[]).contains(COMMIT_IT));
    assert!(applied_note(false, false, &["enable"]).contains("still off until the person clicks"));
    assert!(applied_note(true, false, &["enable"]).contains("stays off"));
    assert!(applied_note(false, false, &["enable", "enforce"]).contains("shadow"));
}

#[test]
fn no_target_is_in_force_until_the_person_confirms_one() {
    let config = parse(
        "version: 1\nsecurity:\n  targets: [\"http://localhost:8080\", \"http://127.0.0.1:9\"]\n",
    )
    .expect("valid");
    assert!(targets_in_force(&config).is_empty());
    let mut env = vec![(
        "DEVFLOW_SECURITY_TARGETS".to_owned(),
        "http://inherited".to_owned(),
    )];
    devflow_engine::security::with_targets(&mut env, &targets_in_force(&config));
    assert!(
        env.iter()
            .filter(|(name, _)| name.ends_with("_SECURITY_TARGETS"))
            .all(|(_, value)| value.is_empty()),
        "an inherited target never reaches a check: {env:?}"
    );
}
