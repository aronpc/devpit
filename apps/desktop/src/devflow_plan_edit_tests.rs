use std::path::Path;

use devpit_rpc::Project;
use serde_json::json;

use super::*;

/// A project with three cards, the third waiting on the second.
fn three_cards(dir: &Path) -> (Store, Project, Vec<String>) {
    let store = crate::projects::store().expect("store");
    let id = store.add_project(dir, None).expect("project");
    store.ensure_board(&id).expect("board");
    let first = store.columns(&id).expect("columns")[0].id.clone();
    let cards: Vec<String> = ["[devflow] index", "[devflow] box", "[devflow] highlight"]
        .iter()
        .map(|title| store.create_card(&id, &first, title, "").expect("card"))
        .collect();
    store
        .conn()
        .execute(
            "INSERT INTO card_blocker (card_id, blocked_by, created_at) VALUES (?1, ?2, 0)",
            [&cards[2], &cards[1]],
        )
        .expect("waits");
    let project = serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": dir.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": null,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project");
    (store, project, cards)
}

fn plan(project: &Project, params: Value) -> Result<Value, String> {
    crate::devflow_plan::respond(
        devpit_core::locale::Locale::En,
        None,
        "devflow_plan",
        project,
        &params,
    )
}

#[test]
fn a_merge_or_a_cut_without_the_persons_yes_only_previews() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project, cards) = three_cards(dir.path());
    let number = store
        .card_tally(&cards[1])
        .expect("tally")
        .number
        .expect("numbered");

    let said = plan(&project, json!({ "cut": format!("#{number}") })).expect("preview");
    assert_eq!(said["applied"], false);
    assert_eq!(said["releases"][0]["id"], cards[2].as_str());
    let said = plan(
        &project,
        json!({ "merge": { "from": cards[1], "into": cards[0] } }),
    )
    .expect("preview");
    assert_eq!(said["applied"], false);
    assert_eq!(store.card_closed(&cards[1]).expect("read"), None);

    let said = plan(
        &project,
        json!({ "merge": { "from": cards[1], "into": cards[0] }, "apply": true }),
    )
    .expect("merged");
    assert_eq!(said["applied"], true);
    assert_eq!(
        store.card_closed(&cards[1]).expect("read"),
        Some(Some(cards[0].clone()))
    );
    let comments = store.comments(&cards[1]).expect("comments");
    assert!(
        comments.iter().any(|one| one.body.contains("merged into")),
        "the card says where its work went"
    );
}

#[test]
fn a_card_is_merged_into_one_that_can_take_its_work_and_one_change_is_asked_at_a_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project, cards) = three_cards(dir.path());
    let refused = |params: Value| plan(&project, params).expect_err("refused");
    assert!(refused(json!({ "merge": { "from": cards[0], "into": cards[0] } })).contains("itself"));
    assert!(refused(json!({ "merge": { "from": cards[0] } })).contains("merge.into"));
    assert!(
        refused(json!({ "cut": cards[0], "merge": { "from": cards[1], "into": cards[2] } }))
            .contains("not both")
    );
    assert!(refused(json!({ "cut": "card_nowhere" })).contains("no card"));
    store.close_card(&cards[2], None).expect("cut");
    assert!(
        refused(json!({ "merge": { "from": cards[1], "into": cards[2] }, "apply": true }))
            .contains("merged or cut")
    );
    assert_eq!(store.card_closed(&cards[1]).expect("read"), None);
}
