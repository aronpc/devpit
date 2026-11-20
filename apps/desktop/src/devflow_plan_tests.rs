use std::path::Path;

use super::*;

/// A project in the store, with the board it starts with.
fn project(dir: &Path) -> (Store, Project) {
    let store = crate::projects::store().expect("store");
    let id = store.add_project(dir, None).expect("project");
    store.ensure_board(&id).expect("board");
    let project = serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": dir.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": null,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project");
    (store, project)
}

fn cards() -> Value {
    json!([
        {
            "key": "s1", "title": "Cards are indexed by their body",
            "build": "A word in a card's body finds the card.",
            "criteria": ["a word in a body finds its card", "an empty query finds nothing"],
            "files": ["crates/core/src/store/search_index.rs"],
            "verify": "cargo test -p devpit-core search_index", "depends_on": []
        },
        { "key": "s2", "title": "The search box lists them", "criteria": ["typing lists"], "depends_on": ["s1"] },
        { "key": "s3", "title": "Matches are highlighted", "depends_on": ["s2"] }
    ])
}

fn titles(store: &Store, project: &Project) -> Vec<String> {
    let mut titles: Vec<String> = store
        .cards(&project.id)
        .expect("cards")
        .into_iter()
        .map(|card| card.title)
        .collect();
    titles.sort();
    titles
}

fn epic(store: &Store, project: &Project) -> String {
    let said = respond(
        Locale::En,
        None,
        "devflow_brief",
        project,
        &json!({
            "title": "Search by body", "goal": "Find a card by its body",
            "for_whom": "people with long boards", "success": "make test passes",
            "out_of_scope": ["comments"], "constraints": [], "decisions": ["one index"]
        }),
    )
    .expect("a brief");
    assert_eq!(said["created"], true);
    let id = said["cardId"].as_str().expect("an id").to_owned();
    assert_eq!(
        store.card(&id).expect("read").expect("card").title,
        "[devflow] Brief: Search by body"
    );
    id
}

#[test]
fn a_brief_becomes_an_epic_and_the_context_is_written_onto_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let epic = epic(&store, &project);
    let said = respond(
            Locale::En,
            None,
        "devflow_brief",
        &project,
        &json!({ "card": epic, "context": "## Context\n### Facts\n- titles are indexed (`a.rs:1`)" }),
    )
    .expect("the context");
    assert_eq!(said["created"], false);
    let note = said["note"].as_str().expect("a note");
    assert!(note.contains("the plan (devflow-plan)"), "{note}");
    let body = store.card(&epic).expect("read").expect("card").body;
    assert!(
        body.starts_with("## Brief\n- Goal: Find a card by its body"),
        "{body}"
    );
    assert!(
        body.ends_with("## Context\n### Facts\n- titles are indexed (`a.rs:1`)"),
        "{body}"
    );
    assert_eq!(titles(&store, &project).len(), 1);

    let elsewhere = respond(
        Locale::En,
        None,
        "devflow_brief",
        &project,
        &json!({ "card": "card_nope", "goal": "x" }),
    );
    assert!(elsewhere.expect_err("no such card").contains("no card"));
}

/// The context stage switched off is not where a brief sends the agent.
#[test]
fn a_brief_points_past_the_context_stage_when_it_is_off() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_store, project) = project(dir.path());
    std::fs::create_dir_all(dir.path().join(".devpit/devflow")).expect("folder");
    std::fs::write(
        dir.path().join(".devpit/devflow/devflow.yaml"),
        "version: 1\nenabled: true\nstages:\n  context: { enabled: false }\n",
    )
    .expect("yaml");
    let said = respond(
        Locale::En,
        None,
        "devflow_brief",
        &project,
        &json!({ "title": "Search by body", "goal": "Find a card by its body" }),
    )
    .expect("a brief");
    let note = said["note"].as_str().expect("a note");
    assert!(note.contains("the plan (devflow-plan)"), "{note}");
    assert!(after_brief(false, true).contains("devflow-context"));
}

#[test]
fn a_plan_is_a_preview_until_the_persons_yes_and_then_cards_that_wait_on_each_other() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let epic = epic(&store, &project);
    let plan = |cards: Value, apply: bool| {
        respond(
            Locale::En,
            None,
            "devflow_plan",
            &project,
            &json!({ "epic": epic, "cards": cards, "apply": apply }),
        )
        .expect("a plan")
    };

    let preview = plan(cards(), false);
    assert_eq!(preview["applied"], false);
    assert_eq!(
        preview["preview"][1]["title"],
        "[devflow] The search box lists them"
    );
    assert_eq!(preview["preview"][1]["blockedBy"], json!(["s1"]));
    assert_eq!(titles(&store, &project).len(), 1, "a preview created cards");
    assert_eq!(waiting(&store, &epic).expect("waiting").cards.len(), 3);

    // Cards the person has not seen are a new preview, not a yes.
    let mut changed = cards();
    changed[2]["title"] = json!("Matches are bold");
    assert_eq!(plan(changed, true)["applied"], false);
    assert_eq!(
        titles(&store, &project).len(),
        1,
        "unseen cards were created"
    );

    let mut seen = cards();
    seen[2]["title"] = json!("Matches are bold");
    let applied = plan(seen.clone(), true);
    assert_eq!(applied["applied"], true, "{applied}");
    assert_eq!(applied["approvedOnBoard"], false);
    let ids: Vec<String> = applied["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|card| card["cardId"].as_str().expect("id").to_owned())
        .collect();
    assert_eq!(applied["startsNow"], json!([ids[0]]));
    assert_eq!(
        titles(&store, &project),
        [
            "[devflow] Brief: Search by body",
            "[devflow] Cards are indexed by their body",
            "[devflow] Matches are bold",
            "[devflow] The search box lists them",
        ]
    );
    let edges = store.card_edges(&project.id).expect("edges");
    assert_eq!(
        edges.blockers,
        [
            (ids[1].clone(), ids[0].clone()),
            (ids[2].clone(), ids[1].clone())
        ]
    );
    assert!(edges.parents.iter().all(|(_, parent)| parent == &epic));
    let body = store.card(&ids[0]).expect("read").expect("card").body;
    assert_eq!(devflow_engine::criteria::criteria(&body).len(), 2, "{body}");
    let said = store.comments(&epic).expect("comments");
    assert!(
        said[0]
            .body
            .starts_with("[devflow] Plan created after the person's yes"),
        "{}",
        said[0].body
    );

    // Applied twice, it is created once.
    assert_eq!(plan(seen, true)["cards"][0]["cardId"], json!(ids[0]));
    assert_eq!(titles(&store, &project).len(), 4);
}

#[test]
fn a_plan_the_person_edited_and_created_on_the_board_is_the_agents_answer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let epic = epic(&store, &project);
    respond(
        Locale::En,
        None,
        "devflow_plan",
        &project,
        &json!({ "epic": epic, "cards": cards() }),
    )
    .expect("a preview");

    // The person cuts the last card and renames the second.
    let mut edited = waiting(&store, &epic).expect("waiting").cards;
    edited.pop();
    edited[1].title = "The search box lists matches".into();
    let kept = kept_on_board(&store, &project.id, &epic, edited).expect("kept");
    assert!(kept.edited);
    let made = created_on_board(&store, &project.id, &epic, None).expect("created");
    assert_eq!(made.card_ids.len(), 2);
    assert!(waiting(&store, &epic).is_none());

    let answer = respond(
        Locale::En,
        None,
        "devflow_plan",
        &project,
        &json!({ "epic": epic, "cards": cards(), "apply": true }),
    )
    .expect("the answer");
    assert_eq!(answer["approvedOnBoard"], true);
    assert_eq!(answer["cards"][1]["cardId"], json!(made.card_ids[1]));
    assert_eq!(
        answer["cards"][1]["title"],
        "[devflow] The search box lists matches"
    );
    assert_eq!(titles(&store, &project).len(), 3);
    assert_eq!(
        store.comments(&epic).expect("comments")[0].author,
        crate::cards::YOU
    );
}

#[test]
fn the_brief_and_the_plan_wait_for_the_persons_click_on_devflow() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    std::fs::create_dir_all(dir.path().join(".devpit/devflow")).expect("folder");
    std::fs::write(
        dir.path().join(".devpit/devflow/devflow.yaml"),
        "version: 1\nenabled: true\n",
    )
    .expect("yaml");
    for method in ["devflow_brief", "devflow_plan", "devflow_evidence"] {
        let refused = crate::devflow::respond_from(
            devpit_core::locale::Locale::En,
            None,
            method,
            &project,
            dir.path(),
            None,
            &json!({ "title": "x", "card": "card_1", "epic": "card_1" }),
        );
        assert!(refused.expect_err(method).contains("not on"), "{method}");
    }
    assert!(store.cards(&project.id).expect("cards").is_empty());
}

/// The ecopar plans waited since the person said yes to the orchestrator,
/// which had no way to pass it on.
#[test]
fn the_orchestrator_passes_the_persons_yes_on_and_the_epic_quotes_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let epic = epic(&store, &project);
    let preview = json!({ "epic": epic, "cards": cards() });
    respond(Locale::En, None, "devflow_plan", &project, &preview).expect("a preview");

    let bare = json!({ "epic": epic, "apply": true });
    assert!(relayed_refusal("devflow_plan", &bare).is_some());
    assert!(relayed_refusal("devflow_plan", &preview).is_none());
    let unquoted = json!({ "epic": epic, "apply": true, "approval": { "by": "orchestrator" } });
    assert!(respond(Locale::En, None, "devflow_plan", &project, &unquoted).is_err());
    assert!(relayed_refusal("devflow_plan", &unquoted).is_some());

    let yes = json!({
        "epic": epic, "apply": true,
        "approval": { "by": "orchestrator", "quote": "pode criar os cards" },
    });
    assert!(relayed_refusal("devflow_plan", &yes).is_none());
    let said = respond(Locale::En, None, "devflow_plan", &project, &yes).expect("created");
    assert_eq!(said["applied"], true);
    assert_eq!(titles(&store, &project).len(), 4);
    let record: Value = serde_json::from_str(
        &store
            .plan(&epic)
            .expect("plan")
            .expect("row")
            .created
            .expect("made"),
    )
    .expect("json");
    assert_eq!(record["by"], "orchestrator");
    let comments = store.comments(&epic).expect("comments");
    assert!(
        comments
            .iter()
            .any(|one| one.body.contains("«pode criar os cards»")),
        "{:?}",
        comments.iter().map(|one| &one.body).collect::<Vec<_>>()
    );
}

#[test]
fn cards_that_differ_from_the_preview_are_a_new_preview_even_with_the_persons_words() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let epic = epic(&store, &project);
    respond(
        Locale::En,
        None,
        "devflow_plan",
        &project,
        &json!({ "epic": epic, "cards": cards() }),
    )
    .expect("a preview");
    let mut other = cards();
    other[2]["title"] = json!("Matches are bold");
    let said = respond(
        Locale::En,
        None,
        "devflow_plan",
        &project,
        &json!({ "epic": epic, "cards": other, "apply": true,
                 "approval": { "quote": "sim" } }),
    )
    .expect("a new preview");
    assert_eq!(said["applied"], false);
    assert_eq!(titles(&store, &project).len(), 1);
}

#[test]
fn a_plan_waits_on_a_card_of_another_epic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let lane = waiting_lane(&store, &project.id).expect("a lane");
    let other = store
        .create_card(&project.id, &lane, "[devflow] The index exists", "")
        .expect("another epic's card");
    let number = store.card_numbers(&project.id).expect("numbers")[&other];
    let epic = epic(&store, &project);
    let plan = |depends_on: &str, apply: bool| {
        respond(
            Locale::En,
            None,
            "devflow_plan",
            &project,
            &json!({ "epic": epic, "apply": apply, "cards": [
                { "key": "s1", "title": "The search box lists them", "depends_on": [depends_on] },
            ] }),
        )
    };

    let unknown = plan("#999", false).expect_err("an unknown card");
    assert!(unknown.contains("`#999` is no card"), "{unknown}");
    assert!(waiting(&store, &epic).is_none(), "an unknown card was kept");

    let by_number = format!("#{number}");
    plan(&by_number, false).expect("a preview");
    let applied = plan(&by_number, true).expect("applied");
    let card = applied["cards"][0]["cardId"].as_str().expect("an id");
    assert_eq!(
        store.card_edges(&project.id).expect("edges").blockers,
        [(card.to_owned(), other.clone())]
    );

    let board = crate::board::board_still_now(project.id.clone()).expect("the board");
    let shown = board.cards.iter().find(|one| one.id == card).expect("card");
    assert!(
        shown
            .blocked_by
            .iter()
            .any(|by| by.id == other && by.holds()),
        "{:?}",
        shown.blocked_by
    );
}

/// With no epic the plan runs on demand, before the person's click: the
/// cards are checked as an epic's are, and nothing is created or kept. With
/// no devflow.yaml at all, Devflow is off and so is the plan.
#[test]
fn a_plan_without_an_epic_is_a_preview_alone_and_needs_no_click() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let ask = |params: Value| {
        crate::devflow::respond_from(
            devpit_core::locale::Locale::En,
            None,
            "devflow_plan",
            &project,
            dir.path(),
            None,
            &params,
        )
    };
    let off = ask(json!({ "cards": cards() })).expect_err("off");
    assert!(off.contains("off in this project"), "{off}");
    std::fs::create_dir_all(dir.path().join(".devpit/devflow")).expect("folder");
    std::fs::write(
        dir.path().join(".devpit/devflow/devflow.yaml"),
        "version: 1\nenabled: true\n",
    )
    .expect("yaml");

    let said = ask(json!({ "cards": cards() })).expect("a preview");
    assert_eq!(said["applied"], false);
    assert_eq!(said["epic"], Value::Null);
    let keys: Vec<&str> = said["preview"]
        .as_array()
        .expect("preview")
        .iter()
        .filter_map(|card| card["key"].as_str())
        .collect();
    assert_eq!(keys, ["s1", "s2", "s3"]);
    assert_eq!(said["preview"][1]["blockedBy"], json!(["s1"]));

    let twice = json!([{ "key": "s1", "title": "a" }, { "key": "s1", "title": "b" }]);
    assert!(ask(json!({ "cards": twice })).is_err());
    let applied = ask(json!({ "cards": cards(), "apply": true })).expect_err("no epic");
    assert!(applied.contains("epic"), "{applied}");
    assert!(store.cards(&project.id).expect("cards").is_empty());
}

#[test]
fn the_persons_edit_keeps_a_cards_abuse_cases_and_its_trust_boundary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let epic = epic(&store, &project);
    let guarded = json!([{
        "key": "s1", "title": "Files are read under the root",
        "criteria": ["a file under the root is read"],
        "files": ["apps/desktop/src/files.rs"],
        "abuse": ["a path outside the root is refused"], "boundary": true
    }]);
    respond(
        Locale::En,
        None,
        "devflow_plan",
        &project,
        &json!({ "epic": epic, "cards": guarded }),
    )
    .expect("a preview");

    let mut edited = waiting(&store, &epic).expect("waiting").cards;
    assert_eq!(edited[0].abuse, ["a path outside the root is refused"]);
    assert!(edited[0].boundary);
    edited[0].title = "Files are read under the project's root".into();
    let kept = kept_on_board(&store, &project.id, &epic, edited).expect("kept");
    assert_eq!(kept.cards[0].abuse, ["a path outside the root is refused"]);
    assert!(kept.cards[0].boundary);

    let made = created_on_board(&store, &project.id, &epic, None).expect("created");
    let body = store
        .card(&made.card_ids[0])
        .expect("read")
        .expect("card")
        .body;
    assert!(
        body.contains("Abuse case: a path outside the root is refused"),
        "{body}"
    );
}

/// Sabotage: give `previewed` the English dialect whatever the session's
/// language is, and the pt-BR session reads the engine's sentence in English.
#[test]
fn a_plan_refusal_is_said_in_the_calling_sessions_language() {
    let refusal = |locale| {
        previewed(
            locale,
            &json!({ "cards": [
                { "key": "s1", "title": "The search box lists them", "depends_on": ["ghost"] },
            ] }),
        )
        .expect_err("a card of no plan and no board")
    };
    let en = refusal(Locale::En);
    let pt = refusal(Locale::PtBr);
    assert!(en.contains("is not another card of this plan"), "{en}");
    assert!(pt.contains("não é outro card deste plano"), "{pt}");
    assert!(!pt.contains("is not another card"), "{pt}");
}
