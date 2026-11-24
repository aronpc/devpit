use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex as Shared, OnceLock};

use devflow_engine::config::Mode;
use devpit_core::store::decisions::DecisionWrite;

use super::*;

/// A provider on loopback answering every request, each question by
/// `answer(id)`, and keeping what it was asked.
fn provider(answer: fn(&str) -> Value) -> (String, Arc<Shared<Vec<Value>>>) {
    listening(move |asked| {
        let answers: serde_json::Map<String, Value> = asked["questions"]
            .as_object()
            .map(|questions| {
                questions
                    .keys()
                    .map(|id| (id.clone(), answer(id)))
                    .collect()
            })
            .unwrap_or_default();
        let reply = json!({ "answers": answers, "usage": { "cost": 0.0001 } });
        ("200 OK", reply.to_string())
    })
}

/// What Decisions answers a question it will not take, on every call.
pub(crate) const REFUSED: &str = "choice question `severity` needs criteria";

/// A provider on loopback refusing every request as Decisions does.
fn refusing() -> String {
    listening(|_| {
        let reply = json!({ "error": { "message": REFUSED } });
        ("400 Bad Request", reply.to_string())
    })
    .0
}

/// A server on loopback replying to each request by `reply(body)`.
fn listening(
    reply: impl Fn(&Value) -> (&'static str, String) + Send + 'static,
) -> (String, Arc<Shared<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!(
        "http://127.0.0.1:{}/api/v1",
        listener.local_addr().expect("addr").port()
    );
    let heard = Arc::new(Shared::new(Vec::new()));
    let kept = Arc::clone(&heard);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let asked: Value = serde_json::from_slice(&body).unwrap_or_default();
            let (status, said) = reply(&asked);
            if let Ok(mut all) = kept.lock() {
                all.push(asked);
            }
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{said}",
                said.len()
            );
        }
    });
    (base, heard)
}

/// Sure of everything: every rubric passes.
fn sure(id: &str) -> Value {
    match id {
        "severity" => {
            json!({ "type": "choice", "choice": "low", "probabilities": { "low": 0.9, "high": 0.1 } })
        }
        "duplicate" => noul(0.05),
        _ => noul(0.95),
    }
}

/// The four ways Decisions stands for a project's Devflow, which every reply
/// that asks it is checked in.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Standing {
    NoKey,
    NotUsed,
    Answering,
    Refusing,
}

pub(crate) const STANDINGS: [Standing; 4] = [
    Standing::NoKey,
    Standing::NotUsed,
    Standing::Answering,
    Standing::Refusing,
];

impl Standing {
    /// devflow.yaml's `decisions:`.
    pub(crate) fn yaml(self) -> &'static str {
        match self {
            Standing::NotUsed => "decisions:\n  use: false\n",
            _ => "decisions:\n  use: true\n",
        }
    }

    /// The settings `project_id` is asked with, in place of the key file's.
    pub(crate) fn give(self, project_id: &str) {
        static ANSWERING: OnceLock<String> = OnceLock::new();
        static REFUSING: OnceLock<String> = OnceLock::new();
        let answering = ANSWERING.get_or_init(|| provider(sure).0);
        let given = match self {
            Standing::NoKey => conf(answering, None),
            Standing::NotUsed | Standing::Answering => conf(answering, Some("key")),
            Standing::Refusing => conf(REFUSING.get_or_init(refusing), Some("key")),
        };
        GIVEN
            .lock()
            .expect("given")
            .insert(project_id.to_owned(), given);
    }

    /// That `reply` says where Decisions stands, and why.
    pub(crate) fn said_in(self, reply: &Value) {
        let said = &reply["decisions"];
        let reason = said["reason"].as_str().unwrap_or_default();
        let (state, why) = match self {
            Standing::NoKey => ("no_key", "no key — set one in Settings → Decisions"),
            Standing::NotUsed => ("off", "decisions.use is false"),
            Standing::Answering => ("on", ""),
            Standing::Refusing => ("failing", REFUSED),
        };
        assert_eq!(said["state"], state, "{self:?}: {reply}");
        assert!(reason.contains(why), "{self:?}: {reply}");
        if let Standing::Answering = self {
            assert!(said["reason"].is_null(), "{self:?}: {reply}");
        }
    }
}

fn noul(p: f64) -> Value {
    json!({ "type": "noul", "noul": p })
}

/// Sure of everything, except that a plan's criteria can be tested.
fn untestable_plan(id: &str) -> Value {
    match id {
        "testable" => noul(0.2),
        "duplicate" => noul(0.05),
        "severity" => {
            json!({ "type": "choice", "choice": "low", "probabilities": { "low": 0.9, "high": 0.1 } })
        }
        _ => noul(0.95),
    }
}

fn conf(base: &str, key: Option<&str>) -> Conf {
    Conf {
        base: base.to_owned(),
        model: "typesafe/jev-1.13".to_owned(),
        cap_usd: 1.0,
        key: key.map(str::to_owned),
        ..Conf::default()
    }
}

fn log_store(dir: &Path) -> Store {
    Store::open(&dir.join("decisions.db")).expect("a store")
}

/// Each of the four rubrics asked of a provider on this machine: every
/// request asks for no retention and no training, carries the rubric's own
/// questions — done once per criterion — and never a diff; each answer is
/// judged by its thresholds and logged.
#[test]
fn the_four_rubrics_are_asked_with_no_retention_and_judged_by_their_thresholds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (base, heard) = provider(untestable_plan);
    let criteria = [
        "opens the first link".to_owned(),
        "prints opened".to_owned(),
    ];
    let jev = Jev::with(
        log_store(dir.path()),
        conf(&base, Some("key")),
        "prj_jev",
        Some("card_1"),
        dir.path(),
        Mode::Shadow,
        &criteria,
    );
    assert!(jev.answers());
    let finding = json!({
        "file": "app.py", "line": 2, "severity": "high", "axis": "security", "title": "Shell injection",
        "description": "d", "evidence": "e", "poc": "p", "agent": "claude", "persona": "devflow-security",
        "anchor": "added",
    });
    let brief = devflow_engine::brief::Brief {
        goal: Some("open links".into()),
        success: Some("tasks open 1 opens it".into()),
        out_of_scope: Some(vec!["previews".into()]),
        ..Default::default()
    };
    let slices: Vec<devflow_engine::plan::Slice> = serde_json::from_value(json!([
        { "key": "s1", "title": "Open a link", "criteria": ["opens it"] }
    ]))
    .expect("a plan");
    let asked = [
        (
            jev::FINDING,
            jev::finding_state(&finding, &[], &[(1, 3)], &criteria),
        ),
        (
            jev::BRIEF,
            jev::brief_state("[devflow] Brief: Links", &brief),
        ),
        (jev::PLAN, jev::plan_state(&slices)),
        (jev::DONE, json!({ "criteria": criteria })),
    ];
    let outcomes: Vec<String> = asked
        .iter()
        .map(|(rubric, state)| {
            let said = jev.judge(rubric, state);
            assert!(said.decision.is_some(), "{rubric} was logged");
            assert_eq!(said.mode, Mode::Shadow);
            said.outcome
        })
        .collect();
    assert_eq!(outcomes, ["pass", "pass", "fail", "pass"]);

    let heard = heard.lock().expect("heard").clone();
    assert_eq!(heard.len(), 4);
    for request in &heard {
        assert_eq!(request["provider"]["zdr"], true);
        assert_eq!(request["provider"]["data_collection"], "deny");
        assert!(!request["state"].to_string().contains("diff --git"));
    }
    assert!(
        heard[0]["questions"].get("inDiff").is_none(),
        "the anchor says it"
    );
    let told: Value = serde_json::from_str(heard[0]["state"].as_str().unwrap_or_default())
        .expect("the state is JSON text");
    assert_eq!(told["finding"]["anchor"], "added");
    assert!(heard[1]["questions"].get("outOfScope").is_some());
    assert!(heard[2]["questions"].get("vertical").is_some());
    assert!(heard[3]["questions"].get("criterion2").is_some());
    let logged = jev
        .store
        .lock()
        .expect("store")
        .decisions_latest(10)
        .expect("log");
    let mut rubrics: Vec<String> = logged.iter().filter_map(|row| row.rubric.clone()).collect();
    rubrics.sort();
    assert_eq!(
        rubrics,
        [
            "devflow-brief",
            "devflow-done",
            "devflow-finding",
            "devflow-plan"
        ]
    );
    assert!(logged.iter().all(|row| row.mode == "shadow"));
}

/// The gate's own rubric moved with the others: a project's
/// `.devpit/devflow/rubrics/` overrides devpit's, and `.devpit/rubrics/` no
/// longer does for the flow's own names.
#[test]
fn a_projects_own_rubric_in_its_devflow_folder_overrides_devpits() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let ours = r#"{ "questions": { "short": { "type": "noul", "instructions": "Is it short?" } }, "thresholds": { "short": { "min": 0.5 } } }"#;
    std::fs::create_dir_all(root.join(".devpit/rubrics")).expect("dir");
    std::fs::write(root.join(".devpit/rubrics/devflow-done.json"), ours).expect("old place");
    let done = rubric(root, jev::DONE, &["a".to_owned()]).expect("done");
    assert!(
        done.questions.get("criterion1").is_some(),
        "the old place is not read"
    );

    std::fs::create_dir_all(root.join(jev::dir(root))).expect("dir");
    for name in [jev::DONE, jev::BRIEF] {
        std::fs::write(root.join(jev::dir(root)).join(format!("{name}.json")), ours)
            .expect("write");
        let read = rubric(root, name, &[]).expect("ours");
        assert_eq!(read.name, name);
        assert!(read.questions.get("short").is_some());
    }
    let plan = rubric(root, jev::PLAN, &[]).expect("devpit's");
    assert!(plan.questions.get("dependencies").is_some());
    assert!(rubric(root, "../escape", &[]).is_err());
}

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

fn epic_of(project: &Project) -> String {
    let said = crate::devflow_plan::respond(
        devpit_core::locale::Locale::En,
        None,
        "devflow_brief",
        project,
        &json!({
            "title": "Links", "goal": "Open a task's link", "success": "tasks open 1 opens it",
            "out_of_scope": ["previews"]
        }),
    )
    .expect("a brief");
    said["cardId"].as_str().expect("an id").to_owned()
}

/// The plans that hold under enforce: only one whose rubric devflow.yaml
/// names; otherwise the answer is advice shown with the plan.
const PLAN_STANDINGS: [(Mode, bool); 3] = [
    (Mode::Shadow, false),
    (Mode::Enforce, false),
    (Mode::Enforce, true),
];

fn plan_config(opted_in: bool) -> Config {
    let mut config = Config::default();
    config.decisions.use_ = true;
    if opted_in {
        config
            .decisions
            .rubrics
            .insert("plan".to_owned(), jev::PLAN.to_owned());
    }
    config
}

/// The same plan, the same answer: Jev's "fail" is shown with the plan and
/// the cards are made, unless the project opted the plan rubric in and Jev
/// acts: then it holds them, and says why.
#[test]
fn the_plans_fail_is_advice_unless_the_project_opts_in_and_enforce_holds_it() {
    let (base, _heard) = provider(untestable_plan);
    for (mode, opted_in) in PLAN_STANDINGS {
        let config = plan_config(opted_in);
        let held = mode == Mode::Enforce && opted_in;
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, project) = project(dir.path());
        let epic = epic_of(&project);
        let jev = Jev::with(
            log_store(dir.path()),
            conf(&base, Some("key")),
            &project.id,
            Some(&epic),
            dir.path(),
            mode,
            &[],
        );
        let plan = |apply: bool| {
            crate::devflow_plan::respond_judged(
                devpit_core::locale::Locale::En,
                None,
                "devflow_plan",
                &project,
                &json!({ "epic": epic, "apply": apply, "cards": [
                    { "key": "s1", "title": "Open a link", "criteria": ["opens it"] }
                ] }),
                Some(&config),
                Some(&jev),
            )
        };
        let preview = plan(false).expect("a preview");
        assert!(
            preview["jev"]["brief"].is_null(),
            "no brief rubric by default: {preview}"
        );
        // The preview is judged too: enforce warns there, and refuses nothing yet.
        assert_eq!(preview["jev"]["plan"]["outcome"], "fail", "{preview}");
        assert_eq!(
            preview["warning"]
                .as_str()
                .is_some_and(|warning| warning.contains("testable")),
            held,
            "{mode:?}, opted in {opted_in}: {preview}"
        );
        assert_eq!(
            preview["advice"]
                .as_str()
                .is_some_and(|advice| advice.contains("testable")),
            !held,
            "the answer is shown with the plan: {preview}"
        );
        let applied = plan(true);
        let cards = store.cards(&project.id).expect("cards").len();
        if held {
            let why = applied.expect_err("held");
            assert!(
                why.contains("testable") && why.contains("nothing was created"),
                "{why}"
            );
            assert_eq!(cards, 1, "only the epic");
        } else {
            let said = applied.expect("made");
            assert_eq!(said["jev"]["plan"]["outcome"], "fail", "{said}");
            assert_eq!(cards, 2, "{mode:?}: the epic and its card");
        }
    }
}

/// The person's click on the board is judged as an agent's apply is: the
/// plan rubric is asked and the cards are made, unless the project opted it
/// in and enforce refuses them, with why.
#[test]
fn the_board_create_runs_the_plan_rubric_and_only_an_opted_in_enforce_refuses() {
    let (base, _heard) = provider(untestable_plan);
    for (mode, opted_in) in PLAN_STANDINGS {
        let config = plan_config(opted_in);
        let dir = tempfile::tempdir().expect("tempdir");
        let (store, project) = project(dir.path());
        let epic = epic_of(&project);
        let jev = Jev::with(
            log_store(dir.path()),
            conf(&base, Some("key")),
            &project.id,
            Some(&epic),
            dir.path(),
            mode,
            &[],
        );
        crate::devflow_plan::respond(
            devpit_core::locale::Locale::En,
            None,
            "devflow_plan",
            &project,
            &json!({ "epic": epic, "cards": [
                { "key": "s1", "title": "Open a link", "criteria": ["opens it"] }
            ] }),
        )
        .expect("a preview");
        let created = crate::devflow_plan::created_on_board(
            &store,
            &project.id,
            &epic,
            Some((&config, &jev)),
        );
        let asked = jev
            .store
            .lock()
            .expect("store")
            .decisions_latest(5)
            .expect("log")
            .len();
        assert_eq!(asked, 1, "{mode:?}: the plan rubric, asked on the click");
        let cards = store.cards(&project.id).expect("cards").len();
        if mode == Mode::Enforce && opted_in {
            let why = created.expect_err("refused").message;
            assert!(
                why.contains("testable") && why.contains("nothing was created"),
                "{why}"
            );
            assert_eq!(cards, 1, "only the epic");
        } else {
            assert_eq!(created.expect("made").card_ids.len(), 1, "{mode:?}");
            assert_eq!(cards, 2, "the epic and its card");
        }
    }
}

/// However Decisions stands, a plan's reply says so and its cards are made.
#[test]
fn a_plan_says_where_decisions_stands_and_makes_its_cards_in_each_standing() {
    for standing in STANDINGS {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let (store, project) = project(&root);
        std::fs::create_dir_all(root.join(".devpit/devflow")).expect("folder");
        std::fs::write(
            root.join(".devpit/devflow/devflow.yaml"),
            format!("version: 1\nenabled: true\n{}", standing.yaml()),
        )
        .expect("yaml");
        standing.give(&project.id);
        let epic = epic_of(&project);
        let plan = |apply: bool| {
            crate::devflow_plan::respond(
                devpit_core::locale::Locale::En,
                None,
                "devflow_plan",
                &project,
                &json!({ "epic": epic, "apply": apply, "cards": [
                    { "key": "s1", "title": "Open a link", "criteria": ["opens it"] }
                ] }),
            )
            .expect("planned")
        };
        standing.said_in(&plan(false));
        standing.said_in(&plan(true));
        let cards = store.cards(&project.id).expect("cards").len();
        assert_eq!(cards, 2, "{standing:?}: the epic and its card");
    }
}

/// The panel's word: the project's last call, so a refusal reads as failing
/// until an answer follows it.
#[test]
fn the_panel_says_failing_with_why_until_a_call_is_answered() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project) = project(&root);
    std::fs::create_dir_all(root.join(".devpit/devflow")).expect("folder");
    std::fs::write(
        root.join(".devpit/devflow/devflow.yaml"),
        "version: 1\nenabled: true\ndecisions:\n  use: true\n",
    )
    .expect("yaml");
    let reply = |standing: DevflowDecisions| json!({ "decisions": standing });
    Standing::NoKey.give(&project.id);
    Standing::NoKey.said_in(&reply(standing(&store, &project.id, &root)));
    Standing::Refusing.give(&project.id);
    Standing::Answering.said_in(&reply(standing(&store, &project.id, &root)));
    let ask = |given: Standing| {
        given.give(&project.id);
        let devflow_engine::State::Valid(_, config) = crate::devflow::read(&root) else {
            panic!("devflow.yaml reads");
        };
        let jev = Jev::of(&project, &config, &root, None, &[]).expect("used");
        jev.judge(jev::PLAN, &json!({}));
    };
    ask(Standing::Refusing);
    Standing::Refusing.said_in(&reply(standing(&store, &project.id, &root)));
    ask(Standing::Answering);
    Standing::Answering.said_in(&reply(standing(&store, &project.id, &root)));
}

/// A call never sent is not in the log, but this reply's state still says
/// why Decisions did not answer it.
#[test]
fn a_spent_budget_reads_as_failing_though_nothing_was_logged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let spent = Conf {
        cap_usd: 0.0,
        ..conf("http://127.0.0.1:9", Some("key"))
    };
    let jev = Jev::with(
        log_store(dir.path()),
        spent,
        "prj_spent",
        None,
        dir.path(),
        Mode::Shadow,
        &[],
    );
    assert_eq!(jev.state().state, devpit_rpc::DecisionsState::On);
    jev.judge(jev::PLAN, &json!({}));
    let state = jev.state();
    assert_eq!(state.state, devpit_rpc::DecisionsState::Failing);
    assert!(state.reason.is_some_and(|why| why.contains("budget")));
}

/// No key: nothing is asked, nothing acts, and the plan is made as it
/// would be without Decisions.
#[test]
fn without_a_key_devflow_works_as_it_does_without_jev() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(dir.path());
    let epic = epic_of(&project);
    let mut config = Config::default();
    config.decisions.use_ = true;
    let jev = Jev::with(
        log_store(dir.path()),
        conf("http://127.0.0.1:9", None),
        &project.id,
        Some(&epic),
        dir.path(),
        Mode::Enforce,
        &[],
    );
    assert!(
        !jev.answers(),
        "a review falls back to a model pass or none"
    );
    let said = jev.judge(jev::FINDING, &json!({}));
    assert_eq!(said.outcome, "skipped");
    assert!(!said.acts());
    let ask = |apply: bool| {
        crate::devflow_plan::respond_judged(
 devpit_core::locale::Locale::En,
 None,
            "devflow_plan",
            &project,
            &json!({ "epic": epic, "apply": apply, "cards": [{ "key": "s1", "title": "Open a link" }] }),
            Some(&config),
                Some(&jev),
        )
        .expect("planned")
    };
    ask(false);
    let made = ask(true);
    assert_eq!(made["jev"]["plan"]["outcome"], "skipped", "{made}");
    assert_eq!(store.cards(&project.id).expect("cards").len(), 2);
    assert!(jev
        .store
        .lock()
        .expect("store")
        .decisions_latest(5)
        .expect("log")
        .is_empty());
}

fn decision(store: &Store, project: &str, card: &str, rubric: &str, outcome: &str) -> String {
    store
        .log_decision(
            &DecisionWrite {
                project_id: Some(project),
                card_id: Some(card),
                gate: rubric,
                mode: "shadow",
                rubric: Some(rubric),
                rubric_version: Some("1"),
                questions: "{}",
                answers: Some(r#"{"real":{"type":"noul","noul":0.9}}"#),
                thresholds: Some(r#"{"real":{"min":0.7}}"#),
                outcome,
                cost_usd: 0.0,
                latency_ms: 1,
                state_digest: "d:1",
                failure: None,
            },
            crate::deciding::now(),
        )
        .expect("logged")
}

fn verdict_of(store: &Store, card: &str, rubric: &str, id: &str) -> Option<String> {
    store
        .decisions_of_card(card, rubric)
        .expect("read")
        .into_iter()
        .find(|row| row.id == id)
        .and_then(|row| row.person_verdict)
}

/// What the person does is the label: reopening a card Jev called done says
/// it was not; calling a finding a false positive says it was not real, and
/// taking that back says it was; finishing the card says yes to what stood.
#[test]
fn reopening_a_card_and_a_false_positive_are_labels_on_jevs_decisions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let (store, project) = project(&root);
    let columns = store.columns(&project.id).expect("columns");
    let role = |name: &str| {
        columns
            .iter()
            .find(|column| {
                devpit_core::store::column_roles::role_of(column.role.as_deref(), &column.name)
                    == Some(name)
            })
            .map(|column| column.id.clone())
            .expect(name)
    };
    let (done, doing) = (role("done"), role("doing"));
    let card = store
        .create_card(&project.id, &done, "[devflow] Open links", "")
        .expect("card");
    let here = root.display().to_string();
    store
        .set_card_front(&card, Some(&here), Some("HEAD"))
        .expect("its checkout");

    let called_done = decision(&store, &project.id, &card, jev::DONE, "pass");
    moved(&store, &project.id, &card, Some(&done), &doing);
    assert_eq!(
        verdict_of(&store, &card, jev::DONE, &called_done).as_deref(),
        Some("fail")
    );

    let real = decision(&store, &project.id, &card, jev::FINDING, "pass");
    let other = decision(&store, &project.id, &card, jev::FINDING, "pass");
    let path = crate::devflow_evidence::runs_of(&project.id, &root)
        .expect("runs")
        .file_in(&card, Some("20261011T143005Z"), "findings.json")
        .expect("a run");
    let finding = |title: &str, decision: &str| {
        json!({
            "file": "app.py", "line": 2, "severity": "low", "axis": "security", "title": title,
            "description": "d", "evidence": "e", "agent": "claude", "persona": "devflow-security",
            "jev": { "decision": decision, "mode": "shadow", "outcome": "pass" },
        })
    };
    std::fs::write(
        &path,
        json!({ "version": 1, "head": "h", "verdict": "approve",
            "findings": [finding("one", &real), finding("two", &other)] })
        .to_string(),
    )
    .expect("findings.json");
    let run = format!("devflow:{card}/20261011T143005Z");
    crate::devflow_review::mark(&store, &run, 0, true).expect("a false positive");
    assert_eq!(
        verdict_of(&store, &card, jev::FINDING, &real).as_deref(),
        Some("fail")
    );
    let kept: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("kept")).expect("json");
    assert_eq!(
        kept["findings"][0]["jev"]["person_verdict"], "fail",
        "beside Jev's word"
    );
    crate::devflow_review::mark(&store, &run, 0, false).expect("taken back");
    assert_eq!(
        verdict_of(&store, &card, jev::FINDING, &real).as_deref(),
        Some("pass")
    );

    moved(&store, &project.id, &card, Some(&doing), &done);
    assert_eq!(
        verdict_of(&store, &card, jev::FINDING, &other).as_deref(),
        Some("pass"),
        "a finding left standing when the card was finished"
    );
}

fn row(rubric: &str, real: f64, outcome: &str, verdict: &str) -> DecisionRow {
    DecisionRow {
        id: format!("dcn_{real}"),
        at: 0,
        project_id: Some("prj".into()),
        card_id: Some("card".into()),
        gate: rubric.into(),
        mode: "shadow".into(),
        rubric: Some(rubric.into()),
        questions: "{}".into(),
        answers: Some(
            json!({ "real": noul(real), "inDiff": noul(0.99), "severity": { "choice": "low", "probabilities": { "low": 0.1 } } })
                .to_string(),
        ),
        thresholds: Some(
            r#"{"real":{"min":0.7,"grey":0.4},"inDiff":{"min":0.6},"severity":{"min":0.5},"duplicate":{"max":0.3}}"#
                .into(),
        ),
        outcome: outcome.into(),
        cost_usd: 0.0,
        latency_ms: 0,
        state_digest: "d".into(),
        person_verdict: (!verdict.is_empty()).then(|| verdict.to_owned()),
        failure: None,
    }
}

/// Synthetic decisions: `devflow-finding` passed three findings the person
/// then called false positives, all scored under 0.8. Its accuracy says so,
/// and a higher `min` is suggested; a choice's low confidence is not what
/// its passing turned on.
#[test]
fn accuracy_and_a_suggested_threshold_come_from_the_labelled_decisions() {
    let mut rows = vec![
        row(jev::FINDING, 0.72, "pass", "fail"),
        row(jev::FINDING, 0.75, "pass", "fail"),
        row(jev::FINDING, 0.78, "pass", "fail"),
        row(jev::FINDING, 0.2, "fail", "fail"),
        row(jev::FINDING, 0.5, "grey", "pass"),
        row(jev::FINDING, 0.6, "grey", ""),
    ];
    for real in [0.9, 0.92, 0.95, 0.97] {
        rows.push(row(jev::FINDING, real, "pass", "pass"));
    }
    rows.push(row(jev::DONE, 0.9, "pass", ""));
    let all = calibration(&rows);
    assert_eq!(all[0].rubric, jev::FINDING, "the flow's order");
    let finding = &all[0];
    assert_eq!(finding.decisions, 10);
    assert_eq!(finding.labelled, 9);
    assert_eq!(finding.grey, 1);
    assert_eq!(finding.agreed, 5);
    assert_eq!(finding.accuracy, Some(5.0 / 8.0));
    assert_eq!(
        finding.min,
        Some(0.5),
        "the lowest min of its questions now"
    );
    let suggested = finding.suggested_min.expect("suggested");
    assert!(suggested > 0.78 && suggested <= 0.9, "{suggested}");
    assert_eq!(all[1].rubric, jev::DONE);
    assert_eq!(all[1].labelled, 0);
    assert!(all[1].note.is_some());
    assert_eq!(
        score_of(
            &json!({ "real": noul(0.8), "inDiff": noul(0.6), "severity": { "choice": "low", "probabilities": { "low": 0.1 } } }),
            &json!({ "real": { "min": 0.7 }, "inDiff": { "min": 0.6 }, "severity": { "min": 0.5 } })
        ),
        Some(0.6)
    );
}

/// A call Decisions never answered is not a decision: it is not counted
/// under its rubric, a rubric with nothing else does not show, and marking
/// its finding labels nothing.
#[test]
fn a_skipped_call_is_neither_counted_nor_labelled_as_a_decision() {
    let mut rows = vec![row(jev::FINDING, 0.9, "pass", "pass")];
    for at in 0..3 {
        let mut skipped = row(jev::FINDING, f64::from(at) / 10.0, "skipped", "fail");
        skipped.id = format!("dcn_skipped_{at}");
        rows.push(skipped);
    }
    rows.push(row(jev::BRIEF, 0.5, "skipped", ""));
    let all = calibration(&rows);
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!((all[0].decisions, all[0].labelled), (1, 1));
    assert_eq!(all[0].accuracy, Some(1.0));

    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = project(&dir.path().canonicalize().expect("root"));
    let id = decision(&store, &project.id, "card", jev::FINDING, "skipped");
    let finding = json!({ "jev": { "decision": id, "mode": "shadow", "outcome": "skipped" } });
    finding_marked(&store, &finding, true);
    let logged = store.decisions_latest(5).expect("log");
    let row = logged.iter().find(|row| row.id == id).expect("logged");
    assert_eq!(row.person_verdict, None);
}

/// One finding as Decisions answered it in 0.43's shadow run, shaped on
/// the calls whose findings people confirmed: how sure it was the finding
/// is real, and how its severity split.
fn answered(real: f64, severity: &str, split: Value) -> Value {
    json!({
        "real": noul(real),
        "severity": { "type": "choice", "choice": severity, "probabilities": split },
        "duplicate": noul(0.1),
    })
}

/// Synthetic findings with the person's word on each, replayed through
/// devpit's finding rubric as it ships: of those the person confirmed, at
/// most one in ten fails — the split severities that failed half of them in
/// 0.43 pass as the band they make — and the false positives still fail.
#[test]
fn the_finding_rubric_fails_few_of_the_findings_people_confirmed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rubric = rubric(dir.path(), jev::FINDING, &[]).expect("devpit's");
    assert_eq!(rubric.version.as_deref(), Some("3"));
    let mut replayed: Vec<(Value, bool)> = Vec::new();
    for _ in 0..8 {
        replayed.push((
            answered(
                0.92,
                "low",
                json!({ "low": 0.8, "info": 0.1, "medium": 0.1 }),
            ),
            true,
        ));
    }
    for _ in 0..10 {
        let split = json!({ "medium": 0.49, "high": 0.32, "low": 0.19 });
        replayed.push((answered(0.85, "medium", split), true));
    }
    for _ in 0..6 {
        let split = json!({ "high": 0.45, "critical": 0.3, "medium": 0.25 });
        replayed.push((answered(0.8, "high", split), true));
    }
    for _ in 0..4 {
        replayed.push((
            answered(0.55, "low", json!({ "low": 0.7, "medium": 0.3 })),
            true,
        ));
    }
    for _ in 0..2 {
        replayed.push((
            answered(0.3, "low", json!({ "low": 0.7, "medium": 0.3 })),
            true,
        ));
    }
    for _ in 0..6 {
        replayed.push((
            answered(0.2, "low", json!({ "low": 0.6, "info": 0.4 })),
            false,
        ));
    }
    let labelled: Vec<Labelled> = replayed
        .iter()
        .map(|(answers, truth)| Labelled {
            score: None,
            outcome: crate::rubric::judged(&rubric, answers)
                .0
                .as_str()
                .to_owned(),
            truth: *truth,
        })
        .collect();
    let (failed, confirmed) = calibrate::failing_confirmed(&labelled);
    assert_eq!(confirmed, 30);
    assert!(
        failed as f64 <= confirmed as f64 * calibrate::ENFORCE_MOST_FAILING,
        "{failed} of {confirmed} confirmed findings failed"
    );
    assert!(calibrate::enforceable(&labelled).is_ok());
    assert!(labelled
        .iter()
        .filter(|one| !one.truth)
        .all(|one| one.outcome == "fail"));
}

/// Enforce acts for devpit's finding rubric only past the bar, counted on
/// the decisions judged by the thresholds it has now; the other rubrics act
/// on the person's click alone.
#[test]
fn enforce_for_the_finding_rubric_is_refused_below_the_calibration_bar() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (base, _) = provider(sure);
    let jev = Jev::with(
        log_store(dir.path()),
        conf(&base, Some("key")),
        "prj_bar",
        Some("card_1"),
        dir.path(),
        Mode::Enforce,
        &[],
    );
    let finding = json!({
        "file": "app.py", "line": 2, "severity": "low", "axis": "security", "title": "t",
        "description": "d", "evidence": "e", "agent": "claude", "persona": "devflow-security",
        "anchor": "added",
    });
    let state = jev::finding_state(&finding, &[], &[], &[]);
    assert_eq!(jev.judge(jev::FINDING, &state).mode, Mode::Shadow);
    assert_eq!(
        jev.judge(jev::BRIEF, &json!({ "goal": "open links" })).mode,
        Mode::Enforce
    );

    let current = rubric(dir.path(), jev::FINDING, &[]).expect("devpit's");
    let confirmed_under = |thresholds: &str| {
        let store = jev.store.lock().expect("store");
        for _ in 0..calibrate::ENFORCE_AFTER {
            let id = store
                .log_decision(
                    &DecisionWrite {
                        project_id: Some("prj_bar"),
                        card_id: Some("card_1"),
                        gate: jev::FINDING,
                        mode: "shadow",
                        rubric: Some(jev::FINDING),
                        rubric_version: None,
                        questions: "{}",
                        answers: Some(r#"{"real":{"type":"noul","noul":0.9}}"#),
                        thresholds: Some(thresholds),
                        outcome: "pass",
                        cost_usd: 0.0,
                        latency_ms: 1,
                        state_digest: "d:1",
                        failure: None,
                    },
                    crate::deciding::now(),
                )
                .expect("logged");
            store.label_decision(&id, "pass", "kept").expect("labelled");
        }
    };
    confirmed_under(r#"{"real":{"min":0.7},"inDiff":{"min":0.6}}"#);
    assert_eq!(
        jev.judge(jev::FINDING, &state).mode,
        Mode::Shadow,
        "an older version's decisions say nothing of this one"
    );
    confirmed_under(&json!(current.thresholds).to_string());
    assert_eq!(jev.judge(jev::FINDING, &state).mode, Mode::Enforce);
}
