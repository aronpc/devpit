//! The flow end to end on the engine, as an agent calls devpit's tools:
//! brief → plan → evidence → gate → finish, on one project's board.

use std::path::Path;

use devflow_engine::Ask;
use devpit_rpc::Project;
use serde_json::{json, Value};

const UNIT: &str = r#"printf '<testsuite><testcase classname="cli" name="rename changes the title"/></testsuite>' > "$DEVPIT_REPORT_DIR/report.xml""#;

fn ask(project: &Project, root: &Path, method: &str, params: Value) -> Value {
    crate::devflow::respond_from(
        devpit_core::locale::Locale::En,
        None,
        method,
        project,
        root,
        None,
        &params,
    )
    .expect(method)
}

/// Sabotage: make `Board::create` drop the epic, or point the dialect at
/// `.devflow/`, and the plan, the evidence or the gate breaks here.
#[test]
fn a_brief_becomes_a_planned_card_that_finishes_only_through_the_gate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let store = crate::projects::store().expect("store");
    let id = store.add_project(&root, None).expect("project");
    store.ensure_board(&id).expect("board");
    let yaml = format!(
        "version: 1\nenabled: true\nstages:\n  review:\n    enabled: false\n  execution:\n    checks:\n      - name: unit\n        run: '{}'\n        report: junit\n",
        UNIT.replace('\'', "''")
    );
    std::fs::create_dir_all(root.join(".devpit/devflow")).expect("folder");
    std::fs::write(root.join(".devpit/devflow/devflow.yaml"), yaml).expect("yaml");
    crate::devflow_asks::remember(&store, &id, Ask::Enable, true).expect("clicked");
    let project: Project = serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": root.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": null,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project");

    let brief = ask(
        &project,
        &root,
        "devflow_brief",
        json!({ "title": "Rename", "goal": "cards can be renamed", "success": "cargo test" }),
    );
    let epic = brief["cardId"].as_str().expect("epic").to_owned();
    let cards = json!([{
        "key": "s1", "title": "Rename a card",
        "criteria": ["rename changes the title"], "depends_on": []
    }]);
    ask(
        &project,
        &root,
        "devflow_plan",
        json!({ "epic": epic, "cards": cards }),
    );
    let applied = ask(
        &project,
        &root,
        "devflow_plan",
        json!({ "epic": epic, "cards": cards, "apply": true }),
    );
    let card = applied["cards"][0]["cardId"]
        .as_str()
        .expect("card")
        .to_owned();
    assert_eq!(
        store.card_edges(&id).expect("edges").parents,
        vec![(card.clone(), epic.clone())]
    );
    store
        .set_card_front(&card, Some(&root.display().to_string()), Some("HEAD"))
        .expect("its checkout");

    let refused =
        crate::devflow_gate::may_finish(devpit_core::locale::Locale::En, &project, &card, &root)
            .expect_err("no run");
    assert!(refused.contains("the card stays where it is"), "{refused}");
    let ran = ask(
        &project,
        &root,
        "devflow_evidence",
        json!({ "card": card, "criteria": [{ "criterion": "rename changes the title", "tests": ["rename changes the title"] }] }),
    );
    assert_eq!(ran["green"], true, "{ran}");
    assert!(ran["run"]
        .as_str()
        .is_some_and(|run| !run.starts_with(".devflow")));
    let gate = ask(&project, &root, "devflow_gate", json!({ "card": card }));
    assert_eq!(gate["result"], "passed", "{gate}");
    crate::devflow_gate::may_finish(devpit_core::locale::Locale::En, &project, &card, &root)
        .expect("finishes");
    assert!(
        !root.join(".devflow").exists(),
        "nothing of devflow's own folder"
    );
}
