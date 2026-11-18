use std::collections::HashMap;
use std::path::Path;

use devpit_rpc::Project;
use serde_json::json;

use super::{card_of, in_their_accounts, pane_of, pane_target, read, remote_of, shown, start_of};

/// tmux's answer to which window each pane is in.
fn windows(panes: &[(&str, &str)]) -> HashMap<String, String> {
    panes
        .iter()
        .map(|(pane, window)| ((*pane).to_owned(), (*window).to_owned()))
        .collect()
}

fn project(id: &str, root: &Path, orchestrator: Option<&str>) -> Project {
    serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": root.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": orchestrator,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project")
}

fn listing(dir: &Path, pid: i32, name: &str, cwd: &Path) {
    let body = json!({ "pid": pid, "name": name, "status": "busy", "kind": "interactive",
        "cwd": cwd.display().to_string(), "statusUpdatedAt": 1.0 });
    std::fs::write(dir.join(format!("{pid}.json")), body.to_string()).expect("listing");
    // The CLI's key file beside it is not a listing.
    std::fs::write(dir.join(format!("{pid}.abc.key")), "secret").expect("key");
}

#[test]
fn a_live_session_is_placed_on_its_project_and_card() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    let app = dir.path().join("app");
    let worktrees = dir.path().join("worktrees");
    let checkout = worktrees.join("prj_1").join("card_7");
    for path in [&sessions, &app, &checkout] {
        std::fs::create_dir_all(path).expect("mkdir");
    }
    listing(&sessions, 10, "api-worker", &app);
    listing(&sessions, 11, "card-worker", &checkout);

    let found = read(
        &sessions,
        |_| true,
        &[project("prj_1", &app, None)],
        &worktrees,
        &HashMap::new(),
        |_, _| None,
        |_| None,
    );
    let names: Vec<_> = found
        .iter()
        .map(|one| {
            (
                one.name.as_str(),
                one.project_id.as_deref(),
                one.card_id.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("api-worker", Some("prj_1"), None),
            // A card's checkout belongs to the card's project.
            ("card-worker", Some("prj_1"), Some("card_7"))
        ]
    );
}

#[test]
fn a_dead_session_and_the_orchestrators_own_are_left_out() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    let orch = dir.path().join("orchestrator").join("claude");
    for path in [&sessions, &orch] {
        std::fs::create_dir_all(path).expect("mkdir");
    }
    listing(&sessions, 20, "gone", dir.path());
    listing(&sessions, 21, "me", &orch);

    let found = read(
        &sessions,
        |pid| pid != 20,
        &[project("orch", &orch, Some("claude"))],
        dir.path(),
        &HashMap::new(),
        |_, _| None,
        |_| None,
    );
    assert!(
        found.is_empty(),
        "{:?}",
        found.iter().map(|one| &one.name).collect::<Vec<_>>()
    );
}

#[test]
fn a_reviewer_devpit_runs_headless_is_not_a_session() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).expect("mkdir");
    listing(&sessions, 30, "fitmate-9f", dir.path());
    listing(&sessions, 31, "fitmate-style", dir.path());

    let found = read(
        &sessions,
        |pid| shown(pid, &[30], |_| true),
        &[],
        dir.path(),
        &HashMap::new(),
        |_, _| None,
        |_| None,
    );
    let names: Vec<_> = found.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["fitmate-style"]);
}

#[test]
fn only_devpits_card_checkouts_name_a_card() {
    let worktrees = Path::new("/home/me/.devpit/worktrees");
    assert_eq!(
        card_of(worktrees, &worktrees.join("prj_1/card_9/src")).as_deref(),
        Some("card_9")
    );
    assert_eq!(card_of(worktrees, &worktrees.join("prj_1/other")), None);
    assert_eq!(card_of(worktrees, Path::new("/elsewhere/card_9")), None);
}

#[test]
fn only_a_devpit_terminal_can_be_typed_into() {
    let none = HashMap::new();
    // An older CLI listed the client alone.
    assert_eq!(
        pane_target("devpit_prj_1__leaf_9", &none).as_deref(),
        Some("devpit_prj_1:leaf_9")
    );
    // As the CLI lists it now: the client, then tmux's window and pane ids.
    assert_eq!(
        pane_target(
            "devpit_prj_1__leaf_9:@26.%26",
            &windows(&[("%26", "leaf_9")])
        )
        .as_deref(),
        Some("devpit_prj_1:leaf_9")
    );
    assert_eq!(pane_target("devpit_prj_1__leaf_9:@2;rm", &none), None);
    assert_eq!(pane_target("main", &none), None);
    assert_eq!(pane_target("other_prj__leaf_9", &none), None);
    assert_eq!(pane_target("devpit_prj;rm__leaf_9", &none), None);
    assert_eq!(
        pane_target("devpit_prj_1__leaf_9:@2.%2", &windows(&[("%2", "rm;x")])),
        None
    );
}

/// After a restart, the CLI can record another pane's client session: every
/// client of a group shows every window, and tmux named an orphan one. The
/// pane id is the CLI's own and right, and it decides.
#[test]
fn a_session_is_placed_in_its_own_pane_whatever_client_the_cli_recorded() {
    let now = windows(&[("%2", "leaf_mine"), ("%5", "leaf_other")]);
    assert_eq!(
        pane_target("devpit_prj_1__leaf_other:@2.%2", &now).as_deref(),
        Some("devpit_prj_1:leaf_mine")
    );
    // A pane tmux no longer has is no terminal at all, never a neighbour's.
    assert_eq!(pane_target("devpit_prj_1__leaf_other:@9.%9", &now), None);
}

#[test]
fn a_session_in_a_devpit_terminal_says_the_question_it_is_stopped_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).expect("mkdir");
    let body = json!({ "pid": 30, "name": "asking", "status": "idle", "kind": "interactive",
        "cwd": dir.path().display().to_string(), "tmux": "devpit_prj_1__leaf_1" });
    std::fs::write(sessions.join("30.json"), body.to_string()).expect("listing");

    let screen = "Proceed?\n❯ 1. Yes\n  2. No\n";
    let found = read(
        &sessions,
        |_| true,
        &[],
        dir.path(),
        &HashMap::new(),
        |_, _| None,
        |target| (target == "devpit_prj_1:leaf_1").then(|| screen.to_owned()),
    );
    let waiting = found[0].waiting.as_ref().expect("waiting on a question");
    assert_eq!(waiting.question, "Proceed?");
    assert_eq!(waiting.options.len(), 2);
}

#[test]
fn a_devpit_terminal_is_opened_by_its_project_and_pane() {
    let target = pane_target(
        "devpit_prj_01AB__leaf_01CD:@3.%3",
        &windows(&[("%3", "leaf_01CD")]),
    )
    .expect("a target");
    assert_eq!(
        pane_of(&target),
        Some(devpit_rpc::LivePane {
            project_id: "prj_01AB".into(),
            pane_id: "leaf_01CD".into(),
        })
    );
}

/// A listing names a process by pid and start; a pid alone is reused after a
/// restart.
#[test]
fn a_process_start_is_read_past_a_command_with_spaces() {
    let stat = "12490 (claude (x) y) S 11194 12490 11194 34816 12490 4194560 1 2 3 4 5 6 7 8 20 0 12 0 5646 1 2";
    assert_eq!(start_of(stat), Some("5646"));
    assert_eq!(start_of("garbage"), None);
}

/// The kept project list is reused only while it names the projects there
/// are: one added since is a reason to read them again.
#[test]
fn a_project_added_since_the_list_was_kept_is_a_reason_to_read_it_again() {
    let there = vec!["prj_a".to_owned(), "prj_b".to_owned()];
    assert!(super::same_ids(["prj_b", "prj_a"].into_iter(), &there));
    assert!(!super::same_ids(["prj_a"].into_iter(), &there));
    assert!(!super::same_ids(["prj_a", "prj_c"].into_iter(), &there));
}

#[test]
fn remote_control_is_on_while_the_listing_names_a_bridge_and_links_its_page() {
    let off = devpit_rpc::RemoteState::default();
    assert_eq!(remote_of(None), off);
    assert_eq!(remote_of(Some("")), off);
    let on = |url: Option<&str>| devpit_rpc::RemoteState {
        on: true,
        url: url.map(str::to_owned),
    };
    assert_eq!(
        remote_of(Some("session_01AbC-x_9")),
        on(Some("https://claude.ai/code/session_01AbC-x_9"))
    );
    // The infrastructure form names the same page.
    assert_eq!(
        remote_of(Some("cse_01AbC")),
        on(Some("https://claude.ai/code/session_01AbC"))
    );
    // On, with no page on claude.ai to send anybody to.
    assert_eq!(remote_of(Some("session_staging_01A")), on(None));
    assert_eq!(remote_of(Some("session_01A/../x")), on(None));
    assert_eq!(remote_of(Some("bridge-7")), on(None));
}

#[test]
fn a_listing_says_whether_remote_control_is_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).expect("mkdir");
    let write = |pid: i32, name: &str, bridge: serde_json::Value| {
        let body = json!({ "pid": pid, "name": name, "status": "idle", "kind": "interactive",
            "cwd": dir.path().display().to_string(), "bridgeSessionId": bridge });
        std::fs::write(sessions.join(format!("{pid}.json")), body.to_string()).expect("listing");
    };
    write(1, "reached", json!("session_01Q"));
    // Disconnected: the CLI writes null over it.
    write(2, "dropped", serde_json::Value::Null);
    let found = read(
        &sessions,
        |_| true,
        &[],
        dir.path(),
        &HashMap::new(),
        |_, _| None,
        |_| None,
    );
    let remote: Vec<_> = found
        .iter()
        .map(|one| (one.name.as_str(), one.remote.on, one.remote.url.as_deref()))
        .collect();
    assert_eq!(
        remote,
        [
            ("dropped", false, None),
            ("reached", true, Some("https://claude.ai/code/session_01Q"))
        ]
    );
}

/// A ptyd pane has no `$TMUX`, so its listing names no client: the session
/// is found by the id its hooks stored against the leaf, and its question is
/// read off that pane's screen.
#[test]
fn a_session_in_a_ptyd_pane_is_found_by_its_session_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).expect("mkdir");
    let body = json!({ "pid": 31, "name": "in-ptyd", "status": "idle", "kind": "interactive",
        "cwd": dir.path().display().to_string(), "sessionId": "sess-ptyd" });
    std::fs::write(sessions.join("31.json"), body.to_string()).expect("listing");

    let in_ptyd = |session: Option<&str>, _pid: Option<i32>| {
        (session == Some("sess-ptyd")).then(|| devpit_rpc::LivePane {
            project_id: "prj_1".into(),
            pane_id: "leaf_p".into(),
        })
    };
    let screen = "Proceed?\n❯ 1. Yes\n  2. No\n";
    let found = read(
        &sessions,
        |_| true,
        &[],
        dir.path(),
        &HashMap::new(),
        in_ptyd,
        |target| (target == crate::ptyd_link::target("leaf_p")).then(|| screen.to_owned()),
    );
    assert!(found[0].in_devpit);
    assert_eq!(
        found[0].pane.as_ref().map(|pane| pane.pane_id.as_str()),
        Some("leaf_p")
    );
    assert_eq!(
        found[0].waiting.as_ref().expect("waiting").question,
        "Proceed?"
    );

    let unknown = read(
        &sessions,
        |_| true,
        &[],
        dir.path(),
        &HashMap::new(),
        |_, _| None,
        |_| None,
    );
    assert!(
        !unknown[0].in_devpit,
        "no hook named it: in no devpit terminal"
    );
}

/// A session devpit typed into a ptyd pane before it kept the launch: no hook
/// tied it to the pane, and the `DEVPIT_PANE` its process was started with
/// still says which one it is, while a layout holds that leaf.
#[cfg(target_os = "linux")]
#[test]
fn a_session_its_hooks_never_placed_is_found_by_the_pane_its_process_started_in() {
    use super::place_of;
    use devpit_rpc::{LayoutNode, SplitDirection};

    let dir = tempfile::tempdir().expect("tempdir");
    let store = devpit_core::Store::open(&dir.path().join("state.db")).expect("store");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    let project = store.add_project(&root, None).expect("project");
    let tree = LayoutNode::leaf("leaf_first", "s:leaf_first")
        .split_leaf(
            "leaf_first",
            SplitDirection::Horizontal,
            LayoutNode::leaf("leaf_unplaced", "s:leaf_unplaced"),
        )
        .expect("split");
    store
        .set_pane_layout(
            &project,
            "tab_one",
            &serde_json::to_string(&tree).expect("json"),
            "leaf_first",
        )
        .expect("layout");
    let started = |leaf: Option<&str>| {
        let mut sleep = std::process::Command::new("sleep");
        sleep.arg("30").env_remove(devpit_tmux::PANE_ENV);
        if let Some(leaf) = leaf {
            sleep.env(devpit_tmux::PANE_ENV, leaf);
        }
        let child = sleep.spawn().expect("spawn");
        // Spawn returns during the exec, before the kernel says where the
        // new environment lies: until then it reads empty.
        let environ = format!("/proc/{}/environ", child.id());
        for _ in 0..500 {
            if std::fs::read(&environ).is_ok_and(|bytes| !bytes.is_empty()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        child
    };
    let mut in_pane = started(Some("leaf_unplaced"));
    let mut in_no_layout = started(Some("leaf_gone"));
    let mut outside = started(None);
    let pid = |child: &std::process::Child| i32::try_from(child.id()).ok();

    let found = place_of(&store, Some("sess-unplaced"), pid(&in_pane));
    let gone = place_of(&store, Some("sess-gone"), pid(&in_no_layout));
    let nowhere = place_of(&store, Some("sess-outside"), pid(&outside));
    for child in [&mut in_pane, &mut in_no_layout, &mut outside] {
        let _ = child.kill();
        let _ = child.wait();
    }

    assert_eq!(found, Some((project, "leaf_unplaced".to_owned())));
    assert_eq!(gone, None);
    assert_eq!(nowhere, None);
}

/// Two processes on one conversation are in two panes: each is placed in
/// the pane it was started in, so stopping one by its pid closes only its own.
#[cfg(target_os = "linux")]
#[test]
fn a_process_is_placed_in_its_own_pane_when_another_runs_its_conversation() {
    use super::place_of;
    use devpit_rpc::{LayoutNode, SplitDirection};

    let dir = tempfile::tempdir().expect("tempdir");
    let store = devpit_core::Store::open(&dir.path().join("state.db")).expect("store");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    let project = store.add_project(&root, None).expect("project");
    let tree = LayoutNode::leaf("leaf_resumed", "s:leaf_resumed")
        .split_leaf(
            "leaf_resumed",
            SplitDirection::Horizontal,
            LayoutNode::leaf("leaf_second", "s:leaf_second"),
        )
        .expect("split");
    store
        .set_pane_layout(
            &project,
            "tab_one",
            &serde_json::to_string(&tree).expect("json"),
            "leaf_resumed",
        )
        .expect("layout");
    store
        .remember_pane_launch(&project, "leaf_resumed", "claude")
        .expect("launch");
    store
        .remember_pane_session("leaf_resumed", "abc", None)
        .expect("session");
    let mut second = std::process::Command::new("sleep")
        .arg("30")
        .env(devpit_tmux::PANE_ENV, "leaf_second")
        .spawn()
        .expect("spawn");
    let environ = format!("/proc/{}/environ", second.id());
    for _ in 0..500 {
        if std::fs::read(&environ).is_ok_and(|bytes| !bytes.is_empty()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let found = place_of(&store, Some("abc"), i32::try_from(second.id()).ok());
    let _ = second.kill();
    let _ = second.wait();

    assert_eq!(found, Some((project, "leaf_second".to_owned())));
}

/// A project with a Claude Code account of its own starts its sessions there:
/// they are the orchestrator's work all the same, and their state shows.
#[test]
fn a_session_in_a_projects_own_account_is_listed_with_the_profiles() {
    let accounts: devpit_rpc::Accounts = serde_json::from_value(json!({
        "accounts": [{ "name": "work", "configDir": "/acc/work" }],
        "projects": [{ "projectId": "prj_w", "account": "work" }],
    }))
    .expect("accounts");
    let session = |name: &str, project: &str| -> devpit_rpc::LiveSession {
        serde_json::from_value(json!({
            "name": name, "pid": 1, "job": null, "status": "busy", "kind": "interactive",
            "cwd": "/w", "projectId": project, "projectName": null, "cardId": null, "since": null,
            "inDevpit": true, "waiting": null, "pane": null, "sessionId": null, "step": null,
            "draft": null, "remote": { "on": false, "url": null },
        }))
        .expect("a session")
    };
    let listing = |dir: &Path| {
        assert_eq!(dir, Path::new("/acc/work/sessions"));
        vec![
            session("led-api", "prj_w"),
            session("someone-else", "prj_x"),
        ]
    };
    let found = in_their_accounts(&accounts, listing, Path::new("/home/me/.claude"));
    let names: Vec<_> = found
        .iter()
        .map(|one| (one.name.as_str(), one.status.as_str()))
        .collect();
    assert_eq!(names, [("led-api", "busy")]);
    // The profile's own folder is read already, not twice.
    assert!(in_their_accounts(
        &accounts,
        |_| vec![session("led-api", "prj_w")],
        Path::new("/acc/work")
    )
    .is_empty());
}

/// A conversation runs in one process: one a live process lists is running,
/// one only a dead process's leftover listing names is not.
#[test]
fn a_conversation_runs_while_a_live_process_lists_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).expect("mkdir");
    let me = std::process::id() as i32;
    for (pid, session) in [(me, "abc"), (999_999_999, "gone")] {
        let body = json!({ "pid": pid, "name": format!("worker-{session}"), "sessionId": session,
            "status": "busy", "kind": "interactive", "cwd": "/w" });
        std::fs::write(sessions.join(format!("{pid}.json")), body.to_string()).expect("listing");
    }
    assert_eq!(
        super::running_conversation(&sessions, "abc").as_deref(),
        Some("worker-abc")
    );
    assert_eq!(super::running_conversation(&sessions, "gone"), None);
    assert_eq!(super::running_conversation(&sessions, "other"), None);
}
