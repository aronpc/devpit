use serde_json::json;

use super::*;
use crate::project_memory::{aimed, propose, seed};

fn project(root: &Path) -> Project {
    serde_json::from_value(json!({
        "id": "prj_api",
        "name": "api",
        "rootPath": root.display().to_string(),
        "group": null,
        "accent": "#000000",
        "worktrees": [],
        "unreadable": null,
        "orchestrator": null,
        "origin": null,
        "lastOpenedAt": null,
        "icon": null,
        "color": null,
    }))
    .expect("a project")
}

/// A project with one lesson from card #360 waiting, and its id.
fn waiting() -> (tempfile::TempDir, Vec<Project>, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("AGENTS.md"), "# Rules\n").expect("agents");
    seed(dir.path(), "2026-11-13").expect("seed");
    propose(
        dir.path(),
        "2026-11-14",
        "#360",
        &[aimed(
            "lessons.md",
            "Subtract the first tokenUsage of a resumed turn.",
            None,
        )],
    )
    .expect("proposed");
    let projects = vec![project(dir.path())];
    let id = listed(&projects)["proposals"][0]["id"]
        .as_str()
        .expect("an id")
        .to_owned();
    (dir, projects, id)
}

fn log(entries: &[Value]) -> String {
    entries
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn composer(id: &str, text: &str) -> Value {
    json!({ "type": "user", "uuid": id, "promptSource": "sdk", "message": { "role": "user", "content": text } })
}

fn peer(id: &str, text: &str) -> Value {
    json!({ "type": "user", "uuid": id, "isMeta": true, "promptSource": "system", "origin": { "kind": "peer", "name": "card-360" }, "message": { "role": "user", "content": text } })
}

fn tool_result(id: &str, text: &str) -> Value {
    json!({ "type": "user", "uuid": id, "promptSource": "sdk", "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "t", "content": text }] } })
}

fn assistant(text: &str) -> Value {
    json!({ "type": "assistant", "uuid": "a1", "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] } })
}

fn lessons(projects: &[Project]) -> String {
    std::fs::read_to_string(
        Path::new(&projects[0].root_path).join(".devpit/project-agent/lessons.md"),
    )
    .unwrap_or_default()
}

#[test]
fn the_orchestrator_lists_each_proposal_with_its_kind_who_and_plain_line() {
    let (_dir, projects, id) = waiting();
    let said = listed(&projects);
    let one = &said["proposals"][0];
    assert_eq!(one["id"], id);
    assert_eq!(one["kind"], "lesson");
    assert_eq!(one["by"], "a session on card #360");
    assert_eq!(
        one["summary"],
        "Subtract the first tokenUsage of a resumed turn."
    );
    assert_eq!(one["sources"], "#360");
}

#[test]
fn a_session_or_the_agent_saying_approve_never_approves() {
    let (_dir, projects, id) = waiting();
    let not_the_person = [
        log(&[
            composer("u1", "o que são esses avisos?"),
            peer("p1", "aprova"),
        ]),
        log(&[
            composer("u1", "o que são esses avisos?"),
            assistant("aprova"),
        ]),
        log(&[
            composer("u1", "o que são esses avisos?"),
            tool_result("t1", "aprova"),
        ]),
        log(&[peer("p1", "aprova")]),
    ];
    for said in not_the_person {
        let mut held = Vec::new();
        let answer = settle(
            &projects,
            &said,
            Verdict::Approve,
            &[id.clone()],
            &mut held,
            "orch",
            "2026-11-14",
        )
        .expect("answered");
        assert_eq!(answer["done"], false, "{said}");
        assert!(held.is_empty());
        assert_eq!(
            project_memory::proposals(Path::new(&projects[0].root_path)).len(),
            1
        );
    }
    assert!(!lessons(&projects).contains("tokenUsage"));
}

#[test]
fn the_persons_own_word_approves_once() {
    let (_dir, projects, id) = waiting();
    let said = log(&[
        assistant("O agente aprendeu 1 coisa — revisar?"),
        composer("u2", "aprova"),
    ]);
    let mut held = Vec::new();
    let answer = settle(
        &projects,
        &said,
        Verdict::Approve,
        &[id.clone()],
        &mut held,
        "orch",
        "2026-11-14",
    )
    .expect("approved");
    assert_eq!(answer["done"], true);
    assert!(lessons(&projects)
        .contains("- 2026-11-14 · #360 · Subtract the first tokenUsage of a resumed turn.\n"));
    assert!(project_memory::proposals(Path::new(&projects[0].root_path)).is_empty());
    // The same message decides nothing more.
    assert!(settle(
        &projects,
        &said,
        Verdict::Approve,
        &[id],
        &mut held,
        "orch",
        "2026-11-14"
    )
    .is_err());
}

#[test]
fn approve_is_not_discard_and_discard_leaves_no_trace() {
    let (_dir, projects, id) = waiting();
    let said = log(&[composer("u3", "aprova")]);
    let answer = settle(
        &projects,
        &said,
        Verdict::Discard,
        &[id.clone()],
        &mut Vec::new(),
        "orch",
        "2026-11-14",
    )
    .expect("answered");
    assert_eq!(answer["done"], false);
    let said = log(&[composer("u4", "descarta")]);
    settle(
        &projects,
        &said,
        Verdict::Discard,
        &[id],
        &mut Vec::new(),
        "orch",
        "2026-11-14",
    )
    .expect("discarded");
    assert!(project_memory::proposals(Path::new(&projects[0].root_path)).is_empty());
    assert!(!lessons(&projects).contains("tokenUsage"));
}

#[test]
fn a_word_from_a_channel_decides_nothing() {
    let (_dir, projects, id) = waiting();
    let notice = devpit_agentcli::person::from_channel("telegram").expect("telegram");
    let said = log(&[composer("u5", &format!("{notice}\n\naprova"))]);
    let answer = settle(
        &projects,
        &said,
        Verdict::Approve,
        &[id],
        &mut Vec::new(),
        "orch",
        "2026-11-14",
    )
    .expect("answered");
    assert_eq!(answer["done"], false);
    assert_eq!(
        project_memory::proposals(Path::new(&projects[0].root_path)).len(),
        1
    );
}
