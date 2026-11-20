use devpit_rpc::{Credentials, Declared, EnvVar, Profile, ProfileSignIn, Reach};

use super::*;

fn seeded() -> (tempfile::TempDir, Store, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "a card", "")
        .expect("card");
    let step = store
        .create_step(&project, "agent", "review", "{}", false)
        .expect("step");
    (dir, store, card, step)
}

fn declared(token: &str) -> Declared {
    Declared {
        id: "work".to_owned(),
        label: "Work".to_owned(),
        base: "claude".to_owned(),
        command: "claude".to_owned(),
        args: Vec::new(),
        env: vec![EnvVar {
            name: "ANTHROPIC_API_KEY".to_owned(),
            value: token.to_owned(),
        }],
        models: Vec::new(),
    }
}

fn sources(given: &[(&str, &str, bool)]) -> Sources {
    let declared: Vec<_> = given
        .iter()
        .map(|(id, token, _)| {
            let mut declared = declared(token);
            declared.id = (*id).to_owned();
            declared.label = (*id).to_owned();
            declared
        })
        .collect();
    let profiles = given
        .iter()
        .map(|(id, token, enabled)| Profile {
            id: (*id).to_owned(),
            label: (*id).to_owned(),
            command: "claude".to_owned(),
            driver: "claude".to_owned(),
            path: Some("/usr/bin/claude".to_owned()),
            reach: Reach::Runnable,
            base: "claude".to_owned(),
            args: Vec::new(),
            env: vec![EnvVar {
                name: "ANTHROPIC_API_KEY".to_owned(),
                value: (*token).to_owned(),
            }],
            mine: true,
            models: vec!["sonnet".to_owned(), "opus".to_owned()],
            offered: Vec::new(),
            own_models: Vec::new(),
            efforts: Vec::new(),
            effort_default: None,
            metered: true,
            enabled: *enabled,
        })
        .collect();
    let credentials = declared
        .iter()
        .map(|profile| ProfileSignIn {
            id: profile.id.clone(),
            label: profile.label.clone(),
            base: profile.base.clone(),
            signed_in: Credentials::Saved,
        })
        .collect();
    Sources {
        home: "/home/me".to_owned(),
        declared,
        profiles,
        credentials,
        waiting: std::collections::HashMap::new(),
    }
}

fn run_row_contains(store: &Store, run_id: &str, text: &str) -> bool {
    let mut statement = store
        .conn()
        .prepare("SELECT * FROM run WHERE id = ?1")
        .expect("query");
    let columns = statement.column_count();
    statement
        .query_row([run_id], |row| {
            Ok((0..columns).any(|column| {
                row.get_ref(column)
                    .ok()
                    .and_then(|value| value.as_str().ok())
                    .is_some_and(|value| value.contains(text))
            }))
        })
        .expect("row")
}

#[test]
fn a_preview_uses_the_same_profile_check_as_a_run_start() {
    let available = sources(&[("work", "signed-in", true)]);
    let ready = available.preview("work", Some("sonnet"));
    assert_eq!(ready.profile, "work");
    assert_eq!(ready.model.as_deref(), Some("sonnet"));
    assert_eq!(ready.check, "Ready");

    let blocked = sources(&[("off", "signed-in", false)]).preview("off", Some("opus"));
    assert_eq!(blocked.check, "off is switched off");
}

#[test]
fn run_start_records_the_snapshot_and_models_before_launching_the_fixed_agent() {
    let (_dir, store, card, step) = seeded();
    let token = "sk-fake-never-store-this";
    let available = sources(&[("work", token, true)]);
    let begun = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "work",
        Some("sonnet"),
        &available,
    )
    .expect("begin");

    let attempt = store
        .attempt_of(&begun.run_id)
        .expect("read")
        .expect("attempt");
    assert!(attempt.pinned.is_some());
    assert_eq!(attempt.requested_model.as_deref(), Some("sonnet"));
    let fixed = begun.fixed.expect("ready to launch");
    assert_eq!(fixed.model.as_deref(), Some("sonnet"));
    assert_eq!(fixed.runner.env[0].1, token);

    record_effective_model(&store, &begun.run_id, Some("claude-sonnet-4-5")).expect("model");
    let recorded = store
        .attempt_of(&begun.run_id)
        .expect("read")
        .expect("attempt");
    assert_eq!(
        recorded.effective_model.as_deref(),
        Some("claude-sonnet-4-5")
    );
    assert!(!run_row_contains(&store, &begun.run_id, token));
}

#[test]
fn run_start_keeps_a_profile_token_out_of_every_run_column() {
    let (_dir, store, card, step) = seeded();
    let token = "sk-fake-column-sentinel";
    let available = sources(&[("work", token, true)]);
    let begun = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "work",
        Some("sonnet"),
        &available,
    )
    .expect("begin");

    assert!(
        !run_row_contains(&store, &begun.run_id, token),
        "the token reached a run column"
    );
}

#[test]
fn run_start_pauses_a_blocked_attempt_without_launching_or_reopening_the_previous_one() {
    let (_dir, store, card, step) = seeded();
    let available = sources(&[("first", "", true), ("blocked", "", false)]);
    let first = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "first",
        Some("sonnet"),
        &available,
    )
    .expect("first");
    store
        .finish_run(&first.run_id, "ok", None, None, None, None)
        .expect("finish");

    let blocked = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "blocked",
        Some("opus"),
        &available,
    )
    .expect("blocked");

    assert!(blocked.fixed.is_none(), "a paused attempt must not launch");
    let paused = store
        .attempt_of(&blocked.run_id)
        .expect("read")
        .expect("paused");
    assert_eq!(
        (paused.state.as_str(), paused.paused_because.as_deref()),
        ("paused", Some("blocked is switched off"))
    );
    assert_eq!(
        store.run_state(&first.run_id).expect("state").as_deref(),
        Some("ok")
    );
}

#[test]
fn run_start_automatically_switches_to_the_first_profile_that_passes() {
    let (_dir, store, card, step) = seeded();
    let project = store
        .project_id_of_card(&card)
        .expect("project")
        .expect("project id");
    store
        .set_preference(
            devpit_core::preference::PROJECT_SETTINGS,
            &format!(r#"{{"{project}":{{"automaticSwitch":true}}}}"#),
        )
        .expect("setting");
    let available = sources(&[
        ("blocked", "", false),
        ("first-ready", "", true),
        ("second-ready", "", true),
    ]);

    let begun = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "blocked",
        Some("opus"),
        &available,
    )
    .expect("begin");

    assert!(begun.fixed.is_some(), "the automatic attempt launches");
    let attempt = store
        .attempt_of(&begun.run_id)
        .expect("read")
        .expect("attempt");
    let pinned: crate::run_pin::PinnedProfile =
        serde_json::from_str(attempt.pinned.as_deref().expect("snapshot")).expect("snapshot");
    assert_eq!(attempt.cause.as_deref(), Some("automatic"));
    assert_eq!(pinned.id, "first-ready");
    let paused = store
        .attempt_of(attempt.previous_attempt.as_deref().expect("previous"))
        .expect("read")
        .expect("paused");
    assert_eq!(paused.state, "paused");
}

#[test]
fn run_start_stays_paused_when_no_profile_passes_the_check() {
    let (_dir, store, card, step) = seeded();
    let project = store
        .project_id_of_card(&card)
        .expect("project")
        .expect("project id");
    store
        .set_preference(
            devpit_core::preference::PROJECT_SETTINGS,
            &format!(r#"{{"{project}":{{"automaticSwitch":true}}}}"#),
        )
        .expect("setting");
    let available = sources(&[("blocked", "", false), ("also-blocked", "", false)]);

    let begun = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "blocked",
        Some("opus"),
        &available,
    )
    .expect("begin");

    assert!(begun.paused);
    assert_eq!(store.runs(&card).expect("runs").len(), 1);
}

#[test]
fn run_start_retries_with_the_previous_attempts_snapshot_and_model() {
    let (_dir, store, card, step) = seeded();
    let available = sources(&[("old", "", true), ("current", "", true)]);
    let first = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "old",
        Some("sonnet"),
        &available,
    )
    .expect("first");
    store
        .finish_run(&first.run_id, "failed", None, None, None, None)
        .expect("finish");

    let retried = begin_profiled(
        &store,
        &card,
        &step,
        None,
        "current",
        Some("opus"),
        &available,
    )
    .expect("retry");

    let attempt = store
        .attempt_of(&retried.run_id)
        .expect("read")
        .expect("attempt");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref()
        ),
        (Some(2), Some(first.run_id.as_str()), Some("retry"))
    );
    let fixed = retried.fixed.expect("ready");
    let pinned: crate::run_pin::PinnedProfile =
        serde_json::from_str(attempt.pinned.as_deref().expect("pinned")).expect("snapshot");
    assert_eq!(pinned.id, "old");
    assert_eq!(fixed.runner.program, "claude");
    assert_eq!(fixed.model.as_deref(), Some("sonnet"));
}
