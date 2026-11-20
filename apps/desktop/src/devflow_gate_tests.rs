use devflow_engine::Ask;
use serde_json::json;

use super::*;

/// A JUnit report naming the card's one test, written where the check is told to.
const UNIT: &str = r#"printf '<testsuite><testcase classname="cli" name="rename changes the title"/></testsuite>' > "$DEVPIT_REPORT_DIR/report.xml""#;

fn project_at(id: &str, root: &Path) -> Project {
    serde_json::from_value(json!({
        "id": id, "name": id, "rootPath": root.display().to_string(), "group": null,
        "accent": "#000000", "worktrees": [], "unreadable": null, "orchestrator": null,
        "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .expect("a project")
}

/// A project with Devflow switched on — one `unit` check, the review stage
/// off, `extra` added to devflow.yaml — and a card of its plan whose checkout
/// is the project's folder.
fn devflow_on(dir: &Path, extra: &str) -> (Store, Project, String) {
    let store = crate::projects::store().expect("store");
    let id = store.add_project(dir, None).expect("project");
    store.ensure_board(&id).expect("board");
    let yaml = format!(
        "version: 1\nenabled: true\nstages:\n  review:\n    enabled: false\n  execution:\n    checks:\n      - name: unit\n        run: '{}'\n        report: junit\n{extra}",
        UNIT.replace('\'', "''")
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
    let here = dir.display().to_string();
    store
        .set_card_front(&card, Some(&here), Some("HEAD"))
        .expect("its checkout");
    (store, project_at(&id, dir), card)
}

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

fn evidence(project: &Project, root: &Path, card: &str) -> Value {
    ask(
        project,
        root,
        "devflow_evidence",
        json!({ "card": card, "criteria": [{ "criterion": "rename changes the title", "tests": ["rename changes the title"] }] }),
    )
}

fn last_comment(store: &Store, card: &str) -> String {
    store
        .comments(card)
        .expect("comments")
        .last()
        .map(|comment| comment.body.clone())
        .unwrap_or_default()
}

/// The Stop payload Claude Code sends through the plugin's hook, as the
/// gate's tool is called with it.
fn claude_stop(continued: &str) -> Value {
    json!({ "event": "Stop", "session": "claude-session", "stop_hook_active": continued })
}

#[test]
fn a_card_whose_checks_never_ran_fails_the_gate_and_its_comment_says_what_is_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, card) = devflow_on(&root, "");
    let said = ask(&project, &root, "devflow_gate", json!({ "card": card }));
    assert_eq!(said["result"], "failed", "{said}");
    assert_eq!(said["criteria"], "0 of 1");
    assert!(said["missing"][0]
        .as_str()
        .is_some_and(|missing| missing.contains("devpit_devflow_evidence")));
    let gate_json = root.join(said["gate"].as_str().expect("where gate.json is"));
    let home = crate::devflow_evidence::runs_of(&project.id, &root)
        .expect("runs")
        .home;
    assert_eq!(gate_json, home.join(&card).join("gate.json"));
    let comment = last_comment(&store, &card);
    assert!(
        comment.starts_with("[devflow] gate: 0 of 1 criteria with evidence"),
        "{comment}"
    );
}

#[test]
fn with_the_evidence_the_gate_passes_keeps_gate_json_in_the_run_and_attaches_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, card) = devflow_on(&root, "");
    let ran = evidence(&project, &root, &card);
    assert_eq!(ran["green"], true, "{ran}");
    assert!(ran.get("criteria_unread").is_none(), "{ran}");

    let said = ask(&project, &root, "devflow_gate", json!({ "card": card }));
    assert_eq!(said["result"], "passed", "{said}");
    assert_eq!(said["criteria"], "1 of 1");
    let gate_json = root.join(said["gate"].as_str().expect("gate.json"));
    assert!(gate_json.starts_with(root.join(ran["run"].as_str().expect("the run"))));
    let kept: Value =
        serde_json::from_str(&std::fs::read_to_string(&gate_json).expect("read")).expect("json");
    assert_eq!(kept["passed"], true);
    let stamp = ran["run"]
        .as_str()
        .and_then(|run| run.rsplit('/').next())
        .expect("stamp");
    let attached = crate::projects::project_home(&project.id)
        .expect("home")
        .artifacts()
        .join(format!("devflow/{card}/{stamp}/gate.json"));
    assert!(attached.is_file(), "gate.json is attached with its run");
    let comment = last_comment(&store, &card);
    assert!(
        comment.starts_with("[devflow] gate: 1 of 1 criteria with evidence"),
        "{comment}"
    );
    assert!(comment.ends_with("Passed."), "{comment}");
}

/// A board that writes its criteria under `## Critérios de aceite`: the
/// evidence says none were read, and the gate names the heading to rename.
#[test]
fn checkboxes_under_a_localized_heading_are_flagged_by_the_evidence_and_the_gate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, card) = devflow_on(&root, "");
    let row = store.card(&card).expect("read").expect("the card");
    let localized =
        "## Critérios de aceite\n- [ ] rename changes the title\n- [ ] the old title is gone\n";
    store
        .update_card(&card, &row.title, localized)
        .expect("localized");

    let ran = evidence(&project, &root, &card);
    assert_eq!(
        ran["criteria_unread"]["heading"], "Critérios de aceite",
        "{ran}"
    );
    assert_eq!(ran["criteria_unread"]["count"], 2, "{ran}");
    assert!(
        ran["criteria_unread"]["said"]
            .as_str()
            .is_some_and(|said| said.contains("were not read as criteria")),
        "{ran}"
    );

    let said = ask(&project, &root, "devflow_gate", json!({ "card": card }));
    assert_eq!(said["result"], "failed", "{said}");
    assert!(
        said["missing"]
            .as_array()
            .is_some_and(|missing| missing
                .iter()
                .any(|one| one.as_str().is_some_and(|one| one
                    .contains("rename `## Critérios de aceite` to `## Acceptance criteria`")))),
        "{said}"
    );
}

/// The acceptance in Claude Code: saying "done" without the checks run is
/// sent back with the reason; with the evidence, the turn ends.
#[test]
fn a_claude_stop_without_evidence_is_sent_back_and_with_it_the_turn_ends() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = devflow_on(&root, "");
    let held = ask(&project, &root, "devflow_gate", claude_stop("false"));
    assert_eq!(held["decision"], "block", "{held}");
    let reason = held["reason"].as_str().expect("a reason");
    assert!(
        reason.starts_with("[devflow] gate: 0 of 1 criteria with evidence"),
        "{reason}"
    );
    assert!(reason.contains("devpit_devflow_evidence"));

    evidence(&project, &root, &card);
    let let_go = ask(&project, &root, "devflow_gate", claude_stop("true"));
    assert_eq!(let_go, json!({}), "the gate passes once the evidence is in");
}

/// The same handler, with the payload Codex's hook sends: a boolean
/// `stop_hook_active`, its block a continuation prompt.
#[test]
fn a_codex_stop_is_held_by_the_same_gate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = devflow_on(&root, "");
    let codex = |continued: bool| json!({ "event": "Stop", "session": "codex-thread", "stop_hook_active": continued });
    let held = ask(&project, &root, "devflow_gate", codex(false));
    assert_eq!(held["decision"], "block", "{held}");
    evidence(&project, &root, &card);
    assert_eq!(ask(&project, &root, "devflow_gate", codex(true)), json!({}));
}

#[test]
fn a_turn_sent_back_that_changes_nothing_ends_with_a_warning_on_the_card() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, card) = devflow_on(&root, "");
    let first =
        json!({ "event": "Stop", "session": "stuck", "stop_hook_active": false, "cap": "8" });
    assert_eq!(
        ask(&project, &root, "devflow_gate", first)["decision"],
        "block"
    );
    let again =
        json!({ "event": "Stop", "session": "stuck", "stop_hook_active": true, "cap": "8" });
    let warned = ask(&project, &root, "devflow_gate", again);
    assert!(warned.get("decision").is_none(), "{warned}");
    assert!(warned["systemMessage"]
        .as_str()
        .is_some_and(|message| message.contains("let this turn end without its evidence")));
    let comment = last_comment(&store, &card);
    assert!(comment.starts_with("[devflow] gate: 0 of 1"), "{comment}");
    assert!(
        comment.contains("after sending it back 1 time"),
        "{comment}"
    );
}

#[test]
fn switching_devflow_off_lets_every_stop_go_and_the_session_hears_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, _card) = devflow_on(&root, "");
    let context = ask(
        &project,
        &root,
        "devflow_session",
        json!({ "agent": "claude" }),
    );
    assert!(context["context"]
        .as_str()
        .is_some_and(|said| said.contains("[devflow] Rename a card")));

    crate::devflow_asks::remember(&store, &project.id, Ask::Enable, false).expect("off");
    assert_eq!(
        ask(&project, &root, "devflow_gate", claude_stop("false")),
        json!({})
    );
    assert_eq!(
        ask(&project, &root, "devflow_session", json!({}))["context"],
        Value::Null
    );
}

#[test]
fn a_subagent_that_changed_no_code_is_let_go() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, _card) = devflow_on(&root, "");
    let transcript = root.join("agent.jsonl");
    std::fs::write(&transcript, "{}\n").expect("transcript");
    let said = ask(
        &project,
        &root,
        "devflow_gate",
        json!({ "event": "SubagentStop", "session": "s", "stop_hook_active": "false",
                "agent": "explorer", "transcript": transcript.display().to_string() }),
    );
    assert_eq!(said, json!({}), "no git, so no change it could have made");
}

/// Committing is not changing the code; writing a file after the run is.
#[test]
fn a_file_written_after_the_run_sends_the_card_back_to_its_checks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "t@example.invalid"],
        vec!["config", "user.name", "T"],
    ] {
        devpit_pty::host_env::command("git")
            .args(&args)
            .current_dir(&root)
            .output()
            .expect("git");
    }
    let (_store, project, card) = devflow_on(&root, "");
    std::fs::write(root.join("cli.rs"), "fn main() {}\n").expect("code");
    let an_hour_ago = SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(root.join("cli.rs"))
        .and_then(|file| file.set_modified(an_hour_ago))
        .expect("written before the run");
    evidence(&project, &root, &card);
    assert_eq!(
        ask(&project, &root, "devflow_gate", json!({ "card": card }))["result"],
        "passed"
    );

    let later = SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(root.join("cli.rs"))
        .and_then(|file| file.set_modified(later))
        .expect("written after the run");
    let said = ask(&project, &root, "devflow_gate", json!({ "card": card }));
    assert_eq!(said["result"], "failed", "{said}");
    assert!(said["missing"][0]
        .as_str()
        .is_some_and(|missing| missing.contains("changed after run")));
}

#[test]
fn finishing_the_card_waits_for_a_gate_that_passed_on_the_newest_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = devflow_on(&root, "");
    let refused = may_finish(devpit_core::locale::Locale::En, &project, &card, &root)
        .expect_err("no evidence yet");
    assert!(refused.contains("the card stays where it is"), "{refused}");
    evidence(&project, &root, &card);
    let refused = may_finish(devpit_core::locale::Locale::En, &project, &card, &root)
        .expect_err("no gate yet");
    assert!(refused.contains("call devpit_devflow_gate"), "{refused}");
    ask(&project, &root, "devflow_gate", json!({ "card": card }));
    may_finish(devpit_core::locale::Locale::En, &project, &card, &root).expect("the gate passed");
}

/// Decisions set up in devflow.yaml but no key kept: the gate asks nothing
/// and the checks alone decide.
#[test]
fn without_a_decisions_key_the_checks_alone_decide() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = devflow_on(&root, "decisions:\n  use: true\n  mode: enforce\n");
    evidence(&project, &root, &card);
    let said = ask(&project, &root, "devflow_gate", json!({ "card": card }));
    assert_eq!(said["result"], "passed", "{said}");
    assert_eq!(said["decisions"]["outcome"], "skipped");
    assert_eq!(
        said["decisions"]["mode"], "shadow",
        "enforce waits for the person's click"
    );
}

/// However Decisions stands, in shadow the checks give the verdict and the
/// reply says where it stands, beside what the rubric said.
#[test]
fn the_gate_says_where_decisions_stands_and_the_checks_decide_in_each_standing() {
    use crate::devflow_jev::tests::{Standing, STANDINGS};
    for standing in STANDINGS {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let (_store, project, card) = devflow_on(&root, standing.yaml());
        standing.give(&project.id);
        evidence(&project, &root, &card);
        let said = ask(&project, &root, "devflow_gate", json!({ "card": card }));
        assert_eq!(said["result"], "passed", "{standing:?}: {said}");
        standing.said_in(&said);
        if !matches!(standing, Standing::NotUsed) {
            assert_eq!(said["decisions"]["rubric"], "devflow-done", "{said}");
        }
    }
}

#[test]
fn devflow_done_asks_done_once_per_criterion_and_keeps_verified_and_in_scope() {
    let criteria = vec!["one".to_owned(), "two".to_owned()];
    let rubric = done_per_criterion(&criteria);
    assert_eq!(rubric.name, "devflow-done");
    let questions = rubric.questions.as_object().expect("questions");
    let mut ids: Vec<&String> = questions.keys().collect();
    ids.sort();
    assert_eq!(ids, ["criterion1", "criterion2", "inScope", "verified"]);
    assert!(questions["criterion2"]["instructions"]
        .as_str()
        .is_some_and(|said| said.contains("\"two\"")));
    assert_eq!(questions["criterion1"]["type"], "noul");
    assert_eq!(rubric.thresholds["criterion1"].min, Some(0.85));
    let many: Vec<String> = (0..40).map(|at| at.to_string()).collect();
    assert_eq!(
        done_per_criterion(&many)
            .questions
            .as_object()
            .expect("questions")
            .len(),
        crate::agent_decide::MOST_QUESTIONS
    );
}

#[test]
fn a_sessions_context_names_its_card_and_the_gate_it_ends_at() {
    let card = devpit_core::store::CardRow {
        id: "card_1".to_owned(),
        column_id: "col".to_owned(),
        title: "[devflow] Rename a card".to_owned(),
        body: "## Acceptance criteria\n- [ ] rename changes the title\n".to_owned(),
        position: 0,
        worktree_path: None,
        base_ref: None,
        due_at: None,
        due_time: false,
    };
    let stages = ["execution", "completion"];
    let said = context(
        Some(&card),
        &["\"Parse\" (card_0)".to_owned()],
        &stages,
        "codex",
    );
    assert!(
        said.contains("card_1 \"[devflow] Rename a card\""),
        "{said}"
    );
    assert!(said.contains("- rename changes the title"));
    assert!(said.contains("Still blocked by \"Parse\" (card_0)"));
    assert!(said.contains("$devflow-<stage>"));
    assert!(said.contains("devpit_devflow_gate"));
    let brief = devpit_core::store::CardRow {
        title: "[devflow] Brief: Renaming".to_owned(),
        ..card
    };
    assert!(!context(Some(&brief), &[], &stages, "claude").contains("devpit_devflow_gate"));
    assert!(context(None, &[], &stages, "claude").contains("on no card yet"));
}

/// The ascbot review approved one commit and the gate passed with a fix
/// committed after it, which nobody reviewed.
#[test]
fn a_review_counts_for_the_commit_it_read_and_not_for_code_after_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let git = |args: &[&str]| -> String {
        let done = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .expect("git");
        assert!(done.status.success(), "git {args:?}: {done:?}");
        String::from_utf8_lossy(&done.stdout).trim().to_owned()
    };
    let commit = |file: &str, text: &str| -> String {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", file]);
        git(&["rev-parse", "HEAD"])
    };
    git(&["init", "-q", "-b", "main"]);
    let reviewed = commit("app.py", "print('hi')\n");
    let findings = json!({ "verdict": "approve", "head": reviewed });

    assert_eq!(
        unreviewed_since(root, &findings),
        None,
        "HEAD is what it read"
    );
    commit(".devpit/devflow/notes.md", "a note\n");
    assert_eq!(
        unreviewed_since(root, &findings),
        None,
        "only Devflow's files"
    );
    commit("app.py", "print('fixed')\n");
    assert_eq!(unreviewed_since(root, &findings), Some(reviewed));
    assert_eq!(
        unreviewed_since(root, &json!({ "verdict": "approve" })),
        None
    );
}

/// A card of `project` waiting on `card`.
fn waiting_on(store: &Store, project: &Project, card: &str, title: &str) -> String {
    let first = store.columns(&project.id).expect("columns")[0].id.clone();
    let next = store
        .create_card(&project.id, &first, title, "")
        .expect("card");
    store
        .conn()
        .execute(
            "INSERT INTO card_blocker (card_id, blocked_by, created_at) VALUES (?1, ?2, 0)",
            [next.as_str(), card],
        )
        .expect("waits");
    next
}

/// Whether `start_card` would let the card go, by the board as it stands.
fn startable(project: &Project, card: &str) -> bool {
    let board = crate::board::board_still_now(project.id.clone()).expect("board");
    crate::card_follows::unblocked(&board, card, true).is_ok()
}

#[test]
fn a_passed_gate_releases_the_cards_waiting_only_where_the_plan_says_unblock_on_gate() {
    for (extra, releases) in [("", false), ("  plan:\n    unblock_on: gate\n", true)] {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let (store, project, card) = devflow_on(&root, extra);
        let next = waiting_on(&store, &project, &card, "[devflow] Show the new title");
        assert!(!startable(&project, &next), "no gate yet: {extra:?}");
        evidence(&project, &root, &card);
        ask(&project, &root, "devflow_gate", json!({ "card": card }));
        assert_eq!(startable(&project, &next), releases, "{extra:?}");
    }
}

#[test]
fn a_card_merged_into_another_finishes_on_its_evidence_and_a_cut_one_on_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project, card) = devflow_on(&root, "");
    let plan = |params: Value| {
        crate::devflow_plan::respond(
            devpit_core::locale::Locale::En,
            None,
            "devflow_plan",
            &project,
            &params,
        )
        .expect("plan")
    };
    let first = store.columns(&project.id).expect("columns")[0].id.clone();
    let criteria = "## Acceptance criteria\n- [ ] the menu renames\n";
    let merged = store
        .create_card(
            &project.id,
            &first,
            "[devflow] Rename from the menu",
            criteria,
        )
        .expect("card");
    let next = waiting_on(&store, &project, &merged, "[devflow] Show the new title");
    plan(json!({ "merge": { "from": merged, "into": card }, "apply": true }));
    let refused = may_finish(devpit_core::locale::Locale::En, &project, &merged, &root)
        .expect_err("nothing passed yet");
    assert!(refused.contains("its work went into"), "{refused}");
    assert!(
        !startable(&project, &next),
        "the card it went into is not done"
    );

    evidence(&project, &root, &card);
    ask(&project, &root, "devflow_gate", json!({ "card": card }));
    may_finish(devpit_core::locale::Locale::En, &project, &merged, &root)
        .expect("finishes on the card it went into");
    let done = store
        .columns(&project.id)
        .expect("columns")
        .into_iter()
        .find(|column| {
            devpit_core::store::column_roles::role_of(column.role.as_deref(), &column.name)
                == Some("done")
        })
        .expect("a done lane");
    store.move_card(&card, &done.id, 0).expect("done");
    assert!(startable(&project, &next));

    let dropped = store
        .create_card(&project.id, &first, "[devflow] Rename by voice", criteria)
        .expect("card");
    let after = waiting_on(&store, &project, &dropped, "[devflow] Say the new title");
    assert!(may_finish(devpit_core::locale::Locale::En, &project, &dropped, &root).is_err());
    plan(json!({ "cut": dropped, "apply": true }));
    may_finish(devpit_core::locale::Locale::En, &project, &dropped, &root)
        .expect("closes without evidence");
    assert!(startable(&project, &after));
}

/// `git` in `root`, with no global config and a fixed author.
fn git_in(root: &Path, args: &[&str]) {
    let done = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("git");
    assert!(done.status.success(), "git {args:?}: {done:?}");
}

/// [`devflow_on`] in a git checkout with `cli.rs` and devflow.yaml committed.
fn committed(root: &Path) -> (Store, Project, String) {
    git_in(root, &["init", "-q", "-b", "main"]);
    let made = devflow_on(root, "");
    std::fs::write(root.join("cli.rs"), "fn main() {}\n").expect("code");
    git_in(root, &["add", "-A"]);
    git_in(root, &["commit", "-q", "-m", "first"]);
    made
}

fn with_proof(project: &Project, root: &Path, card: &str) -> Value {
    ask(
        project,
        root,
        "devflow_evidence",
        json!({ "card": card, "criteria": [{ "criterion": "rename changes the title", "proof": { "kind": "screenshot", "path": "shot.png" } }] }),
    )
}

#[test]
fn a_proof_written_after_the_run_counts_when_head_has_not_changed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = committed(&root);
    let ran = with_proof(&project, &root, &card);
    assert_eq!(
        ran["criteria_green"], "0 of 1",
        "the proof is not there yet: {ran}"
    );
    let gate = |root: &Path| ask(&project, root, "devflow_gate", json!({ "card": card }));
    assert_eq!(gate(&root)["result"], "failed");

    std::fs::write(root.join("shot.png"), "png").expect("the proof");
    let said = gate(&root);
    assert_eq!(said["result"], "passed", "{said}");
}

#[test]
fn a_code_change_after_the_run_is_stale_whatever_the_clocks_say() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = committed(&root);
    evidence(&project, &root, &card);
    let gate = || ask(&project, &root, "devflow_gate", json!({ "card": card }));
    assert_eq!(gate()["result"], "passed");

    std::fs::write(root.join("cli.rs"), "fn main() { todo!() }\n").expect("code");
    let an_hour_ago = SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(root.join("cli.rs"))
        .and_then(|file| file.set_modified(an_hour_ago))
        .expect("an old clock");
    let said = gate();
    assert_eq!(said["result"], "failed", "{said}");
    assert!(said["missing"][0]
        .as_str()
        .is_some_and(|missing| missing.contains("changed after run")));
}

#[test]
fn evidence_asked_again_at_the_same_head_reuses_the_green_run_unless_fresh() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = committed(&root);
    let first = evidence(&project, &root, &card);
    assert_eq!(first["green"], true, "{first}");
    assert!(first.get("reused_from").is_none(), "{first}");
    let stamp = |ran: &Value| {
        ran["run"]
            .as_str()
            .and_then(|run| run.rsplit('/').next())
            .map(str::to_owned)
    };

    let again = evidence(&project, &root, &card);
    assert_eq!(again["green"], true, "{again}");
    assert_eq!(
        again["reused_from"].as_str().map(str::to_owned),
        stamp(&first)
    );
    assert_eq!(
        ask(&project, &root, "devflow_gate", json!({ "card": card }))["result"],
        "passed"
    );

    let forced = ask(
        &project,
        &root,
        "devflow_evidence",
        json!({ "card": card, "fresh": true, "criteria": [{ "criterion": "rename changes the title", "tests": ["rename changes the title"] }] }),
    );
    assert_eq!(forced["green"], true, "{forced}");
    assert!(forced.get("reused_from").is_none(), "{forced}");
}

/// `ran`'s evidence.json as engine v0.2.1 wrote it: no commit, no tree.
fn as_v0_2_1(ran: &Value) {
    let at = PathBuf::from(ran["evidence"].as_str().expect("evidence.json"));
    let mut recorded: Value =
        serde_json::from_str(&std::fs::read_to_string(&at).expect("read")).expect("json");
    let fields = recorded.as_object_mut().expect("an object");
    assert!(fields.remove("head").is_some() && fields.remove("tree").is_some());
    std::fs::write(&at, recorded.to_string()).expect("written back");
}

fn written_at(path: &Path, at: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(at))
        .expect("its clock");
}

/// A run engine v0.2.1 recorded names no commit or tree: its proofs are not
/// looked for again, and the code is judged by when each file was written.
#[test]
fn a_run_recorded_by_engine_v0_2_1_is_judged_as_before() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (_store, project, card) = committed(&root);
    let gate = || ask(&project, &root, "devflow_gate", json!({ "card": card }));
    as_v0_2_1(&with_proof(&project, &root, &card));
    std::fs::write(root.join("shot.png"), "png").expect("the proof");
    let said = gate();
    assert_eq!(said["result"], "failed", "{said}");
    assert_eq!(said["criteria"], "0 of 1", "{said}");

    let ran = evidence(&project, &root, &card);
    assert!(ran.get("reused_from").is_none(), "{ran}");
    as_v0_2_1(&ran);
    std::fs::write(root.join("cli.rs"), "fn main() { todo!() }\n").expect("code");
    written_at(
        &root.join("cli.rs"),
        SystemTime::now() - std::time::Duration::from_secs(3600),
    );
    assert_eq!(
        gate()["result"],
        "passed",
        "an old clock was never a change"
    );
    written_at(
        &root.join("cli.rs"),
        SystemTime::now() + std::time::Duration::from_secs(5),
    );
    let said = gate();
    assert_eq!(said["result"], "failed", "{said}");
    assert!(said["missing"][0]
        .as_str()
        .is_some_and(|missing| missing.contains("changed after run")));
}
