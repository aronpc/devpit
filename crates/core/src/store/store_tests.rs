//! Opening the store, and what a migration must never do to existing rows.

use super::*;

/// Checked against the database: a table renamed in the SQL without going
/// through this test is a migration that breaks whoever has data.
const EXPECTED: [&str; 10] = [
    "trust_workspace",
    "project",
    "project_plugin",
    "pane_layout",
    "preference",
    "board_column",
    "step",
    "card",
    "run",
    "session_link",
];

fn tables(store: &Store) -> Vec<String> {
    let mut stmt = store
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .expect("query the catalog");
    stmt.query_map([], |row| row.get::<_, String>(0))
        .expect("read names")
        .collect::<Result<Vec<_>, _>>()
        .expect("valid names")
}

#[test]
fn migrations_create_the_tables_and_rerun_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");

    let store = Store::open(&path).expect("first open");
    let first = tables(&store);
    for name in EXPECTED {
        assert!(first.contains(&name.to_string()), "missing table {name}");
    }
    drop(store);

    // The second open is the real test: it is what every start after the
    // first one does.
    let store = Store::open(&path).expect("second open");
    assert_eq!(first, tables(&store), "the second open changed the schema");

    let version: i64 = store
        .conn()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read user_version");
    assert_eq!(version, migrations::latest_version());
}

#[test]
fn syncable_resources_are_born_with_a_revision() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");

    for table in ["project", "project_plugin", "trust_workspace"] {
        let has_revision = store
            .conn()
            .prepare(&format!("SELECT * FROM pragma_table_info('{table}')"))
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .expect("read columns")
            .iter()
            .any(|column| column == "revision");

        assert!(
            has_revision,
            "{table} has no revision — adding it later means a backfill over real data"
        );
    }
}

#[test]
fn foreign_keys_are_enforced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");

    // A project pointing at a workspace that does not exist must be
    // refused now, not found as an orphan three months later.
    let orphan = store.conn().execute(
        "INSERT INTO project (id, trust_workspace_id, name, root_path, created_at) \
         VALUES ('prj_x', 'tw_missing', 'x', '/tmp/x', 0)",
        [],
    );
    assert!(orphan.is_err(), "foreign_keys is off");
}

/// The migration has to be additive over real rows, not over an empty
/// file: that is the case where a mistake costs someone their data.
#[test]
fn migrating_leaves_older_rows_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");

    // A database as it stood before the board existed, with rows in it.
    {
        let conn = Connection::open(&path).expect("open raw");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        migrations::run_up_to(&conn, 3).expect("migrate to 3");
        conn.execute_batch(
            "INSERT INTO trust_workspace                    (id, slug, label, color, vault_namespace, is_default, created_at)                  VALUES ('tw_1', 'p', 'P', '#fff', 'p', 1, 0);                  INSERT INTO project (id, trust_workspace_id, name, root_path, created_at)                  VALUES ('prj_1', 'tw_1', 'demo', '/tmp/demo', 0);                  INSERT INTO pane_layout (project_id, tree, focused_id, updated_at)                  VALUES ('prj_1', '{\"leaf\":1}', 'leaf_1', 0);",
        )
        .expect("seed rows");
    }

    let store = Store::open(&path).expect("migrate to latest");

    let version: i64 = store
        .conn()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    // Against the list rather than against a number typed here: this
    // asserts "every migration ran", which is the thing being tested,
    // and does not have to be edited to add one.
    assert_eq!(version, migrations::latest());

    let name: String = store
        .conn()
        .query_row("SELECT name FROM project WHERE id = 'prj_1'", [], |row| {
            row.get(0)
        })
        .expect("the project survived");
    assert_eq!(name, "demo");

    // The layout table is rebuilt by migration 5, not altered — SQLite
    // cannot change a primary key in place. So its rows are exactly where
    // a mistake would cost someone an open session.
    let (tab_id, focused): (String, String) = store
        .conn()
        .query_row(
            "SELECT tab_id, focused_id FROM pane_layout WHERE project_id = 'prj_1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the layout survived being rebuilt");
    assert_eq!(focused, "leaf_1");
    // A tree that predates tabs is adopted by one named after its project,
    // so a session open across the upgrade is still reachable.
    assert_eq!(tab_id, "tab_prj_1");

    // And the board tables arrived alongside them.
    store.ensure_board("prj_1").expect("seed the board");
    assert_eq!(store.columns("prj_1").expect("columns").len(), 6);
}

/// A row written before a migration must survive it.
///
/// The three this release adds are all `ALTER TABLE run ADD COLUMN`, which is
/// safe — but "is safe" is what everybody says about the migration that ate
/// somebody's history. So this builds the store, writes the history, puts the
/// schema **back** to before those migrations, and opens it again: the same
/// thing a person upgrading does, and the only shape of this test that proves
/// anything. Reopening a store that is already current runs no migrations at
/// all, which is why the obvious version of this test passes whatever the
/// migrations do.
///
/// Sabotage: add `DELETE FROM run;` to any of 017-019 and the run is gone.
#[test]
fn a_row_written_before_a_migration_survives_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");

    let store = Store::open(&path).expect("first open");
    let project = store.add_project(dir.path(), None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "a card", "the body")
        .expect("card");
    let step = store
        .create_step(&project, "command", "tests", "{}", false)
        .expect("step");
    let run = store.start_run(&card, &step, None).expect("run");
    store
        .finish_run(&run, "ok", Some("28 passed"), None, None, Some(0))
        .expect("finish");

    // Back to before this release: the columns go, and so does the version
    // that says they are there.
    let undo = [
        "ran_command",
        "ran_in",
        "declared_env",
        "base_revision",
        "head_revision",
        "in_a_worktree",
        "evidence",
        "evidence_version",
        "saw_changes",
        "asked_by",
        "asked_from",
        "carried_by",
        "carried_profile",
    ];
    for column in undo {
        store
            .conn()
            .execute_batch(&format!("ALTER TABLE run DROP COLUMN {column};"))
            .unwrap_or_else(|err| panic!("dropping {column}: {err}"));
    }
    // 020's columns too, or going back to 16 leaves them for 020 to add twice.
    store
        .conn()
        .execute_batch(
            "ALTER TABLE project DROP COLUMN icon; ALTER TABLE project DROP COLUMN color;",
        )
        .expect("undo 020");
    // And 021's index, which the upgrade makes again.
    store
        .conn()
        .execute_batch("DROP INDEX run_one_running;")
        .expect("undo 021");
    store
        .conn()
        .execute_batch("ALTER TABLE notice DROP COLUMN pane_id;")
        .expect("undo 022");
    store
        .conn()
        .execute_batch(
            "DROP INDEX card_reminder; ALTER TABLE card DROP COLUMN due_time; \
             ALTER TABLE card DROP COLUMN reminded_at; ALTER TABLE card DROP COLUMN handled_at;",
        )
        .expect("undo 023");
    store
        .conn()
        .execute_batch("ALTER TABLE board_column DROP COLUMN role;")
        .expect("undo 024");
    store
        .conn()
        .execute_batch(
            "DROP TABLE finding_dismissal; DROP TABLE seen_session; DROP TABLE decision; \
             DROP TABLE card_parent; DROP TABLE card_blocker; DROP TABLE devflow_plan;",
        )
        .expect("undo 025");
    store
        .conn()
        .execute_batch("DROP INDEX card_number; ALTER TABLE card DROP COLUMN number;")
        .expect("undo 029");
    store
        .conn()
        .execute_batch("DROP TABLE card_superseded;")
        .expect("undo 031");
    store
        .conn()
        .execute_batch("ALTER TABLE card DROP COLUMN branch; DROP TABLE host;")
        .expect("undo 033 and 034");
    store
        .conn()
        .execute_batch("PRAGMA user_version = 16;")
        .expect("back to 16");
    drop(store);

    // The upgrade, for real: 017, 018 and 019 apply to a table with a row in it.
    let store = Store::open(&path).expect("the upgrade");
    let kept = store.runs(&card).expect("runs");
    assert_eq!(kept.len(), 1, "the run did not survive the upgrade");
    assert_eq!(kept[0].output.as_deref(), Some("28 passed"));
    assert_eq!(kept[0].exit_code, Some(0));
    assert_eq!(
        store.card(&card).expect("read").expect("a card").body,
        "the body"
    );

    // And the new columns are NULL on it, which is exactly what a row written
    // before them should read as — never a default that looks like an answer.
    assert!(
        !store
            .what_ran(&run)
            .expect("read")
            .expect("a run")
            .is_known(),
        "a row from before 017 claimed to know what it ran"
    );
    assert!(
        !store
            .whose_run(&run)
            .expect("read")
            .expect("a run")
            .is_known(),
        "a row from before 019 claimed to know whose it was"
    );
    assert_eq!(store.evidence_of(&run).expect("read"), None);
}

/// A build being worked on never opens the installed devpit's home.
///
/// Held by the build profile so it cannot be forgotten: two devpits on one
/// home write their listener ports over each other's in `hook-endpoint`, and
/// one of them stops hearing its agents without a word — the board just goes
/// quiet. Tests are debug builds, so this runs as the development side of the
/// rule, which is the side that can do the damage.
#[test]
#[cfg(debug_assertions)]
fn a_debug_build_keeps_a_home_apart_from_the_installed_one() {
    assert_eq!(ROOT_NAME, DEV_ROOT);
    assert_ne!(ROOT_NAME, RELEASE_ROOT);
    /* Beside it, not inside it: a home nested in the installed one would be
    taken with it by anything that clears that folder. */
    assert!(!DEV_ROOT.starts_with(&format!("{RELEASE_ROOT}/")));
}

#[test]
fn an_up_to_date_store_opens_while_another_connection_writes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");
    drop(Store::open(&path).expect("first open migrates"));

    // Another writer holds the lock, as a terminal's hook does mid-write.
    let writer = rusqlite::Connection::open(&path).expect("writer");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("take the write lock");

    let started = std::time::Instant::now();
    Store::open(&path).expect("opens without waiting for the writer");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "open queued behind the writer for {:?}",
        started.elapsed()
    );
    writer.execute_batch("ROLLBACK").expect("release");
}
