use devflow_engine::Ask;
use serde_json::json;

use super::*;

fn project_at(id: &str, root: &Path, worktrees: &[&Path]) -> Project {
    let worktrees: Vec<Value> = worktrees
        .iter()
        .map(|path| {
            json!({
                "id": path.display().to_string(), "branch": "card", "folder": "card",
                "path": path.display().to_string(), "ahead": 0, "behind": 0,
                "dirtyFiles": null, "origin": "devpit", "current": false,
            })
        })
        .collect();
    serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": root.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": worktrees, "unreadable": null, "orchestrator": null,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project")
}

/// A JUnit report, a screenshot and a video, written where the check is told to.
const E2E: &str = r#"mkdir -p "$DEVPIT_REPORT_DIR/test-results/rename" && printf png > "$DEVPIT_REPORT_DIR/test-results/rename/test-finished-1.png" && printf webm > "$DEVPIT_REPORT_DIR/test-results/rename/video.webm" && printf '<testsuite><testcase classname="rename.spec.ts" name="rename changes the title"/></testsuite>' > "$DEVPIT_REPORT_DIR/report.xml""#;

/// A project in the store with Devflow switched on and one e2e check, and a
/// card on its board.
fn devflow_on(dir: &Path) -> (devpit_core::Store, Project, String) {
    let store = crate::projects::store().expect("store");
    let id = store.add_project(dir, None).expect("project");
    store.ensure_board(&id).expect("board");
    let yaml = format!(
        "version: 1\nenabled: true\nstages:\n  execution:\n    checks:\n      - name: e2e\n        run: '{}'\n        report: junit\n        evidence: [screenshots, video]\n",
        E2E.replace('\'', "''")
    );
    std::fs::create_dir_all(dir.join(".devpit/devflow")).expect("folder");
    std::fs::write(dir.join(".devpit/devflow/devflow.yaml"), yaml).expect("yaml");
    crate::devflow_asks::remember(&store, &id, Ask::Enable, true).expect("clicked");
    let first = store.columns(&id).expect("columns")[0].id.clone();
    let card = store
        .create_card(
            &id,
            &first,
            "[devflow] Rename a card",
            "## Acceptance criteria\n- [ ] rename changes the title\n",
        )
        .expect("card");
    (store, project_at(&id, dir, &[]), card)
}

#[test]
fn running_a_cards_checks_keeps_the_run_and_attaches_it_to_the_card() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, card) = devflow_on(&root);
    let said = crate::devflow::respond_from(
 devpit_core::locale::Locale::En,
 None,
        "devflow_evidence",
        &project,
        &root, None,
        &json!({
            "card": card,
            "criteria": [{ "criterion": "rename changes the title", "tests": ["rename changes the title"] }]
        }))
    .expect("evidence");

    assert_eq!(said["green"], true, "{said}");
    assert_eq!(said["checks"][0]["result"], "passed");
    assert_eq!(said["criteria_green"], "1 of 1");
    let run = root.join(said["run"].as_str().expect("the run's folder"));
    let home = runs_of(&project.id, &root).expect("runs").home;
    assert!(run.starts_with(home.join(&card)), "{run:?}");
    assert!(
        !root.join(".devpit/devflow/runs").exists(),
        "a run in the repository"
    );
    for kept in [
        "e2e/report.xml",
        "e2e.log",
        "evidence.json",
        "screenshots/e2e-test-results-rename-test-finished-1.png",
        "video/e2e-test-results-rename-video.webm",
    ] {
        assert!(run.join(kept).is_file(), "{kept} in the run");
    }

    let under = said["attached"]["under"]
        .as_str()
        .expect("under")
        .to_owned();
    assert_eq!(said["attached"]["files"], 5);
    let artifacts = crate::projects::project_home(&project.id)
        .expect("home")
        .artifacts();
    let picture = artifacts
        .join(&under)
        .join("screenshots/e2e-test-results-rename-test-finished-1.png");
    assert!(picture.is_file());

    let comments = store.comments(&card).expect("comments");
    let evidence = comments.last().expect("a comment");
    assert_eq!(evidence.author, SIGNED);
    assert!(
        evidence
            .body
            .starts_with("[devflow] evidence: 1 of 1 checks green"),
        "{}",
        evidence.body
    );
    // A link the card opens in devpit, the picture's whole path.
    assert!(
        evidence.body.contains(&format!("({})", picture.display())),
        "{}",
        evidence.body
    );
}

#[test]
fn a_card_of_another_project_is_not_this_ones_to_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, _) = devflow_on(&root);
    let other = tempfile::tempdir().expect("other");
    let elsewhere = store.add_project(other.path(), None).expect("project");
    store.ensure_board(&elsewhere).expect("board");
    let lane = store.columns(&elsewhere).expect("columns")[0].id.clone();
    let theirs = store
        .create_card(&elsewhere, &lane, "Theirs", "")
        .expect("card");
    let refused = crate::devflow::respond_from(
        devpit_core::locale::Locale::En,
        None,
        "devflow_evidence",
        &project,
        &root,
        None,
        &json!({ "card": theirs }),
    )
    .expect_err("not on this board");
    assert!(refused.contains("no card"), "{refused}");
    assert!(!root.join(".devpit/devflow/runs").exists());
}

#[test]
fn the_checks_run_in_the_checkout_the_agent_stands_in() {
    let root = tempfile::tempdir().expect("root");
    let tree = tempfile::tempdir().expect("worktree");
    let (root_path, tree_path) = (
        root.path().canonicalize().expect("root"),
        tree.path().canonicalize().expect("tree"),
    );
    std::fs::create_dir_all(tree_path.join("src")).expect("src");
    let project = project_at("prj_checkout", &root_path, &[&tree_path]);
    assert_eq!(checkout(&project, &tree_path.join("src")), tree_path);
    assert_eq!(checkout(&project, &root_path), root_path);
    // Standing nowhere of the project's: its own root.
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    assert_eq!(checkout(&project, elsewhere.path()), root_path);
}
