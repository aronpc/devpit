use rusqlite::Connection;

use super::*;
use crate::store::migrations;

fn staging() -> HostRow {
    HostRow {
        id: "host_1".to_owned(),
        name: "staging box".to_owned(),
        target: "staging".to_owned(),
        user: Some("deploy".to_owned()),
        port: Some(2222),
        identity_file: Some("~/.ssh/id_ed25519".to_owned()),
        work_root: "/srv/apps".to_owned(),
        environment: "staging".to_owned(),
    }
}

/// A profile is read back as it was saved, after the store is opened again.
#[test]
fn a_saved_host_is_listed_again_after_a_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");
    Store::open(&path)
        .expect("open")
        .save_host(&staging())
        .expect("saved");

    let again = Store::open(&path).expect("open again");

    assert_eq!(again.hosts().expect("hosts"), vec![staging()]);
}

/// Saving under the same id edits the profile, and removing it forgets it.
#[test]
fn a_host_is_edited_in_place_and_removed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    store.save_host(&staging()).expect("saved");

    let production = HostRow {
        environment: "production".to_owned(),
        ..staging()
    };
    store.save_host(&production).expect("edited");
    assert_eq!(store.hosts().expect("hosts"), vec![production]);

    store.remove_host("host_1").expect("removed");
    assert!(store.hosts().expect("hosts").is_empty());
}

/// Only the three environments are kept: the label is what marks production.
#[test]
fn an_environment_outside_the_three_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");

    let odd = HostRow {
        environment: "prod".to_owned(),
        ..staging()
    };

    assert!(store.save_host(&odd).is_err());
}

/// A database from before hosts migrates with its projects and layouts as
/// they were, and with no host.
#[test]
fn a_database_from_before_hosts_reads_as_before() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");
    {
        let conn = Connection::open(&path).expect("open raw");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        migrations::run_up_to(&conn, 33).expect("migrate to 33");
        conn.execute_batch(
            "INSERT INTO trust_workspace (id, slug, label, color, vault_namespace, is_default, created_at) \
             VALUES ('tw_1', 'p', 'P', '#fff', 'p', 1, 0); \
             INSERT INTO project (id, trust_workspace_id, name, root_path, created_at) \
             VALUES ('prj_1', 'tw_1', 'demo', '/tmp/demo', 0); \
             INSERT INTO pane_layout (project_id, tab_id, tree, focused_id, updated_at) \
             VALUES ('prj_1', 'tab_1', '{\"leaf\":1}', 'leaf_1', 0);",
        )
        .expect("seed rows");
    }

    let store = Store::open(&path).expect("migrate");

    assert!(store.hosts().expect("hosts").is_empty());
    let (name, tree): (String, String) = store
        .conn()
        .query_row(
            "SELECT p.name, l.tree FROM project p JOIN pane_layout l ON l.project_id = p.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the project and its layout");
    assert_eq!((name.as_str(), tree.as_str()), ("demo", "{\"leaf\":1}"));
}
