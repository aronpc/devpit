use serde_json::json;

use super::*;

fn put(base: &Path, relative: &str, text: &str) {
    let path = base.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    std::fs::write(path, text).expect("write");
}

#[test]
fn a_card_shows_its_newest_gate_reviewers_and_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let card = "card_evidence";
    let older = format!("devflow/{card}/20261011T143005Z");
    let newer = format!("devflow/{card}/20261011T150000Z");
    put(
        dir.path(),
        &format!("{older}/gate.json"),
        &json!({ "passed": false, "at": "20261011T143005Z", "criteria": 2 }).to_string(),
    );
    put(
        dir.path(),
        &format!("{older}/review.json"),
        &json!({ "ran": [
            { "agent": "claude", "persona": "devflow-security", "file": "findings-claude.json",
              "findings": 2, "duration_ms": 61000, "cost_usd": 0.42 },
            { "agent": "codex", "persona": "devflow-spec", "file": null, "findings": 0,
              "reason": "codex is not signed in" },
        ] })
        .to_string(),
    );
    put(
        dir.path(),
        &format!("{older}/findings.json"),
        &json!({ "verdict": "request_changes" }).to_string(),
    );
    put(
        dir.path(),
        &format!("{newer}/gate.json"),
        &json!({ "passed": true, "at": "20261011T150000Z", "run": "20261011T150000Z",
                 "criteria_with_evidence": 2, "criteria": 2, "missing": [] })
        .to_string(),
    );
    put(dir.path(), &format!("{newer}/evidence.json"), "{}");
    put(
        dir.path(),
        &format!("{newer}/screenshots/e2e-home.png"),
        "png",
    );

    let said = evidence_in(dir.path(), card);
    let gate = said.gate.expect("a gate");
    assert!(gate.passed, "the newest gate is the one shown");
    assert_eq!((gate.with_evidence, gate.criteria), (2, 2));
    assert_eq!(said.verdict.as_deref(), Some("request_changes"));
    let statuses: Vec<(&str, &str)> = said
        .reviewers
        .iter()
        .map(|one| (one.agent.as_str(), one.status.as_str()))
        .collect();
    assert_eq!(statuses, [("claude", "answered"), ("codex", "skipped")]);
    assert_eq!(said.reviewers[0].cost_usd, Some(0.42));
    assert_eq!(
        said.artifacts,
        [
            format!("{newer}/evidence.json"),
            format!("{newer}/gate.json"),
            format!("{newer}/screenshots/e2e-home.png"),
        ]
    );
}

#[test]
fn a_card_with_no_runs_or_a_strange_id_shows_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let none = evidence_in(dir.path(), "card_none");
    assert!(none.gate.is_none() && none.reviewers.is_empty() && none.artifacts.is_empty());
    put(
        dir.path(),
        "devflow/../escape/20261011T150000Z/gate.json",
        "{}",
    );
    assert!(evidence_in(dir.path(), "../escape").gate.is_none());
}

#[test]
fn a_card_shows_the_exploits_its_newest_review_confirmed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let card = "card_attacked";
    let run = format!("devflow/{card}/20261118T120000Z");
    put(dir.path(), &format!("{run}/review.json"), r#"{"ran": []}"#);
    put(
        dir.path(),
        &format!("{run}/findings.json"),
        &json!({ "verdict": "request_changes", "exploits": [
            { "test": "tests/attack.rs::a_path_outside_the_root_is_refused",
              "file": "src/files.rs", "line": 42, "title": "Path traversal" },
            { "test": "" },
            { "file": "no test" },
        ] })
        .to_string(),
    );

    let said = evidence_in(dir.path(), card);
    assert_eq!(
        said.exploits,
        [DevflowExploit {
            test: "tests/attack.rs::a_path_outside_the_root_is_refused".to_owned(),
            file: Some("src/files.rs".to_owned()),
            line: Some(42),
            title: Some("Path traversal".to_owned()),
        }]
    );
}
