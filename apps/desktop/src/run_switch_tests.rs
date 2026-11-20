use std::collections::HashMap;

use devpit_core::store::{Asked, Carried, RunAsk};
use devpit_core::Store;
use devpit_rpc::{Credentials, Declared, EnvVar, Profile, ProfileSignIn, Reach};

use super::*;

fn seeded() -> (tempfile::TempDir, Store, String, String, String) {
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
    (dir, store, card, step, column)
}

fn sources() -> crate::run_start::Sources {
    let given = [
        ("old", "Old account", "sonnet", true),
        ("work", "Work account", "opus", true),
        ("off", "Switched off", "haiku", false),
    ];
    let declared = given
        .iter()
        .map(|(id, label, model, _)| Declared {
            id: (*id).to_owned(),
            label: (*label).to_owned(),
            base: "claude".to_owned(),
            command: "claude".to_owned(),
            args: Vec::new(),
            env: vec![
                EnvVar {
                    name: "ANTHROPIC_API_KEY".to_owned(),
                    value: "fake-test-token".to_owned(),
                },
                EnvVar {
                    name: "ANTHROPIC_MODEL".to_owned(),
                    value: (*model).to_owned(),
                },
            ],
            models: Vec::new(),
        })
        .collect::<Vec<_>>();
    let profiles = given
        .iter()
        .map(|(id, label, _, enabled)| Profile {
            id: (*id).to_owned(),
            label: (*label).to_owned(),
            command: "claude".to_owned(),
            driver: "claude".to_owned(),
            path: Some("/usr/bin/claude".to_owned()),
            reach: Reach::Runnable,
            base: "claude".to_owned(),
            args: Vec::new(),
            env: Vec::new(),
            mine: true,
            models: vec!["sonnet".to_owned(), "opus".to_owned()],
            offered: Vec::new(),
            own_models: Vec::new(),
            efforts: Vec::new(),
            effort_default: None,
            metered: true,
            enabled: *enabled,
        })
        .collect::<Vec<_>>();
    let credentials = declared
        .iter()
        .map(|profile| ProfileSignIn {
            id: profile.id.clone(),
            label: profile.label.clone(),
            base: profile.base.clone(),
            signed_in: Credentials::Saved,
        })
        .collect();
    crate::run_start::Sources {
        home: "/home/me".to_owned(),
        declared,
        profiles,
        credentials,
        waiting: HashMap::new(),
    }
}

fn paused(
    store: &Store,
    card: &str,
    step: &str,
    column: &str,
    sources: &crate::run_start::Sources,
) -> String {
    store
        .begin_attempt(
            RunAsk::Step {
                card: card.to_owned(),
                step: step.to_owned(),
            },
            Some(column),
            sources.current("old", None),
            Some("Old account's 5-hour quota is exhausted"),
        )
        .expect("paused")
}

#[test]
fn run_switch_lists_only_profiles_that_pass_the_paused_steps_check() {
    let (_dir, store, card, step, column) = seeded();
    let sources = sources();
    let paused = paused(&store, &card, &step, &column, &sources);

    let listed = options_with(&store, &paused, &sources).expect("options");

    assert_eq!(listed.reason, "Old account's 5-hour quota is exhausted");
    assert_eq!(
        listed
            .profiles
            .iter()
            .map(|profile| (
                profile.id.as_str(),
                profile.label.as_str(),
                profile.model.as_deref()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("old", "Old account", Some("sonnet")),
            ("work", "Work account", Some("opus")),
        ]
    );
}

#[test]
fn run_switch_opens_the_next_click_attempt_under_the_chosen_profile_and_model() {
    let (_dir, store, card, step, column) = seeded();
    let sources = sources();
    let paused = paused(&store, &card, &step, &column, &sources);

    let switched = switch_with(&store, &paused, "work", &sources).expect("switch");

    assert_eq!(switched.card_id, card);
    assert_eq!(switched.step.id, step);
    assert_eq!(switched.from_column.as_deref(), Some(column.as_str()));
    assert!(
        switched.begun.fixed.is_some(),
        "the switched attempt launches"
    );
    let attempt = store
        .attempt_of(&switched.begun.run_id)
        .expect("read")
        .expect("attempt");
    let pinned: crate::run_pin::PinnedProfile =
        serde_json::from_str(attempt.pinned.as_deref().expect("snapshot")).expect("snapshot");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref(),
            pinned.label.as_str(),
            attempt.requested_model.as_deref(),
        ),
        (
            Some(2),
            Some(paused.as_str()),
            Some("click"),
            "Work account",
            Some("opus"),
        )
    );
    let listed =
        crate::board::runs_of(&store, &card, std::slice::from_ref(&switched.step)).expect("listed");
    assert_eq!(
        (listed[0].profile.as_deref(), listed[0].model.as_deref()),
        (Some("Work account"), Some("opus"))
    );
}

#[test]
fn run_switch_records_the_profile_that_actually_executes() {
    let (_dir, store, card, step, column) = seeded();
    let sources = sources();
    let paused = paused(&store, &card, &step, &column, &sources);
    let switched = switch_with(&store, &paused, "work", &sources).expect("switch");

    crate::run_launch::record_whose_run(
        &store,
        &switched.begun.run_id,
        &switched.step,
        Asked::Card,
        None,
        switched.begun.fixed.as_ref(),
    )
    .expect("record");

    let whose = store
        .whose_run(&switched.begun.run_id)
        .expect("read")
        .expect("run");
    assert_eq!(
        whose.carried,
        Carried::Agent {
            profile: Some("work".to_owned())
        }
    );
}

#[test]
fn run_switch_refuses_a_blocked_profile_and_a_second_successor() {
    let (_dir, store, card, step, column) = seeded();
    let sources = sources();
    let paused = paused(&store, &card, &step, &column, &sources);

    let Err(refused) = switch_with(&store, &paused, "off", &sources) else {
        panic!("off is refused");
    };
    assert_eq!(refused.message, "Switched off is switched off");
    assert_eq!(store.runs(&card).expect("runs").len(), 1);

    switch_with(&store, &paused, "work", &sources).expect("first switch");
    let Err(second) = switch_with(&store, &paused, "old", &sources) else {
        panic!("one successor");
    };
    assert_eq!(second.code, devpit_rpc::ErrorCode::Busy);
    assert_eq!(store.runs(&card).expect("runs").len(), 2);
}
