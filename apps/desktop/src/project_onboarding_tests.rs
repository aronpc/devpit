use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::project_agent::{Agents, Wake};
use crate::project_memory::{self, ONBOARDING};

/// A CLI that answers a request to learn the project with what it read, a
/// line of it resting on no real file and one said twice.
const READER: &str = r#"#!/bin/sh
while IFS= read -r line; do
  text="answer"
  case "$line" in
    *"Learn this project"*) text="Here is what I read.\n\n- fact · AGENTS.md · Everything versioned is in English.\n- decision · docs/adr/0001-sync.md, AGENTS.md · The person's workspace syncs, not their machine.\n- fact · nowhere.md · Something invented.\n- fact · \`Makefile\`:12 · make test runs the guards and every test.\n- fact · AGENTS.md · Everything versioned is in English.\n- lesson · AGENTS.md · Not a kind it is asked for." ;;
  esac
  printf '{"type":"system","subtype":"init","session_id":"s-1"}\n'
  printf '{"type":"assistant","message":{"id":"m-1","role":"assistant","content":[{"type":"text","text":"%s"}]},"session_id":"s-1"}\n' "$text"
  printf '{"type":"result","subtype":"success","session_id":"s-1","total_cost_usd":0}\n'
done
"#;

struct Bench {
    _dir: tempfile::TempDir,
    root: PathBuf,
    cli: String,
}

fn bench() -> Bench {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join("docs/adr")).expect("root");
    std::fs::write(root.join("AGENTS.md"), "# Working in this repository\n").expect("agents");
    std::fs::write(root.join("Makefile"), "test:\n\tcargo test\n").expect("makefile");
    std::fs::write(root.join("docs/adr/0001-sync.md"), "# 1. Sync\n").expect("adr");
    let cli = dir.path().join("claude");
    std::fs::write(&cli, READER).expect("stand-in");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    Bench {
        cli: cli.display().to_string(),
        root,
        _dir: dir,
    }
}

fn wake(bench: &Bench) -> Wake<'_> {
    Wake {
        command: &bench.cli,
        env: &[],
        root: &bench.root,
        name: "pilot",
        mcp_config: None,
    }
}

/// Learned with the retry a just-written script needs.
fn learned(agents: &Agents, bench: &Bench) -> usize {
    for _ in 0..5 {
        match learn(agents, "p1", &wake(bench), "2026-11-14") {
            Err(why) if why.contains(crate::project_agent::NOT_STARTED) => {
                std::thread::sleep(Duration::from_millis(50))
            }
            other => return other.expect("learned"),
        }
    }
    panic!("the stand-in never started");
}

#[test]
fn learning_proposes_the_facts_and_decisions_it_read_each_resting_on_real_files() {
    let bench = bench();
    let agents = Agents::idling_after(Duration::from_secs(60));
    assert_eq!(learned(&agents, &bench), 3);
    let waiting = project_memory::proposals(&bench.root);
    let said: Vec<(&str, &str)> = waiting
        .iter()
        .map(|one| (one.source.as_str(), one.text.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            (
                "onboarding, AGENTS.md",
                "facts.md: Everything versioned is in English."
            ),
            (
                "onboarding, docs/adr/0001-sync.md, AGENTS.md",
                "decisions.md: The person's workspace syncs, not their machine."
            ),
            (
                "onboarding, Makefile",
                "facts.md: make test runs the guards and every test."
            ),
        ]
    );
    assert!(waiting.iter().all(|one| one.source.starts_with(ONBOARDING)));
    // Proposed, never written: the memory holds only what its first wake wrote.
    assert!(!project_memory::kept(&bench.root, "facts.md")
        .iter()
        .any(|one| one.text.contains("English")));
    assert!(learned_before(&bench.root));
}

#[test]
fn learning_again_leaves_out_what_the_memory_already_says_and_replaces_its_old_proposals() {
    let bench = bench();
    let agents = Agents::idling_after(Duration::from_secs(60));
    learned(&agents, &bench);
    project_memory::decide(
        &bench.root,
        "2026-11-14",
        "onboarding, AGENTS.md",
        "Everything versioned is in English.",
        None,
        Some("Everything versioned is in English."),
    )
    .expect("approved");
    assert_eq!(learned(&agents, &bench), 2);
    assert_eq!(project_memory::proposals(&bench.root).len(), 2);
}

fn project(id: &str, root: &Path, orchestrator: Option<&str>) -> devpit_rpc::Project {
    serde_json::from_value(json!({
        "id": id,
        "name": id,
        "rootPath": root.display().to_string(),
        "group": null,
        "accent": "#000000",
        "worktrees": [],
        "unreadable": null,
        "orchestrator": orchestrator,
        "origin": null,
        "lastOpenedAt": null,
        "icon": null,
        "color": null,
    }))
    .expect("a project")
}

#[test]
fn a_linked_project_learns_on_its_own_once_and_one_left_out_in_labs_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (api, web, loose) = (
        dir.path().join("api"),
        dir.path().join("web"),
        dir.path().join("loose"),
    );
    for root in [&api, &web, &loose] {
        std::fs::create_dir_all(root).expect("root");
    }
    let projects = [
        project("api", &api, None),
        project("web", &web, None),
        project("loose", &loose, None),
        project("orch", dir.path(), Some("claude")),
    ];
    let linked = ["api".to_owned(), "web".to_owned(), "orch".to_owned()];
    let left_out = ["web".to_owned()];

    assert_eq!(to_learn(&projects, &linked, true, &left_out), ["api"]);
    assert!(
        to_learn(&projects, &linked, false, &[]).is_empty(),
        "Labs off"
    );

    project_memory::seed(&api, "2026-11-14").expect("seed");
    project_memory::onboarded(&api, "2026-11-14").expect("learned");
    assert!(
        to_learn(&projects, &linked, true, &left_out).is_empty(),
        "learned once"
    );
}

#[test]
fn a_project_left_out_in_labs_is_refused_like_labs_off() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = devpit_core::Store::open(&dir.path().join("state.db")).expect("open");
    assert!(on_for(&store, "api"));
    leave_out(&store, "api", true).expect("left out");
    assert!(!on_for(&store, "api"));
    assert!(on_for(&store, "web"));
    leave_out(&store, "api", false).expect("back in");
    assert!(on_for(&store, "api"));
    store
        .set_preference_flag(devpit_core::preference::LABS_PROJECT_AGENT, false)
        .expect("flag");
    assert!(!on_for(&store, "web"), "Labs off leaves every project out");
}

#[test]
fn an_agent_answering_is_busy_and_learning_waits_for_it() {
    let bench = bench();
    let agents = Agents::idling_after(Duration::from_secs(60));
    assert!(!agents.busy("p1"));
    learned(&agents, &bench);
    assert!(!agents.busy("p1"), "free once it answered");
}

#[test]
fn a_project_is_queued_once_and_not_while_it_is_being_learned() {
    let mut queued = std::collections::VecDeque::from(["api".to_owned()]);
    added(
        &mut queued,
        Some("web"),
        vec![
            "api".to_owned(),
            "web".to_owned(),
            "cli".to_owned(),
            "cli".to_owned(),
        ],
    );
    assert_eq!(queued, ["api", "cli"]);
}
