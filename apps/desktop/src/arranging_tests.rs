use super::*;
use crate::ptyd_link::target;
#[cfg(target_os = "linux")]
use crate::ptyd_link::{pane_number, running};
use devpit_core::Store;

fn tab_of(project_id: &str, leaves: &[(&str, String)]) -> SessionLayout {
    let (first, first_target) = &leaves[0];
    let mut tree = LayoutNode::leaf(*first, first_target.clone());
    for (leaf, at) in &leaves[1..] {
        tree = tree
            .split_leaf(
                first,
                SplitDirection::Horizontal,
                LayoutNode::leaf(*leaf, at.clone()),
            )
            .expect("split");
    }
    SessionLayout {
        project_id: project_id.into(),
        focused_id: (*first).into(),
        tree,
    }
}

/// A shell whose child ignores `SIGHUP`, as a TUI agent does: the daemon's
/// `Kill` alone ends the shell and leaves the child running.
#[cfg(target_os = "linux")]
fn an_agent_in(home: &std::path::Path, leaf: &str) {
    let asked = devpit_ptyd::pane::Asked {
        rows: 24,
        cols: 80,
        cwd: home.to_string_lossy().into_owned(),
        argv: vec![
            "/bin/sh".into(),
            "-c".into(),
            "sh -c \"trap '' HUP; exec sleep 300\" & wait".into(),
        ],
        env: vec![(devpit_tmux::PANE_ENV.into(), leaf.into())],
    };
    crate::ptyd_link::spawn(home, pane_number(leaf), &asked).expect("spawn");
}

#[cfg(target_os = "linux")]
fn gone(leaf: &str) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if devpit_pty::started_with(devpit_tmux::PANE_ENV, leaf).is_some_and(|left| left.is_empty())
        {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

#[cfg(target_os = "linux")]
#[test]
fn a_tab_of_ptyd_panes_closes_with_no_tmux_and_ends_what_ignores_a_hangup() {
    let daemon = crate::ptyd_link::tests::serving();
    let home = daemon.home.path();
    let store = Store::open(&home.join("state.db")).expect("store");
    let project = store.add_project(home, None).expect("project");
    let leaf = format!("leaf_hup_{}", std::process::id());
    an_agent_in(home, &leaf);
    let layout = tab_of(&project, &[(leaf.as_str(), target(&leaf))]);
    persist(&store, "tab_1", &layout).expect("persist");

    let closed = end_tab_at(&store, Some(home), &layout, "tab_1", |_| {
        panic!("tmux was asked for a tab of ptyd panes")
    })
    .expect("closed");

    assert_eq!(closed, std::slice::from_ref(&leaf));
    assert_eq!(store.pane_layout(&project, "tab_1").expect("read"), None);
    assert!(gone(&leaf), "the agent outlived its pane");
    assert!(
        closed_in_daemon(home, &leaf),
        "the daemon still holds the pane"
    );
}

/// The daemon reaps a killed pane on its own time: wait for it, as `gone` does.
#[cfg(target_os = "linux")]
fn closed_in_daemon(home: &std::path::Path, leaf: &str) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if !running(home, pane_number(leaf)) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

#[cfg(unix)]
#[test]
fn only_a_tabs_tmux_leaves_are_handed_to_tmux() {
    let daemon = crate::ptyd_link::tests::serving();
    let home = daemon.home.path();
    let store = Store::open(&home.join("state.db")).expect("store");
    let project = store.add_project(home, None).expect("project");
    let layout = tab_of(
        &project,
        &[
            ("leaf_t", "devpit_p:leaf_t".to_owned()),
            ("leaf_q", target("leaf_q")),
        ],
    );
    persist(&store, "tab_1", &layout).expect("persist");
    let mut asked = Vec::new();
    end_tab_at(&store, Some(home), &layout, "tab_1", |leaves| {
        asked = leaves.iter().map(|leaf| leaf.to_string()).collect();
    })
    .expect("closed");
    assert_eq!(asked, ["leaf_t"]);
    assert_eq!(store.pane_layout(&project, "tab_1").expect("read"), None);
}

/// A host's leaf is ended on its host, never handed to this machine's tmux:
/// closing its tab asks tmux here of the local leaves alone.
#[cfg(unix)]
#[test]
fn a_host_s_leaf_is_never_handed_to_tmux_here() {
    let home = tempfile::tempdir().expect("home");
    let store = Store::open(&home.path().join("state.db")).expect("store");
    let project = store.add_project(home.path(), None).expect("project");
    let layout = tab_of(
        &project,
        &[
            ("leaf_t", "devpit_p:leaf_t".to_owned()),
            (
                "leaf_h",
                crate::host_panes::target("host_gone", "devpit_p__leaf_h"),
            ),
        ],
    );
    persist(&store, "tab_1", &layout).expect("persist");
    let mut asked = Vec::new();

    let closed = end_tab_at(&store, Some(home.path()), &layout, "tab_1", |leaves| {
        asked = leaves.iter().map(|leaf| leaf.to_string()).collect();
    })
    .expect("closed");

    assert_eq!(asked, ["leaf_t"]);
    assert_eq!(closed.len(), 2, "both leaves are closed");
}
