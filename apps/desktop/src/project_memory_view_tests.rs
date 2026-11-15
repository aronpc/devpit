use serde_json::json;

use super::*;
use crate::project_memory::{aimed, propose, seed, ONBOARDING};

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

#[test]
fn the_page_lists_the_memory_and_what_waits_each_with_its_sources() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("AGENTS.md"), "# Working in this repository\n").expect("agents");
    let before = memory_of(&project(root), true);
    assert!(!before.kept && before.facts.is_empty() && before.proposals.is_empty());

    seed(root, "2026-11-13").expect("seed");
    propose(
        root,
        "2026-11-14",
        &format!("{ONBOARDING}, AGENTS.md"),
        &[aimed(
            "facts.md",
            "Everything versioned is in English.",
            None,
        )],
    )
    .expect("learned");
    propose(
        root,
        "2026-11-14",
        "#12, AGENTS.md",
        &["Wait for the queue.".to_owned()],
    )
    .expect("lesson");

    let page = memory_of(&project(root), true);
    assert!(page.kept);
    assert_eq!(page.facts[0].files, ["AGENTS.md"]);
    assert_eq!(page.facts[0].day, "2026-11-13");
    let learned = &page.proposals[0];
    assert!(learned.proposal.onboarding);
    assert_eq!(learned.files, ["AGENTS.md"]);
    assert_eq!(learned.proposal.file, "facts.md");
    assert_eq!(learned.proposal.text, "Everything versioned is in English.");
    let lesson = &page.proposals[1];
    assert!(!lesson.proposal.onboarding);
    assert_eq!(lesson.cards, ["#12"]);
    assert_eq!(lesson.proposal.file, "lessons.md");
    assert_eq!(page.learned, None);
}
