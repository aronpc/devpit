use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::project_agent::{Agents, Wake};
use crate::project_memory;

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
fn learning_puts_its_file_facts_in_the_memory_and_leaves_its_decisions_waiting() {
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
        [(
            "onboarding, docs/adr/0001-sync.md, AGENTS.md",
            "decisions.md: The person's workspace syncs, not their machine."
        )]
    );
    let facts: Vec<(String, String)> = project_memory::kept(&bench.root, "facts.md")
        .into_iter()
        .filter(|one| one.auto())
        .map(|one| (one.source, one.text))
        .collect();
    assert_eq!(
        facts,
        [
            (
                "AGENTS.md, accepted automatically".to_owned(),
                "Everything versioned is in English.".to_owned()
            ),
            (
                "Makefile, accepted automatically".to_owned(),
                "make test runs the guards and every test.".to_owned()
            ),
        ]
    );
    assert!(learned_before(&bench.root));
}

#[test]
fn learning_again_leaves_out_what_the_memory_already_says_and_replaces_its_old_proposals() {
    let bench = bench();
    let agents = Agents::idling_after(Duration::from_secs(60));
    learned(&agents, &bench);
    assert_eq!(learned(&agents, &bench), 1, "its facts are kept already");
    assert_eq!(project_memory::proposals(&bench.root).len(), 1);
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

fn store(dir: &Path) -> devpit_core::Store {
    devpit_core::Store::open(&dir.join("state.db")).expect("open")
}

/// A project's folder, learned before when `learned`.
fn folder(dir: &Path, name: &str, learned: bool) -> PathBuf {
    let root = dir.join(name);
    std::fs::create_dir_all(&root).expect("root");
    if learned {
        project_memory::seed(&root, "2026-11-14").expect("seed");
        project_memory::onboarded(&root, "2026-11-14").expect("learned");
    }
    root
}

#[test]
fn a_project_is_off_until_the_person_turns_it_on_and_labs_only_makes_it_available() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let projects = [
        project("api", &folder(dir.path(), "api", false), None),
        project("web", &folder(dir.path(), "web", false), None),
    ];
    assert!(!on_for(&store, &projects[0]), "off by default");
    assert!(
        turn(&store, &projects, "api", true).expect("on"),
        "turned on, it reads the project it never learned"
    );
    assert!(on_for(&store, &projects[0]));
    assert!(!on_for(&store, &projects[1]), "only the project turned on");
    store
        .set_preference_flag(devpit_core::preference::LABS_PROJECT_AGENT, false)
        .expect("flag");
    assert!(
        !on_for(&store, &projects[0]),
        "Labs off leaves every project out"
    );
    store
        .set_preference_flag(devpit_core::preference::LABS_PROJECT_AGENT, true)
        .expect("flag");
    assert!(!turn(&store, &projects, "api", false).expect("off"));
    assert!(!on_for(&store, &projects[0]));
}

#[test]
fn turning_on_a_project_already_learned_does_not_read_it_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let projects = [project("api", &folder(dir.path(), "api", true), None)];
    turn(&store, &projects, "api", false).expect("off");
    assert!(!turn(&store, &projects, "api", true).expect("on"));
    assert!(on_for(&store, &projects[0]));
}

#[test]
fn on_update_a_project_already_learned_stays_on_unless_labs_left_it_out() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let projects = [
        project("api", &folder(dir.path(), "api", true), None),
        project("web", &folder(dir.path(), "web", true), None),
        project("cli", &folder(dir.path(), "cli", false), None),
    ];
    store
        .set_preference(
            devpit_core::preference::LABS_PROJECT_AGENT_OFF,
            r#"["web"]"#,
        )
        .expect("left out");
    let on = |store: &devpit_core::Store| -> Vec<bool> {
        projects.iter().map(|one| on_for(store, one)).collect()
    };
    assert_eq!(on(&store), [true, false, false], "read as it was left");
    carry_over(&store, &projects, "2026-11-15").expect("carried over");
    assert_eq!(on(&store), [true, false, false], "and kept so");
    assert_eq!(
        store
            .preference(devpit_core::preference::LABS_PROJECT_AGENT_ON)
            .expect("read")
            .as_deref(),
        Some(r#"["api"]"#)
    );
}

#[test]
fn on_update_the_onboarding_proposals_waiting_are_accepted_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let root = folder(dir.path(), "api", true);
    std::fs::write(root.join("AGENTS.md"), "# Working here\n").expect("agents");
    let projects = [project("api", &root, None)];
    let day = "2026-11-14";
    let fact = project_memory::aimed("facts.md", "Everything versioned is in English.", None);
    let decision = project_memory::aimed("decisions.md", "The workspace syncs.", None);
    project_memory::propose(&root, day, "onboarding, AGENTS.md", &[fact, decision])
        .expect("proposed");
    project_memory::propose(&root, day, "#12", &["Wait for the queue.".to_owned()])
        .expect("proposed");

    assert_eq!(
        carry_over(&store, &projects, "2026-11-15").expect("carried over"),
        ["api"]
    );
    let waiting = project_memory::proposals(&root);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].source, "#12", "only what onboarding proposed");
    let kept = |file: &str| -> Vec<(String, String)> {
        project_memory::kept(&root, file)
            .into_iter()
            .filter(|one| one.day == "2026-11-15")
            .map(|one| (one.source, one.text))
            .collect()
    };
    assert_eq!(
        kept("facts.md"),
        [(
            "AGENTS.md".to_owned(),
            "Everything versioned is in English.".to_owned()
        )]
    );
    assert_eq!(
        kept("decisions.md"),
        [("AGENTS.md".to_owned(), "The workspace syncs.".to_owned())]
    );

    let later = project_memory::aimed("decisions.md", "A later read.", None);
    project_memory::propose(&root, day, "onboarding, AGENTS.md", &[later]).expect("proposed");
    assert!(carry_over(&store, &projects, "2026-11-16")
        .expect("once")
        .is_empty());
    assert_eq!(
        project_memory::proposals(&root).len(),
        2,
        "a later read waits"
    );
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

#[test]
fn nothing_turns_a_project_on_without_the_persons_answer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let orch = dir.path().join("orch");
    std::fs::create_dir_all(&orch).expect("orch");
    let projects = [
        project("orch", &orch, Some("claude")),
        project("api", &folder(dir.path(), "api", false), None),
        project("web", &folder(dir.path(), "web", false), None),
    ];
    let link = |ids: &[&str]| -> bool {
        let ids = ids.iter().map(|id| (*id).to_owned()).collect();
        crate::orchestrator_links::link_in(&store, &orch, &projects, ids).expect("linked")
    };
    let shown = || -> Vec<(String, bool, bool)> {
        labs_projects(&projects, &store)
            .into_iter()
            .map(|one| (one.id, one.on, one.asked))
            .collect()
    };
    let off = || projects.iter().all(|one| !on_for(&store, one));

    assert!(link(&["api", "web"]), "linking asks the person");
    assert_eq!(
        shown(),
        [("api".into(), false, true), ("web".into(), false, true)]
    );
    carry_over(&store, &projects, "2026-11-15").expect("carried over");
    assert!(off(), "linking and the carry-over turn nothing on");
    assert!(!learning("api") && !learning("web"), "and read nothing");
    assert!(!link(&["api", "web"]), "a project waiting is asked once");

    assert!(!turn(&store, &projects, "web", false).expect("not now"));
    link(&["api"]);
    assert!(!link(&["api", "web"]), "Not now is never asked again");
    assert!(off(), "and leaves it off");

    assert!(turn(&store, &projects, "api", true).expect("turn on"));
    assert_eq!(
        shown(),
        [("api".into(), true, false), ("web".into(), false, false)]
    );
}

#[test]
fn linking_asks_nothing_while_labs_has_the_project_agent_off_or_of_a_project_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store(dir.path());
    let orch = dir.path().join("orch");
    std::fs::create_dir_all(&orch).expect("orch");
    let projects = [
        project("orch", &orch, Some("claude")),
        project("api", &folder(dir.path(), "api", true), None),
        project("web", &folder(dir.path(), "web", false), None),
    ];
    let link = |ids: &[&str]| -> bool {
        let ids = ids.iter().map(|id| (*id).to_owned()).collect();
        crate::orchestrator_links::link_in(&store, &orch, &projects, ids).expect("linked")
    };
    assert!(!link(&["api"]), "learned before, so on: nothing to ask");
    store
        .set_preference_flag(devpit_core::preference::LABS_PROJECT_AGENT, false)
        .expect("flag");
    assert!(!link(&["api", "web"]), "Labs off asks nothing");
}
