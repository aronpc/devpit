use devflow_engine::Ask;
use serde_json::json;

use super::*;

fn project_at(id: &str, root: &Path) -> Project {
    serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": root.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": null,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project")
}

/// A project on version 2 with an own stage gated by `command`, switched on,
/// and a card on its board.
fn flow_with(dir: &Path, command: &str, publish: &str) -> (Store, Project, String) {
    flow_gated(dir, &format!("{{ command: '{command}' }}"), publish, "")
}

/// [`flow_with`], the own stage's `gate` as given and `extra` after the stages.
fn flow_gated(dir: &Path, gate: &str, publish: &str, extra: &str) -> (Store, Project, String) {
    let store = crate::projects::store().expect("store");
    let id = store.add_project(dir, None).expect("project");
    store.ensure_board(&id).expect("board");
    let yaml = format!(
        "version: 2\nenabled: true\nstages:\n  - uses: builtin/plan\n    publish: {publish}\n  - uses: builtin/execution\n    checks:\n      - {{ name: unit, run: 'true' }}\n  - uses: project/pr\n    gate: {gate}\n  - uses: builtin/completion\n{extra}"
    );
    std::fs::create_dir_all(dir.join(".devpit/devflow/skills/pr")).expect("folder");
    std::fs::write(dir.join(".devpit/devflow/devflow.yaml"), yaml).expect("yaml");
    std::fs::write(
        dir.join(".devpit/devflow/skills/pr/SKILL.md"),
        "---\nname: pr\ndescription: Open the pull request.\n---\nOpen it.\n",
    )
    .expect("skill");
    crate::devflow_asks::remember(&store, &id, Ask::Enable, true).expect("clicked");
    let first = store.columns(&id).expect("columns")[0].id.clone();
    let card = store
        .create_card(&id, &first, "[devflow] Open the pull request", "")
        .expect("card");
    (store, project_at(&id, dir), card)
}

fn ask(project: &Project, root: &Path, method: &str, params: Value) -> Result<Value, String> {
    crate::devflow::respond_from(
        devpit_core::locale::Locale::En,
        None,
        method,
        project,
        root,
        None,
        &params,
    )
}

#[test]
fn an_own_stage_with_a_command_gate_runs_in_the_flow() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = flow_with(&root, "test \"$DEVFLOW_STAGE\" = pr", "board");

    let skill = ask(&project, &root, "devflow_stage", json!({ "stage": "pr" })).expect("skill");
    assert_eq!(skill["layer"], "project");
    assert_eq!(skill["gate"]["command"], "test \"$DEVFLOW_STAGE\" = pr");

    // The completion gate holds the card until the stage's gate passed.
    let before = ask(&project, &root, "devflow_gate", json!({ "card": card })).expect("gate");
    let missing = before["missing"].to_string();
    assert!(missing.contains("the stage `pr`"), "{missing}");

    let passed = ask(
        &project,
        &root,
        "devflow_stage",
        json!({ "stage": "pr", "card": card }),
    )
    .expect("ran");
    assert_eq!(passed["result"], "passed", "{passed}");
    let record = root.join(passed["record"].as_str().expect("record"));
    let kept: Value =
        serde_json::from_str(&std::fs::read_to_string(record).expect("kept")).expect("json");
    assert_eq!(kept["passed"], true);
    let after = ask(&project, &root, "devflow_gate", json!({ "card": card })).expect("gate");
    assert!(
        !after["missing"].to_string().contains("the stage `pr`"),
        "{after}"
    );
}

#[test]
fn a_stage_whose_command_fails_holds_the_card_with_what_it_said() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = flow_with(&root, "echo checklist incomplete; exit 3", "board");
    let failed = ask(
        &project,
        &root,
        "devflow_stage",
        json!({ "stage": "pr", "card": card }),
    )
    .expect("ran");
    assert_eq!(failed["result"], "failed");
    assert!(failed["said"].to_string().contains("exited 3"), "{failed}");
    assert!(
        failed["said"].to_string().contains("checklist incomplete"),
        "{failed}"
    );
    let gate = ask(&project, &root, "devflow_gate", json!({ "card": card })).expect("gate");
    assert!(
        gate["missing"].to_string().contains("gate failed"),
        "{gate}"
    );
}

#[test]
fn a_plan_published_to_jira_is_deferred_with_the_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, _card) = flow_with(&root, "true", "jira");
    let refused = ask(
        &project,
        &root,
        "devflow_plan",
        json!({ "epic": "card_none", "cards": [] }),
    )
    .expect_err("not yet");
    assert!(refused.contains("publish: jira is not yet"), "{refused}");
    assert!(refused.contains("O-08"), "{refused}");
}

/// However Decisions stands, a stage it judges says so, and in shadow passes.
#[test]
fn a_rubric_stage_says_where_decisions_stands_in_each_standing() {
    use crate::devflow_jev::tests::STANDINGS;
    for standing in STANDINGS {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let (_store, project, card) =
            flow_gated(&root, "{ rubric: devflow-plan }", "board", standing.yaml());
        standing.give(&project.id);
        let said = ask(
            &project,
            &root,
            "devflow_stage",
            json!({ "stage": "pr", "card": card }),
        )
        .expect("ran");
        assert_eq!(said["result"], "passed", "{standing:?}: {said}");
        standing.said_in(&said);
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = flow_with(&root, "true", "board");
    let said = ask(
        &project,
        &root,
        "devflow_stage",
        json!({ "stage": "pr", "card": card }),
    )
    .expect("ran");
    assert!(
        said.get("decisions").is_none(),
        "a command asks nothing: {said}"
    );
}
