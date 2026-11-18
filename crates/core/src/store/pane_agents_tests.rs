//! What a pane remembers about the agent in it.

use crate::store::pane_agents::PaneAgent;
use crate::store::Store;

fn opened() -> (tempfile::TempDir, Store, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("create");
    let project = store.add_project(&root, None).expect("project");
    (dir, store, project)
}

#[test]
fn a_launched_agent_learns_its_session_from_the_hooks() {
    let (_dir, store, project) = opened();
    store
        .remember_pane_launch(&project, "leaf_1", "prof_glm")
        .expect("launch");
    store
        .remember_pane_session(
            "leaf_1",
            "abc",
            Some("/home/me/.claude/projects/-w/abc.jsonl"),
        )
        .expect("session");
    assert_eq!(
        store.pane_agent("leaf_1").expect("read"),
        Some(PaneAgent {
            launch: "prof_glm".to_owned(),
            session_id: Some("abc".to_owned()),
            transcript_path: Some("/home/me/.claude/projects/-w/abc.jsonl".to_owned()),
        })
    );
}

/// Only an agent devpit started can be started again; a hook alone is not a launch.
#[test]
fn a_hook_from_a_pane_nobody_launched_in_records_nothing() {
    let (_dir, store, _project) = opened();
    store
        .remember_pane_session("leaf_9", "abc", None)
        .expect("session");
    assert_eq!(store.pane_agent("leaf_9").expect("read"), None);
}

#[test]
fn a_new_launch_starts_clean_and_forgetting_removes_it() {
    let (_dir, store, project) = opened();
    store
        .remember_pane_launch(&project, "leaf_1", "claude")
        .expect("launch");
    store
        .remember_pane_session("leaf_1", "abc", None)
        .expect("session");
    store
        .remember_pane_launch(&project, "leaf_1", "codex")
        .expect("relaunch");
    let again = store.pane_agent("leaf_1").expect("read").expect("row");
    assert_eq!((again.launch.as_str(), again.session_id), ("codex", None));

    store.forget_pane_agent("leaf_1").expect("forget");
    assert_eq!(store.pane_agent("leaf_1").expect("read"), None);
}

#[test]
fn a_session_is_found_in_the_project_and_pane_its_hooks_named() {
    let (_dir, store, project) = opened();
    store
        .remember_pane_launch(&project, "leaf_1", "prof_glm")
        .expect("launch");
    assert_eq!(store.place_of_session("abc").expect("read"), None);
    store
        .remember_pane_session("leaf_1", "abc", None)
        .expect("session");
    assert_eq!(
        store.place_of_session("abc").expect("read"),
        Some((project, "leaf_1".to_owned()))
    );
}

/// A conversation runs in one pane. Resumed in another, the pane it left
/// forgets it, so that pane coming back never starts it a second time.
#[test]
fn a_conversation_resumed_in_another_pane_is_forgotten_by_the_one_it_left() {
    let (_dir, store, project) = opened();
    for leaf in ["leaf_left", "leaf_resumed"] {
        store
            .remember_pane_launch(&project, leaf, "claude")
            .expect("launch");
    }
    store
        .remember_pane_session("leaf_left", "abc", None)
        .expect("first");
    store
        .remember_pane_session("leaf_resumed", "abc", None)
        .expect("resumed");
    assert_eq!(store.pane_agent("leaf_left").expect("read"), None);
    assert_eq!(
        store.place_of_session("abc").expect("read"),
        Some((project, "leaf_resumed".to_owned()))
    );
}

/// Resumed by hand in a pane devpit launched nothing in, the conversation
/// stays with the pane that can start it again.
#[test]
fn a_conversation_resumed_by_hand_stays_with_the_pane_that_launched_it() {
    let (_dir, store, project) = opened();
    store
        .remember_pane_launch(&project, "leaf_launched", "claude")
        .expect("launch");
    store
        .remember_pane_session("leaf_launched", "abc", None)
        .expect("first");
    store
        .remember_pane_session("leaf_by_hand", "abc", None)
        .expect("by hand");
    assert_eq!(
        store.place_of_session("abc").expect("read"),
        Some((project, "leaf_launched".to_owned()))
    );
}
