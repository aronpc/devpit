use super::*;

fn comment(id: &str, body: &str) -> Value {
    json!({ "id": id, "author": "agent", "body": body, "createdAt": 1.0, "editedAt": null })
}

/// A card as the app answers it, with `comments` comments of `long` characters.
fn detail(comments: usize, long: usize) -> Value {
    let mut all: Vec<Value> = (0..comments)
        .map(|at| comment(&format!("cmt_{at}"), &"x".repeat(long)))
        .collect();
    all.insert(
        1,
        comment(
            "cmt_handoff",
            "**Handoff** — where this card's work stands\n\n**Done:**\nthe relay",
        ),
    );
    json!({
        "card": { "id": "card_1", "title": "A card", "body": "Do it.", "comments": all.len(), "position": 3, "dueAt": null, "cut": false,
                  "criteria": [{ "done": false, "text": "it works" }] },
        "comments": all,
        "children": [],
        "runs": [],
    })
}

fn lean(tool: &str, arguments: Value, answer: Value) -> Value {
    serde_json::from_str(&said(tool, &arguments, answer)).expect("json")
}

#[test]
fn full_gives_the_answer_whole_as_it_was() {
    let answer = detail(4, 50);
    let text = said("devpit_card", &json!({ "full": true }), answer.clone());
    assert_eq!(text, serde_json::to_string_pretty(&answer).expect("json"));
}

#[test]
fn the_project_agents_brief_and_answer_come_back_whole() {
    let long = format!("# Brief\n\n{}", "Read the card. ".repeat(600));
    for (tool, field) in [
        ("devpit_project_agent_brief", "brief"),
        ("devpit_project_agent_ask", "answer"),
    ] {
        let answer = json!({ "project": "devpit", "cardId": "card_1", field: long, "woke": false });
        assert_eq!(
            lean(tool, json!({}), answer.clone()),
            answer,
            "{tool} was cut"
        );
    }
}

#[test]
fn a_comment_answers_its_card_and_newest_comment_not_the_card() {
    let text = said("devpit_comment", &json!({}), detail(10, 4000));
    let answer: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(
        answer,
        json!({ "card": "card_1", "comments": 11, "newest": "cmt_9" })
    );
}

#[test]
fn an_edit_answers_the_comment_it_edited() {
    let answer = lean(
        "devpit_comment_edit",
        json!({ "commentId": "cmt_0" }),
        detail(3, 10),
    );
    assert_eq!(answer["edited"], "cmt_0");
}

#[test]
fn a_handoff_answers_like_a_comment() {
    let answer = lean("devpit_handoff", json!({}), detail(2, 10));
    assert_eq!(answer["card"], "card_1");
    assert!(answer.get("comments").is_some() && answer.get("newest").is_some());
}

#[test]
fn a_card_shows_its_newest_comments_short_and_its_latest_handoff() {
    let answer = lean("devpit_card", json!({}), detail(6, 1000));
    let comments = &answer["comments"];
    assert_eq!(comments["total"], 7);
    let newest = comments["newest"].as_array().expect("newest");
    assert_eq!(newest.len(), NEWEST_COMMENTS);
    assert_eq!(newest[0]["id"], "cmt_5");
    assert!(newest[0]["head"].as_str().expect("head").chars().count() <= COMMENT_HEAD + 1);
    assert_eq!(comments["handoff"]["id"], "cmt_handoff");
    assert!(comments["handoff"]["text"]
        .as_str()
        .expect("text")
        .contains("the relay"));
    assert!(answer["card"].get("position").is_none());
    // What says nothing is left out; what says no stays.
    assert!(answer["card"].get("dueAt").is_none());
    assert_eq!(answer["card"]["criteria"][0]["done"], false);
}

#[test]
fn a_card_asked_for_its_comments_has_them_whole() {
    let answer = lean("devpit_card", json!({ "comments": true }), detail(2, 30));
    let comments = answer["comments"].as_array().expect("every comment");
    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0]["body"], "x".repeat(30));
}

#[test]
fn a_board_card_is_one_short_row() {
    let board = json!([{ "id": "col_1", "name": "todo", "runsAStep": false, "cards": [{
        "id": "card_1", "number": 7, "title": "A card", "comments": 4, "lastRun": null, "parentId": null,
        "blockedBy": [{ "id": "card_0", "title": "the one before" }], "progress": null,
        "criteria": { "done": 1, "total": 3 },
    }] }]);
    let answer = lean("devpit_board", json!({}), board);
    assert_eq!(
        answer,
        json!({ "columns": [{ "id": "col_1", "name": "todo", "cards": [{
            "id": "card_1", "number": 7, "title": "A card", "blockedBy": ["card_0"], "criteria": "1/3",
        }] }] })
    );
}

#[test]
fn evidence_lists_only_the_criteria_not_green() {
    let tests = json!([{ "test": "a", "result": "passed" }, { "test": "b", "result": "passed" }]);
    let answer = lean(
        "devpit_devflow_evidence",
        json!({}),
        json!({
            "card": "card_1", "green": false, "criteria_green": "1/2", "run": "/runs/1", "evidence": "/runs/1/evidence.json",
            "attached": { "files": 2, "under": "/runs" },
            "checks": [{ "name": "unit", "result": "failed", "exit": 1, "log": "/runs/1/unit.log", "reports": ["/runs/1/r.xml"], "tests": { "passed": 2, "failed": 1 } }],
            "criteria": [
                { "criterion": "green one", "green": true, "tests": tests },
                { "criterion": "red one", "green": false, "tests": [{ "test": "c", "result": "failed" }] },
            ],
        }),
    );
    assert_eq!(
        answer["checks"][0],
        json!({ "name": "unit", "result": "failed", "exit": 1, "tests": { "passed": 2, "failed": 1 } })
    );
    let criteria = answer["criteria"].as_array().expect("criteria");
    assert_eq!(criteria.len(), 1);
    assert_eq!(criteria[0]["criterion"], "red one");
    assert_eq!(criteria[0]["tests"][0]["test"], "c");
    assert_eq!(answer["criteria_green"], "1/2");
    assert_eq!(answer["evidence"], "/runs/1/evidence.json");
    assert!(answer.get("attached").is_none());
}

#[test]
fn a_review_lists_the_findings_that_can_hold_a_card_and_counts_the_rest_and_keeps_its_packet() {
    let answer = lean(
        "devpit_devflow_review",
        json!({}),
        json!({
            "verdict": "approve", "run": "r1", "next": "completion",
            "findings": ["medium `a.rs:2` a hole (1 of 1)", "low `a.rs:1` a nit (1 of 1)", "info `a.rs:3` a note (1 of 1)", "low `a.rs:4` another (1 of 1)"],
            "findings_file": "/runs/r1/findings.json", "packet": "/runs/r1/packet.md", "base": "abc", "head": "def",
            "consolidation": { "used": "no" }, "attached": { "files": 1 },
            "reviewers": [{ "persona": "devflow-all", "agent": "claude", "result": "answered", "findings": 1, "cost_usd": 0.5, "duration_ms": 10, "by": "devpit" }],
        }),
    );
    assert_eq!(
        answer,
        json!({
            "verdict": "approve", "run": "r1", "next": "completion",
            "findings": ["medium `a.rs:2` a hole (1 of 1)"], "findings_counted": { "low": 2, "info": 1 },
            "findings_file": "/runs/r1/findings.json", "packet": "/runs/r1/packet.md",
            "reviewers": [{ "persona": "devflow-all", "agent": "claude", "result": "answered", "findings": 1 }],
        })
    );
}

#[test]
fn status_leaves_the_yaml_it_also_answers_parsed() {
    let answer = lean(
        "devpit_devflow_status",
        json!({}),
        json!({ "state": "active", "yaml": "a: 1", "config": { "a": 1 } }),
    );
    assert_eq!(answer, json!({ "state": "active", "config": { "a": 1 } }));
}

/// The ceiling holds whatever the tool and however big the app's answer.
#[test]
fn no_answer_passes_the_budget_and_a_cut_one_says_so() {
    let huge = json!({
        "card": { "id": "card_1", "body": "b".repeat(20_000) },
        "children": (0..200).map(|at| json!({ "id": format!("card_{at}"), "title": "t".repeat(80) })).collect::<Vec<_>>(),
        "comments": [],
    });
    for tool in ["devpit_card", "devpit_devflow_plan", "devpit_context"] {
        let text = said(tool, &json!({}), huge.clone());
        assert!(text.len() <= BUDGET, "{tool}: {}", text.len());
        assert!(text.contains("full: true"), "{tool}");
    }
    let wide = json!({ "card": { "id": "card_1" }, "plan": (0..200).map(|at| (format!("field_{at}"), json!({ "key": format!("s{at}"), "value": at }))).collect::<serde_json::Map<_, _>>(), "comments": [] });
    let text = said("devpit_card", &json!({}), wide);
    assert!(text.len() <= BUDGET, "wide: {}", text.len());
    assert!(text.contains("card_1"));
    let board = json!([{ "id": "col", "name": "done", "cards": (0..500).map(|at| json!({ "id": format!("card_{at}"), "number": at, "title": "t".repeat(60) })).collect::<Vec<_>>() }]);
    let text = said("devpit_board", &json!({}), board);
    assert!(text.len() <= BUDGET, "board: {}", text.len());
    assert!(text.contains("full: true"), "a cut board says so");
    let cut: Value = serde_json::from_str(&text).expect("json");
    let cards = cut["columns"][0]["cards"].as_array().expect("cards");
    let shown = cards.len() - 1;
    assert_eq!(cards[shown], json!(format!("…and {} more", 500 - shown)));
}

#[test]
fn an_answer_within_the_budget_is_not_cut() {
    let answer = lean(
        "devpit_context",
        json!({}),
        json!({ "project": "p", "note": "n".repeat(2000) }),
    );
    assert_eq!(answer["note"].as_str().map(str::len), Some(2000));
    assert!(answer.get("cut").is_none());
}

fn skill(digest: &str) -> Value {
    json!({ "name": "devflow-execution", "text": "# Execution", "skill": { "resources": [{ "digest": digest }] } })
}

#[test]
fn a_skill_is_given_once_a_session_until_it_changes() {
    let given = Given::default();
    assert!(!given.again(&skill("sha256:1")));
    assert!(given.again(&skill("sha256:1")));
    assert!(!given.again(&skill("sha256:2")));
    let mut part = skill("sha256:2");
    part["part"] = json!("verify");
    assert!(!given.again(&part), "a part is its own skill");
}

#[test]
fn the_shaped_tools_offer_full_and_the_card_its_comments() {
    let mut card = json!({ "type": "object", "properties": { "cardId": { "type": "string" } } });
    offered("devpit_card", &mut card);
    assert_eq!(card["properties"][FULL]["type"], "boolean");
    assert_eq!(card["properties"][COMMENTS]["type"], "boolean");
    let mut context = json!({ "type": "object", "properties": {} });
    offered("devpit_context", &mut context);
    assert_eq!(context["properties"], json!({}));
}

#[test]
fn the_board_is_asked_a_page_unless_the_call_wants_it_whole_or_says_which() {
    assert_eq!(asked("devpit_board", json!({}))["limit"], BOARD_PAGE);
    assert!(asked("devpit_board", json!({ "full": true }))
        .get("limit")
        .is_none());
    assert_eq!(
        asked("devpit_board", json!({ "column": "doing" })),
        json!({ "column": "doing" })
    );
    assert_eq!(
        asked("devpit_card", json!({ "cardId": "c" })),
        json!({ "cardId": "c" })
    );
}

#[test]
fn a_paged_board_keeps_each_columns_total_and_cursor() {
    let page = json!({ "columns": [{ "id": "col_1", "name": "todo", "runsAStep": false, "total": 30, "nextCursor": "col_1:19:card_19",
        "cards": [{ "id": "card_1", "number": 1, "title": "A card", "comments": 2, "blockedBy": [] }] }] });
    let answer = lean("devpit_board", json!({}), page);
    assert_eq!(
        answer,
        json!({ "columns": [{ "id": "col_1", "name": "todo", "total": 30, "nextCursor": "col_1:19:card_19",
            "cards": [{ "id": "card_1", "number": 1, "title": "A card" }] }] })
    );
}

/// Twenty wide cards in each of three lanes do not fit: each lane keeps
/// fewer, and its cursor names the last kept, the first left out and the
/// reading, when the app gave one (lane `c` comes from an app without them).
#[test]
fn a_page_too_wide_keeps_fewer_cards_and_its_cursor_names_the_last_kept_and_the_first_left_out() {
    let lane = |id: &str| {
        let mut lane = json!({ "id": id, "name": id, "total": 40, "nextCursor": format!("{id}:19:{id}_card_19:20:{id}_card_20"),
        "cards": (0..20).map(|at| json!({ "id": format!("{id}_card_{at}"), "position": at, "title": "t".repeat(60) })).collect::<Vec<_>>() });
        if id != "c" {
            lane["reading"] = json!(format!("R{id}"));
        }
        lane
    };
    let text = said(
        "devpit_board",
        &json!({}),
        json!({ "columns": [lane("a"), lane("b"), lane("c")] }),
    );
    assert!(text.len() <= BUDGET, "{}", text.len());
    let answer: Value = serde_json::from_str(&text).expect("json");
    for column in answer["columns"].as_array().expect("lanes") {
        let cards = column["cards"].as_array().expect("cards");
        assert!(
            cards.len() < 20 && cards.iter().all(Value::is_object),
            "kept fewer, no marker"
        );
        let id = column["id"].as_str().expect("id");
        let (last, next) = (cards.len() - 1, cards.len());
        let reading = if id == "c" {
            String::new()
        } else {
            format!(":R{id}")
        };
        assert_eq!(
            column["nextCursor"],
            format!("{id}:{last}:{id}_card_{last}:{next}:{id}_card_{next}{reading}")
        );
        assert!(column.get("reading").is_none(), "the reading is not shown");
        assert!(cards[0].get("position").is_none(), "the place is not shown");
    }
}
