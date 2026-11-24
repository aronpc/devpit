//! Every row of the transition table, as a test: whether a run is born, its
//! number, the attempt before it, and the cause.

use crate::store::{AttemptCarry, RunAsk, Store, StoreError, SwitchKind};

fn seeded() -> (tempfile::TempDir, Store, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    let project = store.add_project(&root, None).expect("project");
    store.ensure_board(&project).expect("board");
    let columns = store.columns(&project).expect("columns");
    let card = store
        .create_card(&project, &columns[0].id, "a card", "")
        .expect("card");
    let step = store
        .create_step(&project, "agent", "review", "{}", false)
        .expect("step");
    (dir, store, card, step)
}

/// The step was asked for, plain: a drag, a play, the auto chain.
fn asked(card: &str, step: &str) -> RunAsk {
    RunAsk::Step {
        card: card.to_owned(),
        step: step.to_owned(),
    }
}

/// A retrato and a model, as the caller chose them.
fn under(pinned: &str, model: &str) -> AttemptCarry {
    AttemptCarry {
        pinned: Some(pinned.to_owned()),
        requested_model: Some(model.to_owned()),
    }
}

/// Row 1: the first execution of a step on a card is attempt 1, continuing
/// nothing, caused `start`, carrying the retrato and model it was given.
#[test]
fn the_first_ask_of_a_step_is_attempt_one_caused_start() {
    let (_dir, store, card, step) = seeded();
    let run = store
        .begin_attempt(
            asked(&card, &step),
            None,
            under("the glm retrato", "glm-4.6"),
            None,
        )
        .expect("begin");
    let attempt = store.attempt_of(&run).expect("read").expect("there");
    assert_eq!(attempt.state, "running");
    assert_eq!(attempt.attempt, Some(1));
    assert_eq!(attempt.previous_attempt, None);
    assert_eq!(attempt.cause.as_deref(), Some("start"));
    assert_eq!(attempt.pinned.as_deref(), Some("the glm retrato"));
    assert_eq!(attempt.requested_model.as_deref(), Some("glm-4.6"));
    assert_eq!(attempt.paused_because, None);
}

/// Row 2: an `ok` attempt closed the cycle, and so did a `cancelled` one —
/// asking again after either is a new first attempt, not a continuation of a
/// finished one.
#[test]
fn an_ask_after_ok_or_cancelled_starts_over_at_attempt_one() {
    let (_dir, store, card, step) = seeded();
    let first = store
        .begin_attempt(asked(&card, &step), None, under("a", "m"), None)
        .expect("begin");
    store
        .finish_run(&first, "ok", None, None, None, None)
        .expect("finish");

    let second = store
        .begin_attempt(asked(&card, &step), None, under("b", "m"), None)
        .expect("begin");
    let attempt = store.attempt_of(&second).expect("read").expect("there");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref()
        ),
        (Some(1), None, Some("start"))
    );

    store
        .finish_run(&second, "cancelled", None, None, None, None)
        .expect("finish");
    let third = store
        .begin_attempt(asked(&card, &step), None, under("c", "m"), None)
        .expect("begin");
    let attempt = store.attempt_of(&third).expect("read").expect("there");
    assert_eq!(
        (attempt.attempt, attempt.cause.as_deref()),
        (Some(1), Some("start")),
        "a cancelled attempt is as closed a cycle as an ok one"
    );
}

/// Row 3: when the check blocks a start, the run is born `paused` with the
/// reason — and the transition it was stays: the cause is still the cause,
/// the number still the number. The pause is a state, not a different event.
#[test]
fn a_blocked_start_is_born_paused_with_its_reason_and_the_transition_intact() {
    let (_dir, store, card, step) = seeded();
    let run = store
        .begin_attempt(
            asked(&card, &step),
            None,
            under("a", "m"),
            Some("the glm account has no quota left"),
        )
        .expect("begin");
    let attempt = store.attempt_of(&run).expect("read").expect("there");
    assert_eq!(attempt.state, "paused");
    assert_eq!(
        attempt.paused_because.as_deref(),
        Some("the glm account has no quota left")
    );
    assert_eq!(
        (attempt.attempt, attempt.cause.as_deref()),
        (Some(1), Some("start"))
    );
}

/// Row 4: play on a step whose last attempt is `paused`, `failed` or `lost`
/// is a retry — attempt N+1, chained to N, carrying N's own retrato and
/// model, whatever the caller happened to pass.
#[test]
fn play_after_paused_failed_or_lost_is_a_retry_chained_to_the_last_attempt() {
    let (_dir, store, card, step) = seeded();
    let first = store
        .begin_attempt(
            asked(&card, &step),
            None,
            under("the glm retrato", "glm-4.6"),
            None,
        )
        .expect("begin");
    store
        .finish_run(&first, "failed", None, None, None, None)
        .expect("finish");

    let of_failed = store
        .begin_attempt(
            asked(&card, &step),
            None,
            under("not this one", "nor this"),
            None,
        )
        .expect("begin");
    let attempt = store.attempt_of(&of_failed).expect("read").expect("there");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref()
        ),
        (Some(2), Some(first.as_str()), Some("retry"))
    );
    assert_eq!(
        (
            attempt.pinned.as_deref(),
            attempt.requested_model.as_deref()
        ),
        (Some("the glm retrato"), Some("glm-4.6")),
        "a retry carries the last attempt's own retrato and model"
    );

    store
        .finish_run(&of_failed, "lost", None, None, None, None)
        .expect("finish");
    let of_lost = store
        .begin_attempt(asked(&card, &step), None, under("no", "no"), None)
        .expect("begin");
    let attempt = store.attempt_of(&of_lost).expect("read").expect("there");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref()
        ),
        (Some(3), Some(of_failed.as_str()), Some("retry"))
    );

    // The paused ending: a retry that the check blocks is born paused, and
    // the next play retries *it*.
    store
        .finish_run(&of_lost, "failed", None, None, None, None)
        .expect("finish");
    let paused = store
        .begin_attempt(
            asked(&card, &step),
            None,
            under("no", "no"),
            Some("no quota"),
        )
        .expect("begin");
    let of_paused = store
        .begin_attempt(asked(&card, &step), None, under("no", "no"), None)
        .expect("begin");
    let attempt = store.attempt_of(&of_paused).expect("read").expect("there");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref()
        ),
        (Some(5), Some(paused.as_str()), Some("retry"))
    );
}

/// Row 5: a click switching account or model during a pause — attempt N+1,
/// chained to the paused one, caused `click`, under the retrato and model
/// the person chose.
#[test]
fn a_click_during_a_pause_switches_to_the_new_retrato_as_the_next_attempt() {
    let (_dir, store, card, step) = seeded();
    let paused = store
        .begin_attempt(
            asked(&card, &step),
            None,
            under("the glm retrato", "glm-4.6"),
            Some("the glm account has no quota left"),
        )
        .expect("begin");
    let switched = store
        .begin_attempt(
            RunAsk::Switch {
                from: paused.clone(),
                how: SwitchKind::Click,
            },
            None,
            under("the codex retrato", "g-5"),
            None,
        )
        .expect("begin");
    let attempt = store.attempt_of(&switched).expect("read").expect("there");
    assert_eq!(attempt.state, "running");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref()
        ),
        (Some(2), Some(paused.as_str()), Some("click"))
    );
    assert_eq!(
        (
            attempt.pinned.as_deref(),
            attempt.requested_model.as_deref()
        ),
        (Some("the codex retrato"), Some("g-5"))
    );
}

/// Row 6: the project's own switch, with the option on — same shape as the
/// click, caused `automatic`.
#[test]
fn an_automatic_switch_is_the_next_attempt_caused_automatic() {
    let (_dir, store, card, step) = seeded();
    let paused = store
        .begin_attempt(asked(&card, &step), None, under("a", "m"), Some("blocked"))
        .expect("begin");
    let switched = store
        .begin_attempt(
            RunAsk::Switch {
                from: paused.clone(),
                how: SwitchKind::Automatic,
            },
            None,
            under("the next account", "m2"),
            None,
        )
        .expect("begin");
    let attempt = store.attempt_of(&switched).expect("read").expect("there");
    assert_eq!(
        (
            attempt.attempt,
            attempt.previous_attempt.as_deref(),
            attempt.cause.as_deref()
        ),
        (Some(2), Some(paused.as_str()), Some("automatic"))
    );
    assert_eq!(
        (
            attempt.pinned.as_deref(),
            attempt.requested_model.as_deref()
        ),
        (Some("the next account"), Some("m2"))
    );
}

/// A switch asked of a run that is not paused has nothing to switch: the
/// pause is what offers it.
#[test]
fn a_switch_away_from_a_run_that_is_not_paused_is_refused() {
    let (_dir, store, card, step) = seeded();
    let running = store
        .begin_attempt(asked(&card, &step), None, under("a", "m"), None)
        .expect("begin");
    assert!(matches!(
        store.begin_attempt(
            RunAsk::Switch {
                from: running.clone(),
                how: SwitchKind::Click,
            },
            None,
            under("b", "m"),
            None,
        ),
        Err(StoreError::NotPaused)
    ));
}

/// Row 7, and the rule under it: one attempt, one successor. The second write
/// chained to the same attempt is refused, and creates no run.
#[test]
fn an_attempt_gets_one_successor_and_the_second_write_is_refused() {
    let (_dir, store, card, step) = seeded();
    let paused = store
        .begin_attempt(asked(&card, &step), None, under("a", "m"), Some("blocked"))
        .expect("begin");
    let _first = store
        .begin_attempt(
            RunAsk::Switch {
                from: paused.clone(),
                how: SwitchKind::Click,
            },
            None,
            under("b", "m"),
            None,
        )
        .expect("the first switch goes through");

    let before = store.runs(&card).expect("runs").len();
    assert!(matches!(
        store.begin_attempt(
            RunAsk::Switch {
                from: paused.clone(),
                how: SwitchKind::Automatic,
            },
            None,
            under("c", "m"),
            None,
        ),
        Err(StoreError::AlreadyContinued)
    ));
    assert_eq!(
        store.runs(&card).expect("runs").len(),
        before,
        "the refused write created no run"
    );
}

/// The index beneath the function's check: a second row chained to the same
/// attempt does not go in, whatever wrote it. The forced row is born
/// `paused`, so it holds no one-run slot — the only rule that can refuse it
/// is the one-successor index this is here to witness.
///
/// Sabotage: drop `run_one_successor` from the migration and this is the test
/// that notices — the function's own refusal would still hold, which is
/// exactly why the index needs its own witness.
#[test]
fn the_database_itself_refuses_a_second_successor_row() {
    let (_dir, store, card, step) = seeded();
    let paused = store
        .begin_attempt(asked(&card, &step), None, under("a", "m"), Some("blocked"))
        .expect("begin");
    let _successor = store
        .begin_attempt(
            RunAsk::Switch {
                from: paused.clone(),
                how: SwitchKind::Click,
            },
            None,
            under("b", "m"),
            None,
        )
        .expect("begin");
    let refused = store.conn().execute(
        "INSERT INTO run (id, card_id, step_id, state, started_at, attempt, previous_attempt, cause) \
         VALUES ('run_forced', ?1, ?2, 'paused', 0, 3, ?3, 'retry')",
        rusqlite::params![card, step, paused],
    );
    assert!(
        refused.is_err(),
        "the database let a second successor of the same attempt in"
    );
}

/// A paused run does not hold the card's one-run slot: that index counts runs
/// that are going, and a pause is waiting. The next attempt begins where a
/// running run would have been refused.
#[test]
fn a_paused_run_does_not_hold_the_cards_one_run_slot() {
    let (_dir, store, card, step) = seeded();
    store
        .begin_attempt(asked(&card, &step), None, under("a", "m"), Some("blocked"))
        .expect("begin");
    store
        .start_run(&card, &step, None)
        .expect("a card with a paused run can still run");
}

/// A pause is not work in flight, and it survives a restart: the launch sweep
/// closes runs whose process is gone, and a pause waiting for a person is not
/// one of them.
#[test]
fn a_pause_is_not_work_in_flight_and_survives_the_launch_sweep() {
    let (_dir, store, card, step) = seeded();
    let paused = store
        .begin_attempt(asked(&card, &step), None, under("a", "m"), Some("blocked"))
        .expect("begin");
    assert!(
        store.running_runs().expect("running").is_empty(),
        "a paused run is not going"
    );
    assert!(
        store.close_abandoned_runs().expect("sweep").is_empty(),
        "the sweep closed a pause that was waiting for a person"
    );
    assert_eq!(
        store
            .attempt_of(&paused)
            .expect("read")
            .expect("there")
            .state,
        "paused"
    );
}

/// The work migration 36 does over `rows` old runs, in SQLite's own count of
/// the steps its statements took, and the table `previous_attempt` ends up
/// pointing at.
fn migration_36_over(rows: usize) -> (i64, String) {
    use rusqlite::fallible_iterator::FallibleIterator;

    let dir = tempfile::tempdir().expect("tempdir");
    let conn = rusqlite::Connection::open(dir.path().join("state.db")).expect("open");
    super::super::migrations::run_up_to(&conn, 35).expect("migrate to 35");
    conn.execute_batch(
        "BEGIN; \
         INSERT INTO trust_workspace (id, slug, label, color, vault_namespace, created_at) \
         VALUES ('tw', 'tw', 'TW', '#000000', 'ns', 0); \
         INSERT INTO project (id, trust_workspace_id, name, root_path, created_at) \
         VALUES ('p', 'tw', 'p', '/p', 0); \
         INSERT INTO step (id, project_id, kind, name, config, irreversible, created_at) \
         VALUES ('s', 'p', 'command', 'tests', '{}', 0, 0); \
         INSERT INTO board_column (id, project_id, name, position, created_at) \
         VALUES ('col', 'p', 'todo', 0, 0); \
         INSERT INTO card (id, project_id, column_id, title, position, created_at, updated_at) \
         VALUES ('c', 'p', 'col', 'a card', 0, 0, 0);",
    )
    .expect("seed");
    for at in 0..rows {
        conn.execute(
            "INSERT INTO run (id, card_id, step_id, state, started_at, ended_at) \
             VALUES (?1, 'c', 's', 'ok', ?2, ?2)",
            rusqlite::params![format!("r{at:06}"), at as i64],
        )
        .expect("old run");
    }
    conn.execute_batch("COMMIT").expect("commit");

    // As `Store::open` runs it: foreign keys on, inside one transaction.
    conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
    conn.execute_batch("BEGIN IMMEDIATE").expect("begin");
    let mut steps = 0_i64;
    let mut batch = rusqlite::Batch::new(&conn, super::super::migrations::sql_of(36));
    while let Some(mut statement) = batch.next().expect("parse") {
        statement.execute([]).expect("apply");
        steps += i64::from(statement.get_status(rusqlite::StatementStatus::VmStep));
    }
    conn.execute_batch("COMMIT").expect("commit");

    let target: String = conn
        .query_row(
            "SELECT \"table\" FROM pragma_foreign_key_list('run') WHERE \"from\" = 'previous_attempt'",
            [],
            |row| row.get(0),
        )
        .expect("the chain's key");
    (steps, target)
}

/// Migration 36 rebuilds `run` once per old row, not once per pair of rows:
/// a key from the new table to the old one made `DROP TABLE run` scan the
/// new table for every row it deleted, and a few thousand runs held the
/// write lock for most of a minute.
#[test]
fn migration_36_grows_with_the_runs_not_with_their_square() {
    let (small, target) = migration_36_over(400);
    let (large, _) = migration_36_over(1_600);
    assert_eq!(target, "run", "the chain points at the rebuilt table");
    // Four times the rows: about four times the work when it is linear, about
    // sixteen when every deleted row scans every new one.
    assert!(
        large < small * 8,
        "400 runs took {small} steps and 1600 took {large}"
    );
}

/// Migration 36 over rows an older devpit wrote: they keep everything they
/// had — state, and the dismissal findings pinned to them — and gain the
/// attempt columns as NULL, which reads as `unknown` and never as an answer.
/// A run born `paused` afterwards is the CHECK the rebuild was for.
#[test]
fn migration_36_keeps_old_rows_their_dismissals_and_accepts_paused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");
    let old = rusqlite::Connection::open(&path).expect("open");
    super::super::migrations::run_up_to(&old, 35).expect("migrate to 35");
    old.execute_batch(
        "INSERT INTO trust_workspace (id, slug, label, color, vault_namespace, created_at) \
         VALUES ('tw', 'tw', 'TW', '#000000', 'ns', 0); \
         INSERT INTO project (id, trust_workspace_id, name, root_path, created_at) \
         VALUES ('p', 'tw', 'p', '/p', 0); \
         INSERT INTO step (id, project_id, kind, name, config, irreversible, created_at) \
         VALUES ('s', 'p', 'command', 'tests', '{}', 0, 0); \
         INSERT INTO board_column (id, project_id, name, position, created_at) \
         VALUES ('col', 'p', 'todo', 0, 0); \
         INSERT INTO card (id, project_id, column_id, title, position, created_at, updated_at) \
         VALUES ('c', 'p', 'col', 'a card', 0, 0, 0); \
         INSERT INTO run (id, card_id, step_id, state, started_at) \
         VALUES ('r1', 'c', 's', 'ok', 10); \
         INSERT INTO finding_dismissal (run_id, at, dismissed_at) VALUES ('r1', 1, 2);",
    )
    .expect("seed the old shape");
    drop(old);

    let store = Store::open(&path).expect("migrate");
    let attempt = store.attempt_of("r1").expect("read").expect("there");
    assert_eq!(attempt.state, "ok");
    assert_eq!(
        attempt.attempt, None,
        "an old row's number is unknown, not 1"
    );
    assert_eq!(attempt.previous_attempt, None);
    assert_eq!(attempt.cause, None);
    assert_eq!(attempt.pinned, None);
    assert_eq!(attempt.requested_model, None);
    assert_eq!(attempt.effective_model, None);

    let dismissals: i64 = store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM finding_dismissal WHERE run_id = 'r1'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(
        dismissals, 1,
        "the rebuild of run took the dismissal findings with it"
    );

    let paused = store
        .begin_attempt(
            asked("c", "s"),
            None,
            AttemptCarry::default(),
            Some("blocked"),
        )
        .expect("born paused");
    assert_eq!(
        store
            .attempt_of(&paused)
            .expect("read")
            .expect("there")
            .state,
        "paused"
    );
    assert_eq!(
        store.runs("c").expect("runs").len(),
        2,
        "the old row and the new one, both readable"
    );
}
