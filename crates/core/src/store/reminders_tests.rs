use crate::store::Store;

fn store_with_card() -> (tempfile::TempDir, Store, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    let project = store.add_project(&root, None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "Review the PR", "")
        .expect("card");
    (dir, store, card)
}

#[test]
fn a_reminder_goes_off_once_and_waits_to_be_dealt_with() {
    let (_dir, store, card) = store_with_card();
    assert_eq!(store.next_reminder().expect("next"), None);

    store.set_card_due(&card, Some(1_000), true).expect("due");
    assert_eq!(store.next_reminder().expect("next"), Some(1_000));
    assert!(store.reminders_due(999).expect("due").is_empty(), "not yet");

    let due = store.reminders_due(1_000).expect("due");
    assert_eq!(due.len(), 1);
    assert!(due[0].timed);
    store.mark_reminded(&card, 1_001).expect("reminded");
    assert_eq!(store.next_reminder().expect("next"), None, "it went off");
    assert!(store.reminders_due(5_000).expect("due").is_empty());

    let pending = store.reminders_pending().expect("pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].reminded_at, Some(1_001));

    assert!(store.handle_reminder(&card, 1_002).expect("handled"));
    assert!(store.reminders_pending().expect("pending").is_empty());
    assert_eq!(
        store.card(&card).expect("read").expect("there").due_at,
        Some(1_000),
        "dealing with it keeps the card's date"
    );
}

/// Schedules list a reminder still to come or waiting, and one dealt with
/// lately, with its card's number; one dealt with long ago is gone.
#[test]
fn schedules_list_the_open_reminders_and_those_dealt_with_since() {
    let (_dir, store, card) = store_with_card();
    store.set_card_due(&card, Some(1_000), true).expect("due");
    let listed = store.reminders_listed(0).expect("listed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].handled_at, None);
    assert_eq!(listed[0].number, Some(1));

    assert!(store.handle_reminder(&card, 2_000).expect("handled"));
    let listed = store.reminders_listed(1_500).expect("listed");
    assert_eq!(listed[0].handled_at, Some(2_000));
    assert!(store.reminders_listed(2_001).expect("listed").is_empty());
}

/// Moving the date — a snooze — reminds again: the bell's own record would
/// have remembered the card as told, forever.
#[test]
fn a_date_moved_reminds_again() {
    let (_dir, store, card) = store_with_card();
    store.set_card_due(&card, Some(1_000), true).expect("due");
    store.mark_reminded(&card, 1_000).expect("reminded");
    store
        .set_card_due(&card, Some(2_000), true)
        .expect("snoozed");
    assert_eq!(store.next_reminder().expect("next"), Some(2_000));
    assert!(store.reminders_pending().expect("pending").is_empty());
    assert_eq!(store.reminders_open(None).expect("open").len(), 1);

    store.set_card_due(&card, None, true).expect("cleared");
    assert_eq!(store.next_reminder().expect("next"), None);
    assert!(store.reminders_open(None).expect("open").is_empty());
}

/// An archived card reminds of nothing.
#[test]
fn an_archived_card_reminds_of_nothing() {
    let (_dir, store, card) = store_with_card();
    store.set_card_due(&card, Some(1_000), false).expect("due");
    store.archive_card(&card).expect("archived");
    assert_eq!(store.next_reminder().expect("next"), None);
    assert!(store.reminders_due(9_000).expect("due").is_empty());
}

/// The first run of the migration leaves dates already past as told: a
/// banner for each would be weeks of dates nobody asked about again.
#[test]
fn dates_past_before_the_upgrade_do_not_remind() {
    let (dir, store, card) = store_with_card();
    let column = store.card(&card).expect("read").expect("there").column_id;
    let project: String = store
        .conn()
        .query_row(
            "SELECT project_id FROM card WHERE id = ?1",
            [&card],
            |row| row.get(0),
        )
        .expect("project");
    let later = store
        .create_card(&project, &column, "Later", "")
        .expect("card");
    store.set_card_due(&card, Some(1_000), false).expect("past");
    store
        .set_card_due(&later, Some(4_000_000_000), false)
        .expect("future");
    store
        .conn()
        .execute_batch(
            "DROP INDEX card_reminder; ALTER TABLE card DROP COLUMN due_time; \
             ALTER TABLE card DROP COLUMN reminded_at; ALTER TABLE card DROP COLUMN handled_at; \
             ALTER TABLE board_column DROP COLUMN role; DROP TABLE finding_dismissal; DROP TABLE seen_session; DROP TABLE decision; \
             DROP TABLE card_parent; DROP TABLE card_blocker; DROP TABLE devflow_plan; \
             DROP INDEX card_number; ALTER TABLE card DROP COLUMN number; DROP TABLE card_superseded; \
             ALTER TABLE card DROP COLUMN branch; DROP TABLE project_host; DROP TABLE host; PRAGMA user_version = 22;",
        )
        .expect("back to 22");
    drop(store);

    let store = Store::open(&dir.path().join("state.db")).expect("the upgrade");
    assert!(store
        .reminders_due(i64::MAX)
        .expect("due")
        .iter()
        .all(|one| one.card_id == later));
    assert!(store.reminders_pending().expect("pending").is_empty());
    assert_eq!(store.next_reminder().expect("next"), Some(4_000_000_000));
}
