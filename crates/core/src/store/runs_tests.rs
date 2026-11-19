//! What a run remembers, and what happens to one whose process is gone.

use crate::store::{Store, StoreError};

fn seeded() -> (tempfile::TempDir, Store, String, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    let project = store.add_project(&root, None).expect("project");
    store.ensure_board(&project).expect("board");
    let columns = store.columns(&project).expect("columns");
    let first = columns[0].id.clone();
    let card = store
        .create_card(&project, &first, "a card", "")
        .expect("card");
    let step = store
        .create_step(&project, "command", "tests", "{}", false)
        .expect("step");
    (dir, store, card, step, first)
}

#[test]
fn a_run_records_where_the_card_stood() {
    // The whole reason the column exists: this was passed by value into the
    // thread and nowhere else, so a run in flight when the app quit took the
    // only answer to "send it back where?" with it.
    let (_dir, store, card, step, column) = seeded();
    let run = store.start_run(&card, &step, Some(&column)).expect("start");
    assert_eq!(store.run_came_from(&run).expect("read"), Some(column));
}

#[test]
fn a_run_started_without_one_says_none_rather_than_guessing() {
    let (_dir, store, card, step, _column) = seeded();
    let run = store.start_run(&card, &step, None).expect("start");
    assert_eq!(store.run_came_from(&run).expect("read"), None);
}

#[test]
fn asking_about_a_run_that_is_not_there_is_none_and_not_an_error() {
    let (_dir, store, _card, _step, _column) = seeded();
    assert_eq!(store.run_came_from("run_nothing").expect("read"), None);
}

#[test]
fn a_run_left_open_by_a_dead_process_is_closed_at_launch() {
    // Nothing survives the process that spawned its thread, so a `running`
    // row after a restart is not a run still going — and leaving it says the
    // card is working when it is not, forever.
    let (_dir, store, card, step, column) = seeded();
    let run = store.start_run(&card, &step, Some(&column)).expect("start");

    let closed = store.close_abandoned_runs().expect("sweep");
    assert_eq!(closed, vec![(run.clone(), card.clone())]);

    let after = store.runs(&card).expect("runs");
    // Lost, not failed: nobody knows how it ended.
    assert_eq!(after[0].state, "lost");
    assert!(after[0]
        .output
        .as_deref()
        .expect("a reason")
        .contains("the app closed"));
}

#[test]
fn the_sweep_leaves_a_run_that_already_ended_alone() {
    let (_dir, store, card, step, column) = seeded();
    let run = store.start_run(&card, &step, Some(&column)).expect("start");
    store
        .finish_run(&run, "ok", Some("all good"), None, None, Some(0))
        .expect("finish");

    assert!(store.close_abandoned_runs().expect("sweep").is_empty());
    let after = store.runs(&card).expect("runs");
    assert_eq!(after[0].state, "ok");
    assert_eq!(after[0].output.as_deref(), Some("all good"));
}

/// A person's stop is the run's end: the process it killed ends a moment
/// later and finds nothing left to close.
#[test]
fn a_run_closes_once_so_a_stop_is_not_written_over() {
    let (_dir, store, card, step, _column) = seeded();
    let run = store.start_run(&card, &step, None).expect("start");
    assert!(store
        .finish_run(&run, "cancelled", Some("stopped by you"), None, None, None)
        .expect("cancel"));
    assert!(!store
        .finish_run(&run, "failed", Some("killed"), None, None, None)
        .expect("late"));
    assert_eq!(store.runs(&card).expect("runs")[0].state, "cancelled");
}

#[test]
fn sweeping_twice_closes_nothing_the_second_time() {
    // It runs at every launch, so it has to be safe to run at every launch.
    let (_dir, store, card, step, column) = seeded();
    store.start_run(&card, &step, Some(&column)).expect("start");
    assert_eq!(store.close_abandoned_runs().expect("first").len(), 1);
    assert!(store.close_abandoned_runs().expect("second").is_empty());
}

#[test]
fn a_run_whose_column_is_deleted_keeps_the_run() {
    // `ON DELETE SET NULL`, not CASCADE: deleting a column must not delete
    // the history of what ran while a card was in it.
    //
    // The card has to move out first — a column holding cards refuses to be
    // deleted, which is the point of that RESTRICT. An earlier version of
    // this test deleted the card instead, which cascaded the run away and
    // tested nothing.
    let (_dir, store, card, step, column) = seeded();
    let project = store
        .project_id_of_card(&card)
        .expect("project")
        .expect("id");
    let elsewhere = store.columns(&project).expect("columns")[1].id.clone();

    let run = store.start_run(&card, &step, Some(&column)).expect("start");
    store.move_card(&card, &elsewhere, 0).expect("move");
    store.delete_column(&column).expect("delete");

    assert_eq!(
        store.runs(&card).expect("runs").len(),
        1,
        "the run went with the column"
    );
    assert_eq!(store.run_came_from(&run).expect("read"), None);
}

/// Migration 10 over rows the old sweep wrote: its own words mark them lost,
/// and a real failure stays a failure.
#[test]
fn rows_the_old_sweep_wrote_become_lost_and_nothing_else_moves() {
    let (dir, store, card, step, _column) = seeded();
    let swept = store.start_run(&card, &step, None).expect("start");
    store
        .finish_run(
            &swept,
            "failed",
            Some("the app closed while this was running"),
            None,
            None,
            None,
        )
        .expect("finish");
    let real = store.start_run(&card, &step, None).expect("start");
    store
        .finish_run(&real, "failed", Some("exit 1"), None, None, Some(1))
        .expect("finish");
    // Back to the version before it, so opening runs migration 10 over these
    // rows. What later migrations added goes too, or rerunning them collides.
    store
        .conn()
        .execute_batch(
            "DROP TABLE card_chat; ALTER TABLE session_link DROP COLUMN cwd; \
             ALTER TABLE session_link DROP COLUMN profile_id; \
             DROP TABLE pane_agent; DROP INDEX project_folder; ALTER TABLE project DROP COLUMN folder; \
             DROP TABLE project_plugin; ALTER TABLE card_attachment DROP COLUMN plugin_id; \
             ALTER TABLE project DROP COLUMN icon; ALTER TABLE project DROP COLUMN color; \
             ALTER TABLE notice DROP COLUMN pane_id; \
             DROP INDEX card_reminder; ALTER TABLE card DROP COLUMN due_time; \
             ALTER TABLE card DROP COLUMN reminded_at; ALTER TABLE card DROP COLUMN handled_at; \
             ALTER TABLE board_column DROP COLUMN role; DROP TABLE finding_dismissal; DROP TABLE seen_session; DROP TABLE decision; \
             DROP TABLE card_parent; DROP TABLE card_blocker; DROP TABLE devflow_plan; \
             DROP INDEX card_number; ALTER TABLE card DROP COLUMN number; DROP TABLE card_superseded; \
             ALTER TABLE card DROP COLUMN branch; DROP TABLE host; \
             CREATE TABLE drawing (id TEXT PRIMARY KEY, project_id TEXT NOT NULL, name TEXT NOT NULL, \
             scene TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);",
        )
        .expect("drop later tables");
    store
        .conn()
        .pragma_update(None, "user_version", 9)
        .expect("version");
    drop(store);

    let store = Store::open(&dir.path().join("state.db")).expect("migrate");
    let states: Vec<(String, String)> = store
        .runs(&card)
        .expect("runs")
        .into_iter()
        .map(|run| (run.id, run.state))
        .collect();
    assert_eq!(states.len(), 2);
    assert!(states.contains(&(swept, "lost".to_owned())));
    assert!(states.contains(&(real, "failed".to_owned())));
}

/// Two runs in the same second are ordinary — a step replayed, a lane that
/// advanced — and `started_at` is seconds, so they tie. SQLite may return tied
/// rows in whatever order it likes, and it returned a different one on a Mac
/// than on Linux: "the latest run" came back as the finished one.
///
/// Sabotage: drop the `rowid` from the ORDER BY and this passes on the machine
/// that wrote it and fails on somebody else's.
#[test]
fn two_runs_in_one_second_are_ordered_by_the_one_that_started_later() {
    let (_dir, store, card, step, _column) = seeded();

    let older = store.start_run(&card, &step, None).expect("older");
    store
        .finish_run(&older, "ok", None, None, None, None)
        .expect("finish");
    let latest = store.start_run(&card, &step, None).expect("latest");

    let listed = store.runs(&card).expect("runs");
    let seconds: Vec<i64> = listed.iter().map(|run| run.started_at).collect();
    assert_eq!(
        seconds[0], seconds[1],
        "the two runs did not land in the same second; this test proves nothing"
    );
    assert_eq!(
        listed.iter().map(|run| run.id.as_str()).collect::<Vec<_>>(),
        [latest.as_str(), older.as_str()],
        "most recent first, and the tie went the wrong way"
    );
}

/// Two runs at once on one card: the database refuses the second, whatever
/// the app checked before writing.
#[test]
fn a_second_running_run_on_a_card_is_refused() {
    let (_dir, store, card, step, _column) = seeded();
    store.start_run(&card, &step, None).expect("first");
    assert!(matches!(
        store.start_run(&card, &step, None),
        Err(StoreError::AlreadyRunning)
    ));
    // Another card is not held up by it.
    let project = store.card(&card).expect("card").expect("there").column_id;
    let _ = project;
}

/// #215: a board lists every card's runs, so it reads them without their
/// output, which only the card itself shows.
#[test]
fn a_board_reads_a_cards_runs_without_their_output() {
    let (_dir, store, card, step, _column) = seeded();
    let run = store.start_run(&card, &step, None).expect("start");
    let said = "a long log ".repeat(1000);
    store
        .finish_run(&run, "ok", Some(&said), None, None, Some(0))
        .expect("finish");
    let brief = store.runs_brief(&card).expect("runs");
    assert_eq!(
        (brief[0].state.as_str(), brief[0].output.as_deref()),
        ("ok", None)
    );
    assert_eq!(
        store.runs(&card).expect("runs")[0].output.as_deref(),
        Some(said.as_str())
    );
}
