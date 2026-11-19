//! What the project rows have to keep straight.
//!
//! Beside `projects.rs` rather than inside it, for the size ratchet: the file
//! is the rules, and this is the evidence that they hold.

use super::*;
use crate::origin::origin_hash;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    (dir, store)
}

#[test]
fn forgetting_keeps_the_row_and_erasing_takes_the_board_with_it() {
    // The two halves of the same dialog. Forget has to be reversible in
    // principle — the cards are still there — and erase has to actually
    // take them, or the box that says "this cannot be undone" is wrong in
    // the one direction that matters.
    let (dir, store) = store();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    let id = store.add_project(&root, None).expect("add");
    store.ensure_board(&id).expect("board");
    let column = store.columns(&id).expect("columns")[0].id.clone();
    store.create_card(&id, &column, "a card", "").expect("card");

    assert!(store.forget_project(&id).expect("forget"));
    assert_eq!(store.cards(&id).expect("cards").len(), 1);
    assert!(store.project(&id).expect("read").is_some());

    assert!(store.erase_project(&id).expect("erase"));
    assert!(store.project(&id).expect("read").is_none());
    assert!(store.cards(&id).expect("cards").is_empty());
}

#[test]
fn renaming_changes_the_name_and_not_the_folder() {
    let (dir, store) = store();
    let root = dir.path().join("api-v2-final");
    std::fs::create_dir_all(&root).expect("create");
    let id = store.add_project(&root, None).expect("add");

    assert!(store.rename_project(&id, "API").expect("rename"));
    let row = store.project(&id).expect("read").expect("still there");
    assert_eq!(row.name, "API");
    assert_eq!(row.root_path, root.to_string_lossy());
}

/// A group is renamed everywhere at once, and emptied when the name is.
#[test]
fn a_group_is_renamed_across_its_projects() {
    let (dir, store) = store();
    let ids: Vec<String> = ["a", "b", "c"]
        .iter()
        .map(|name| {
            let root = dir.path().join(name);
            std::fs::create_dir_all(&root).expect("create");
            store.add_project(&root, None).expect("add")
        })
        .collect();
    for id in &ids[..2] {
        store
            .edit_project(id, "x", Some("Work"), None, None)
            .expect("edit");
    }

    assert_eq!(
        store.rename_group("Work", Some("Clients")).expect("rename"),
        2
    );
    let group = |id: &str| store.project(id).expect("read").expect("there").group;
    assert_eq!(group(&ids[0]).as_deref(), Some("Clients"));
    assert_eq!(group(&ids[2]), None);

    assert_eq!(store.rename_group("Clients", None).expect("ungroup"), 2);
    assert_eq!(group(&ids[1]), None);
}

/// The edit dialog sets name, group and mark together, and clears what it
/// leaves empty.
#[test]
fn an_edit_sets_the_group_and_the_mark_and_can_clear_them() {
    let (dir, store) = store();
    let root = dir.path().join("site");
    std::fs::create_dir_all(&root).expect("create");
    let id = store.add_project(&root, None).expect("add");

    assert!(store
        .edit_project(
            &id,
            "Site",
            Some("Clients"),
            Some("icon:rocket"),
            Some("#e2795b")
        )
        .expect("edit"));
    let row = store.project(&id).expect("read").expect("there");
    assert_eq!(row.name, "Site");
    assert_eq!(row.group.as_deref(), Some("Clients"));
    assert_eq!(row.icon.as_deref(), Some("icon:rocket"));
    assert_eq!(row.color.as_deref(), Some("#e2795b"));

    assert!(store
        .edit_project(&id, "Site", None, None, None)
        .expect("clear"));
    let row = store.project(&id).expect("read").expect("there");
    assert_eq!((row.group, row.icon, row.color), (None, None, None));
}

#[test]
fn a_project_remembers_its_remote_and_when_it_was_opened() {
    // Both were written on the way in and read by nothing, so the list
    // was ordered by a column it could not show.
    let (dir, store) = store();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    let id = store
        .add_project(&root, Some("git@github.com:owner/name.git"))
        .expect("add");

    let row = store.project(&id).expect("read").expect("there");
    assert_eq!(row.origin.as_deref(), Some("git@github.com:owner/name.git"));
    assert!(row.last_opened_at.is_some(), "adding did not set the clock");
}

#[test]
fn adding_the_same_folder_twice_opens_it_rather_than_failing() {
    let (dir, store) = store();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");

    let first = store.add_project(&root, None).expect("add");
    let second = store.add_project(&root, None).expect("add again");
    assert_eq!(first, second, "the second add created a duplicate");
    assert_eq!(store.projects().expect("list").len(), 1);
}

#[test]
fn the_two_shapes_of_one_origin_hash_the_same() {
    // The reason this exists: the same repository cloned over ssh on one
    // machine and https on another has to converge on one project.
    assert_eq!(
        origin_hash("git@github.com:jholhewres/devpit.git"),
        origin_hash("https://github.com/jholhewres/devpit")
    );
}

#[test]
fn a_project_whose_origin_changed_takes_the_new_identity() {
    let (dir, store) = store();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    let id = store
        .add_project(&root, Some("git@github.com:old/repo.git"))
        .expect("add");

    let moved = store
        .refresh_origin(&id, "git@github.com:new/repo.git")
        .expect("refresh");

    assert!(moved, "a changed origin was not taken");
    let synced = store.synced_projects().expect("synced");
    assert_eq!(
        synced[0].origin_hash.as_deref(),
        Some(origin_hash("https://github.com/new/repo").as_str())
    );
    assert_eq!(
        synced[0].origin_url.as_deref(),
        Some("git@github.com:new/repo.git")
    );
    assert!(!store
        .refresh_origin(&id, "https://github.com/new/repo")
        .expect("same repository"));
}

#[test]
fn an_origin_with_a_token_is_kept_without_it() {
    let (dir, store) = store();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    store
        .add_project(&root, Some("https://jhol:<REDACTED>@github.com/o/r.git"))
        .expect("add");

    let synced = store.synced_projects().expect("synced");
    assert_eq!(
        synced[0].origin_url.as_deref(),
        Some("https://github.com/o/r.git")
    );
}

#[test]
fn an_older_hash_is_recomputed_when_the_store_opens() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.db");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    {
        let store = Store::open(&path).expect("open");
        store
            .add_project(&root, Some("https://github.com:443/o/r.git"))
            .expect("add");
        // As a devpit before this normalisation left it.
        store
            .conn
            .execute_batch(
                "UPDATE project SET origin_hash = 'old', \
                 origin_url = 'https://jhol:tok@github.com:443/o/r.git'; \
                 ALTER TABLE card DROP COLUMN branch; DROP TABLE project_host; DROP TABLE host; PRAGMA user_version = 31;",
            )
            .expect("age it");
    }

    let store = Store::open(&path).expect("reopen");

    let synced = store.synced_projects().expect("synced");
    assert_eq!(
        synced[0].origin_hash.as_deref(),
        Some(origin_hash("git@github.com:o/r.git").as_str())
    );
    assert_eq!(
        synced[0].origin_url.as_deref(),
        Some("https://github.com:443/o/r.git"),
        "an older build's token is not kept"
    );
}

#[test]
fn a_project_carries_its_workspace_colour() {
    let (dir, store) = store();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    store.add_project(&root, None).expect("add");

    let listed = store.projects().expect("list");
    assert_eq!(listed[0].accent, DEFAULT_ACCENT);
    assert_eq!(listed[0].name, "project");
}
