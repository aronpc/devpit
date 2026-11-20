//! The verdict and naming helpers, tested beside them.

use super::*;

/// Every run speaks in a session of its own, shaped like the UUID the CLI
/// takes, and two runs of one card keep theirs apart on the card's links.
#[test]
fn two_runs_of_one_card_speak_in_different_sessions() {
    let first = fresh_session_id();
    assert_eq!(first.len(), 36, "{first} is not shaped like a uuid");
    assert_eq!(first.matches('-').count(), 4);
    assert_eq!(&first[14..15], "4", "{first} is not a version 4 uuid");
    assert!(
        matches!(&first[19..20], "8" | "9" | "a" | "b"),
        "{first} has no uuid variant"
    );

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
    for _ in 0..2 {
        let run = store.start_run(&card, &step, None).expect("start");
        store
            .set_run_session(&run, &fresh_session_id())
            .expect("session");
        // One after the other: a card holds one running run at a time.
        store
            .finish_run(&run, "ok", None, None, None, None)
            .expect("finish");
    }
    let links = store.card_links(&card).expect("links");
    assert_eq!(links.runs.len(), 2);
    assert_ne!(links.runs[0].session_id, links.runs[1].session_id);
}

/// The chain, end to end, with a real agent at the far side of it.
///
/// Every piece has a test of its own; this is the one that proves they add
/// up — a card in a store, a step configured against it, the agent named from
/// the catalogue on disk, and a run that comes back with an answer the schema
/// accepts and a cost above zero.
///
/// Opt-in behind `DEVPIT_LIVE_TURN`: it spends money.
#[test]
fn a_card_and_a_step_produce_a_real_answer() {
    if std::env::var_os("DEVPIT_LIVE_TURN").is_none() {
        eprintln!("skipped: set DEVPIT_LIVE_TURN=1 to spend money on this one");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[1].id.clone();

    let card = store
        .create_card(
            &project,
            &column,
            "Add a retry to the upload",
            "It fails on a flaky network and loses the file.",
        )
        .expect("card");

    // The agent the step names has to be one that is actually on disk.
    let catalogue = devpit_agentcli::read_agents(&agents_dir().expect("agents dir"));
    let Some(agent) = catalogue.agents.first() else {
        eprintln!("skipped: no agents seeded on this machine");
        return;
    };

    // The schema travels as a string inside the config, which is why it is
    // built separately rather than nested: the step stores JSON, and one of
    // its fields is itself JSON.
    let schema = r#"{"type":"object",
        "properties":{"verdict":{"type":"string"},"findings":{"type":"array"}},
        "required":["verdict","findings"]}"#;
    let config = serde_json::json!({
        "agent": agent.name,
        "prompt": "Reply with a verdict of \"approved\" and an empty findings array.",
        "schema": schema,
        "budgetUsd": 0.5,
        "model": "claude-haiku-4-5-20251001"
    })
    .to_string();

    let step_id = store
        .create_step(&project, "agent", "review it", &config, false)
        .expect("step");
    store
        .set_column_step(&column, Some(&step_id))
        .expect("attach");

    let step = devpit_rpc::Step {
        id: step_id.clone(),
        kind: devpit_rpc::StepKind::Agent,
        name: "review it".to_owned(),
        config,
        irreversible: false,
    };

    // Opened first, as the board does, so the step can record its folder and session.
    let run = store.start_run(&card, &step_id, None).expect("start");
    let mut streamed = 0;
    let finished = agent::run(
        &store,
        &card,
        &step,
        agent::Starting {
            run_id: &run,
            session_id: &fresh_session_id(),
            fixed: None,
        },
        |_| streamed += 1,
        |_| {},
    )
    .expect("the step ran");

    assert!(finished.ok, "the step failed: {}", finished.output);
    assert!(
        finished.cost_usd > 0.0,
        "a turn that cost nothing: {}",
        finished.output
    );
    assert!(streamed > 0, "nothing reached the card while it worked");

    // And the run lands on the card, which is where a person reads it.
    store
        .finish_run(
            &run,
            "ok",
            Some(&finished.output),
            Some(finished.cost_usd),
            Some(finished.duration_ms),
            None,
        )
        .expect("finish");

    assert_eq!(store.runs(&card).expect("runs").len(), 1);
    assert!(store.card_cost(&card).expect("cost") > 0.0);
}
