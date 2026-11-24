use super::*;

/// Every preference key the store names, read from where they are named.
fn every_key() -> Vec<String> {
    include_str!("../../core/src/store/settings_keys.rs")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub const "))
        .filter_map(|rest| rest.split_once("= \""))
        .filter_map(|(_, value)| value.split_once('"'))
        .map(|(value, _)| value.to_owned())
        .collect()
}

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    (dir, store)
}

#[test]
fn every_preference_is_on_one_list_and_one_only() {
    let keys = every_key();
    assert!(keys.len() > 40, "the keys were read: {}", keys.len());
    for name in &keys {
        let local = LOCAL.iter().filter(|(one, _)| one == name).count();
        let synced = SYNCED.iter().filter(|one| *one == name).count();
        assert_eq!(
            local + synced,
            1,
            "{name} must be on SYNCED or LOCAL in prefs.rs, once — a new preference \
             stays on its machine until someone decides it travels"
        );
    }
    assert_eq!(
        keys.len(),
        SYNCED.len() + LOCAL.len(),
        "no list names a key that is gone"
    );
}

#[test]
fn what_belongs_to_one_machine_never_syncs() {
    for name in [
        key::REMOTE,
        key::REMOTE_VIA,
        key::REMOTE_MCP,
        key::CALENDAR_LINK,
        key::AGENT_PROFILES,
        key::WORKTREE_BASE,
        key::SIDEBAR_WIDTH,
        key::ISLAND_SCREEN,
        key::ISLAND_ANYWAY,
        key::KEEP_IN_TRAY,
        key::TRANSCRIBE_MODEL,
        key::SYNC,
    ] {
        assert!(!syncs(name), "{name} synced");
    }
    for name in [
        key::THEME,
        key::LANGUAGE,
        key::SHORTCUT_KEYS,
        key::TERMINAL_CONTRAST,
    ] {
        assert!(syncs(name), "{name} did not sync");
    }
}

#[test]
fn only_what_syncs_is_read_from_this_machine() {
    let (_dir, store) = store();
    store.set_preference(key::THEME, "light").expect("set");
    store.set_preference(key::REMOTE, "true").expect("set");
    store
        .set_preference(key::CALENDAR_LINK, "https://devpit.app/cal/SECRET.ics")
        .expect("set");
    let here = Prefs { store: &store }.here().expect("here");
    let fields = &here[RECORD];
    assert_eq!(fields.keys().collect::<Vec<_>>(), vec![key::THEME]);
    assert!(fields[key::THEME]
        .at_ms
        .is_some_and(|ms| ms > 1_700_000_000_000));
}

#[test]
fn only_what_syncs_is_written_to_this_machine() {
    let (_dir, store) = store();
    let prefs = Prefs { store: &store };
    prefs
        .write(RECORD, key::REMOTE, Some("true"))
        .expect("write");
    prefs
        .write(
            RECORD,
            key::CALENDAR_LINK,
            Some("https://elsewhere.example/x.ics"),
        )
        .expect("write");
    prefs
        .write(RECORD, key::THEME, Some("light"))
        .expect("write");
    prefs
        .write(
            RECORD,
            key::TERMINAL_CONTRAST,
            Some(&"9".repeat(LONGEST + 1)),
        )
        .expect("write");
    prefs
        .write("elsewhere", key::FOCUS_MODE, Some("true"))
        .expect("write");
    assert_eq!(store.preference(key::REMOTE).expect("read"), None);
    assert_eq!(store.preference(key::CALENDAR_LINK).expect("read"), None);
    assert_eq!(
        store.preference(key::TERMINAL_CONTRAST).expect("read"),
        None
    );
    assert_eq!(store.preference(key::FOCUS_MODE).expect("read"), None);
    assert_eq!(
        store.preference(key::THEME).expect("read").as_deref(),
        Some("light")
    );
}
