use super::*;

fn never(_: &str, _: Value) -> Result<Value, String> {
    panic!("the app should not have been asked")
}

fn reply(line: &str, ask: &dyn Fn(&str, Value) -> Result<Value, String>) -> Value {
    serde_json::from_str(&handle(line, ask).expect("an answer")).expect("json")
}

fn offering(
    line: &str,
    offer: &Offer,
    ask: &dyn Fn(&str, Value) -> Result<Value, String>,
) -> Value {
    serde_json::from_str(
        &handle_offering(line, ask, true, offer, &Seat::default()).expect("an answer"),
    )
    .expect("json")
}

fn listed(offer: &Offer) -> Vec<String> {
    offering(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        offer,
        &never,
    )["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

struct MemoryInput(std::io::Cursor<Vec<u8>>);

impl std::io::Read for MemoryInput {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buffer)
    }
}

impl BufRead for MemoryInput {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.0.consume(amount);
    }

    fn read_line(&mut self, _: &mut String) -> std::io::Result<usize> {
        panic!("the message was read into a String before its size was bounded")
    }

    fn read_until(&mut self, _: u8, _: &mut Vec<u8>) -> std::io::Result<usize> {
        panic!("read_until was called without a size limit")
    }
}

fn answered_messages(input: impl BufRead, ask: Ask) -> Vec<Value> {
    let messages = Arc::new(Mutex::new(Vec::<Value>::new()));
    let out: Out = {
        let messages = messages.clone();
        Arc::new(move |message: &str| {
            messages
                .lock()
                .expect("messages")
                .push(serde_json::from_str(message).expect("json"));
            true
        })
    };
    let home = tempfile::tempdir().expect("home");
    answered(
        input,
        &ask,
        true,
        &Mutex::new(Watch::new(home.path())),
        &AtomicBool::new(true),
        &out,
    );
    let answered = messages.lock().expect("messages").clone();
    answered
}

#[test]
fn the_handshake_offers_tools_and_says_what_devpit_is() {
    let said = reply(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
        &never,
    );
    assert_eq!(said["result"]["protocolVersion"], "2025-03-26");
    assert!(said["result"]["capabilities"]["tools"].is_object());
    assert!(said["result"]["instructions"]
        .as_str()
        .unwrap_or_default()
        .contains("devpit_context"));
}

/// Switching Devflow on or off changes the list mid-session.
#[test]
fn the_handshake_says_the_tool_list_can_change() {
    let said = reply(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &never,
    );
    for list in ["tools", "resources", "prompts"] {
        assert_eq!(
            said["result"]["capabilities"][list]["listChanged"], true,
            "{list}"
        );
    }
    assert!(greets(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#));
    assert!(!greets(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#));
}

/// The names devpit listed before Devflow existed, in their order.
const BEFORE_DEVFLOW: [&str; 36] = [
    "devpit_context",
    "devpit_board",
    "devpit_artifacts",
    "devpit_artifact_save",
    "devpit_artifact_restore",
    "devpit_artifact_remove",
    "devpit_projects",
    "devpit_sessions",
    "devpit_session_screen",
    "devpit_stop_session",
    "devpit_resume_session",
    "devpit_session_transcript",
    "devpit_repo_state",
    "devpit_remind",
    "devpit_reminders",
    "devpit_resolve_reminder",
    "devpit_propose_project",
    "devpit_draft_reply",
    "devpit_send_draft",
    "devpit_handoff",
    "devpit_mcp_health",
    "devpit_log",
    "devpit_start_session",
    "devpit_project_agent_ask",
    "devpit_project_agent_brief",
    "devpit_card",
    "devpit_comment",
    "devpit_comment_edit",
    "devpit_comment_delete",
    "devpit_create_card",
    "devpit_update_card",
    "devpit_start_card",
    "devpit_finish_card",
    "devpit_move_card",
    "devpit_decide",
    "devpit_rubric_run",
];

/// The message bus's tools, after every tool devpit had before them.
const BUS: [&str; 2] = ["devpit_session_message", "devpit_session_notify_idle"];

#[test]
fn with_devflow_off_the_list_and_the_instructions_are_what_they_were() {
    let off = listed(&Offer::Off);
    assert_eq!(off[..36], BEFORE_DEVFLOW);
    assert_eq!(off[36..], BUS);
    let said = offering(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &Offer::Off,
        &never,
    );
    assert_eq!(said["result"]["instructions"], crate::guide::INSTRUCTIONS);
}

#[test]
fn the_setup_and_on_demand_tools_come_after_devpits_own_until_devflow_is_switched_off() {
    let setup = listed(&Offer::Setup);
    assert_eq!(setup[..36], BEFORE_DEVFLOW);
    assert_eq!(setup[36..38], BUS);
    assert_eq!(
        setup[38..],
        [
            "devpit_devflow_status",
            "devpit_devflow_setup",
            "devpit_devflow_set",
            "devpit_devflow_skill",
            "devpit_devflow_plan",
            "devpit_devflow_review",
            "devpit_devflow_publish"
        ]
    );
    let schema = |name: &str| {
        (SETUP_TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .expect("a setup tool")
            .input)()
    };
    assert_eq!(
        schema("devpit_devflow_setup")["properties"]["preset"]["enum"],
        json!(["recommended", "custom"])
    );
    assert_eq!(
        schema("devpit_devflow_set")["required"],
        json!(["path", "value"])
    );
    let active = offering(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &Offer::Active(vec!["plan".into()]),
        &never,
    );
    assert!(active["result"]["instructions"]
        .as_str()
        .unwrap_or_default()
        .contains("Devflow is on in this project"));
}

#[test]
fn a_setup_tool_answers_with_devflow_off_in_the_project() {
    let ask = |method: &str, params: Value| -> Result<Value, String> {
        assert_eq!(method, "devflow_setup");
        assert_eq!(params, json!({ "preset": "recommended" }));
        Ok(json!({ "applied": false }))
    };
    let said = offering(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"devpit_devflow_setup","arguments":{"preset":"recommended"}}}"#,
        &Offer::Off,
        &ask,
    );
    assert_eq!(said["result"]["isError"], false);
}

#[test]
fn a_notification_gets_no_answer() {
    assert_eq!(
        handle(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &never
        ),
        None
    );
}

#[test]
fn every_tool_is_listed_with_a_schema() {
    let said = reply(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, &never);
    let tools = said["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), TOOLS.len());
    assert!(tools
        .iter()
        .all(|tool| tool["inputSchema"]["type"] == "object"));
}

#[test]
fn a_call_asks_the_app_by_its_method_and_says_what_it_answered() {
    let ask = |method: &str, params: Value| -> Result<Value, String> {
        assert_eq!(method, "card");
        assert_eq!(params, json!({ "cardId": "crd_1" }));
        Ok(json!({ "title": "Fix login" }))
    };
    let said = reply(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"devpit_card","arguments":{"cardId":"crd_1"}}}"#,
        &ask,
    );
    assert_eq!(said["result"]["isError"], false);
    assert!(said["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("Fix login"));
}

/// The agent reads a refusal and carries on; it is not a broken server.
#[test]
fn a_refusal_is_the_tools_error_not_the_protocols() {
    let ask = |_: &str, _: Value| -> Result<Value, String> { Err("devpit is not open".into()) };
    let said = reply(
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"devpit_board"}}"#,
        &ask,
    );
    assert!(said.get("error").is_none());
    assert_eq!(said["result"]["isError"], true);
}

#[test]
fn an_unknown_method_is_a_protocol_error() {
    let said = reply(
        r#"{"jsonrpc":"2.0","id":5,"method":"completion/complete"}"#,
        &never,
    );
    assert_eq!(said["error"]["code"], -32601);
}

#[test]
fn no_tool_answers_a_question_a_session_is_stopped_on() {
    // Answering a prompt is the person's, from the window; an agent only reads.
    assert!(!TOOLS
        .iter()
        .any(|tool| tool.method.contains("answer") || tool.method.contains("press")));
}

#[test]
fn the_board_card_and_sessions_come_with_pages_a_host_can_read() {
    let ask = |_: &str, _: Value| -> Result<Value, String> { Ok(Value::Null) };
    let listed: Value = serde_json::from_str(
        &handle(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, &ask).expect("answer"),
    )
    .expect("json");
    let uri = |name: &str| {
        listed["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .find(|tool| tool["name"] == name)
            .and_then(|tool| {
                tool["_meta"]["ui"]["resourceUri"]
                    .as_str()
                    .map(str::to_owned)
            })
    };
    assert_eq!(
        uri("devpit_board").as_deref(),
        Some("ui://devpit/board.html")
    );
    assert_eq!(uri("devpit_card").as_deref(), Some("ui://devpit/card.html"));
    assert_eq!(
        uri("devpit_sessions").as_deref(),
        Some("ui://devpit/sessions.html")
    );
    assert_eq!(uri("devpit_comment"), None);

    let read: Value = serde_json::from_str(
        &handle(
            r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"ui://devpit/board.html"}}"#,
            &ask,
        )
        .expect("answer"),
    )
    .expect("json");
    let page = &read["result"]["contents"][0];
    assert_eq!(page["mimeType"], crate::apps::APP_MIME);
    let html = page["text"].as_str().expect("html");
    assert!(html.contains("ui/initialize") && html.contains("devpit_start_session"));
    // Nothing is fetched from anywhere: no host is granted.
    assert!(!html.contains("src=\"http") && !html.contains("href=\"http"));
    assert_eq!(
        page["_meta"]["ui"]["csp"]["connectDomains"],
        serde_json::json!([])
    );

    let missing = handle(
        r#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"ui://devpit/nope.html"}}"#,
        &ask,
    )
    .expect("answer");
    assert!(missing.contains("-32002"));
}

#[test]
fn decisions_come_as_two_tools_with_their_questions_bounded() {
    let decide = TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_decide")
        .expect("devpit_decide");
    assert_eq!(decide.method, "decide");
    let schema = (decide.input)();
    assert_eq!(schema["required"], json!(["state", "questions"]));
    assert_eq!(schema["properties"]["questions"]["maxProperties"], 20);
    assert_eq!(
        schema["properties"]["questions"]["additionalProperties"]["properties"]["type"]["enum"],
        json!(["noul", "choice", "score"])
    );
    let rubric = TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_rubric_run")
        .expect("devpit_rubric_run");
    assert_eq!(rubric.method, "rubric_run");
    assert_eq!((rubric.input)()["required"], json!(["rubric", "state"]));
}

/// A model over the network takes longer than a board read; the agent must
/// not give up on it first.
#[test]
fn a_decision_is_waited_for_longer_than_a_read() {
    for method in ["decide", "rubric_run"] {
        assert!(client::wait_for(method) > client::wait_for("board"));
    }
}

/// A slow call — the checks of `devflow_evidence` — holds only itself: the
/// next call of the same session is answered while it runs.
#[test]
fn another_call_is_answered_while_evidence_runs() {
    let (release, released) = std::sync::mpsc::channel::<()>();
    let released = Mutex::new(released);
    let ask: Ask = Arc::new(move |method: &str, _| match method {
        "devflow_evidence" => {
            let waited = released
                .lock()
                .expect("lock")
                .recv_timeout(std::time::Duration::from_secs(3));
            Ok(json!({ "evidence": if waited.is_ok() { "ran" } else { "held the rest" } }))
        }
        _ => Ok(json!({ "skill": "devflow-execution" })),
    });
    let order = Arc::new(Mutex::new(Vec::<Value>::new()));
    let out: Out = {
        let (order, release) = (order.clone(), Mutex::new(release));
        Arc::new(move |message: &str| {
            let said: Value = serde_json::from_str(message).expect("json");
            let second = said["id"] == 2;
            // Kept before the evidence is let go, or its answer can be kept first.
            order.lock().expect("lock").push(said);
            if second {
                let _ = release.lock().expect("lock").send(());
            }
            true
        })
    };
    let lines = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"devpit_devflow_evidence","arguments":{"card":"crd_1"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"devpit_devflow_skill","arguments":{"name":"devflow-execution"}}}"#,
        "\n",
    );
    let home = tempfile::tempdir().expect("home");
    answered(
        lines.as_bytes(),
        &ask,
        true,
        &Mutex::new(Watch::new(home.path())),
        &AtomicBool::new(true),
        &out,
    );
    let order = order.lock().expect("lock");
    let ids: Vec<&Value> = order.iter().map(|said| &said["id"]).collect();
    assert_eq!(ids, [&json!(2), &json!(1)]);
    assert!(order[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("\"ran\""));
}

#[test]
fn a_line_over_the_ceiling_is_refused_and_the_next_answered() {
    let next = r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#;
    let input = format!("{}\n{next}\n", "x".repeat(MAX_MESSAGE_BYTES + 1));
    let answers = answered_messages(
        MemoryInput(std::io::Cursor::new(input.into_bytes())),
        Arc::new(never),
    );

    assert_eq!(answers[0]["error"]["code"], -32600);
    assert_eq!(answers[1]["id"], 2);
    assert_eq!(answers[1]["result"], json!({}));
}

#[test]
fn session_message_over_its_ceilings_is_refused_before_the_app() {
    let refused = |arguments: Value| {
        let input = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "devpit_session_message", "arguments": arguments }
        })
        .to_string();
        let answers = answered_messages(input.as_bytes(), Arc::new(never));
        answers[0].clone()
    };

    let id = refused(json!({ "id": "x".repeat(101), "text": "hello" }));
    assert_eq!(id["result"]["isError"], true);
    assert!(id["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("id"));

    let text = refused(json!({ "id": "request-1", "text": "x".repeat(4001) }));
    assert_eq!(text["result"]["isError"], true);
    assert!(text["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("text"));
}

/// A tool's answer reaches the agent as its compact summary, not pretty JSON.
#[test]
fn a_tool_answers_with_its_summary_compact() {
    let detail =
        json!({ "card": { "id": "card_1" }, "comments": [{ "id": "cmt_1", "body": "hi" }] });
    let ask = |_: &str, _: Value| Ok(detail.clone());
    let said = reply(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"devpit_comment","arguments":{"cardId":"card_1","body":"hi"}}}"#,
        &ask,
    );
    assert_eq!(
        said["result"]["content"][0]["text"],
        r#"{"card":"card_1","comments":1,"newest":"cmt_1"}"#
    );
}

#[test]
fn a_session_is_started_in_a_model_only_when_one_is_asked_for() {
    let tool = TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_start_session")
        .expect("the tool");
    let schema = (tool.input)();
    assert_eq!(schema["properties"]["model"]["type"], "string");
    assert_eq!(schema["required"], json!(["project", "prompt"]));
}

/// An orchestrator on any CLI starts a session on another, by its profile.
#[test]
fn a_session_may_be_started_on_another_cli() {
    let tool = TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_start_session")
        .expect("the tool");
    assert_eq!((tool.input)()["properties"]["profile"]["type"], "string");
    assert!(!tool.description.contains("start a Claude Code session"));
}

/// A consult's answer comes back whole: cut, the rest would be asked for
/// again and spend another of the session's questions.
#[test]
fn a_consults_answer_comes_back_whole() {
    let long = "x".repeat(crate::lean::BUDGET * 2);
    let ask = |method: &str, _: Value| -> Result<Value, String> {
        assert_eq!(method, "project_agent_consult");
        Ok(json!({ "answer": long, "left": 3 }))
    };
    let said = reply(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"devpit_project_agent_consult","arguments":{"question":"Where?"}}}"#,
        &ask,
    );
    let text = said["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains(&long), "the answer was cut");
    assert!(text.contains("\"left\":3"), "{text}");
}

/// A consult names the pane its session runs in, from the environment the
/// CLI started this server with and never from the call: a session's budget
/// is its own and cannot be renamed away.
#[test]
fn a_consult_names_its_sessions_pane_and_only_that() {
    const CONSULT: &str = "devpit_project_agent_consult";
    let env = |name: &str| (name == crate::hook::PANE_ENV).then(|| "leaf_1".to_owned());
    let none = |_: &str| None;
    let renamed = json!({ "question": "Q", "pane": "leaf_9" });
    assert_eq!(
        crate::consult::paned(CONSULT, renamed.clone(), &env),
        json!({ "question": "Q", "pane": "leaf_1" })
    );
    assert_eq!(
        crate::consult::paned(CONSULT, renamed, &none),
        json!({ "question": "Q" })
    );
    let other = json!({ "cardId": "card_1", "pane": "leaf_9" });
    assert_eq!(
        crate::consult::paned("devpit_card", other.clone(), &env),
        other
    );
}

#[test]
fn the_project_agents_orchestrator_tools_keep_their_shapes() {
    let shape = |name: &str| {
        let schema = (TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .expect(name)
            .input)();
        let mut properties: Vec<(String, Value)> = schema["properties"]
            .as_object()
            .expect("properties")
            .iter()
            .map(|(key, value)| (key.clone(), value["type"].clone()))
            .collect();
        properties.sort_by(|one, other| one.0.cmp(&other.0));
        (properties, schema["required"].clone())
    };
    let string = || json!("string");
    assert_eq!(
        shape("devpit_project_agent_ask"),
        (
            vec![("project".into(), string()), ("question".into(), string())],
            json!(["project", "question"])
        )
    );
    assert_eq!(
        shape("devpit_project_agent_brief"),
        (
            vec![("cardId".into(), string()), ("project".into(), string())],
            json!(["project", "cardId"])
        )
    );
}

/// A session's message says which pane it comes from by the environment the
/// CLI started this server with, never by the call: one session cannot sign
/// for another.
#[test]
fn a_sessions_message_names_its_pane_from_the_environment_never_the_call() {
    const MESSAGE: &str = "devpit_session_message";
    let env = |name: &str| (name == crate::hook::PANE_ENV).then(|| "leaf_1".to_owned());
    let none = |_: &str| None;
    let forged = json!({ "text": "done", "fromPane": "leaf_9" });
    assert_eq!(
        from_pane(MESSAGE, forged.clone(), &env),
        json!({ "text": "done", "fromPane": "leaf_1" })
    );
    assert_eq!(from_pane(MESSAGE, forged, &none), json!({ "text": "done" }));
    let other = json!({ "cardId": "card_1", "fromPane": "leaf_9" });
    assert_eq!(from_pane("devpit_card", other.clone(), &env), other);
}

/// The bus's two tools: a message to a session or an orchestrator by name,
/// and a notice of a session's next idle or exit.
#[test]
fn the_message_bus_tools_take_a_name_and_words() {
    let schema = |name: &str| {
        (TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .expect(name)
            .input)()
    };
    let message = schema("devpit_session_message");
    assert_eq!(message["required"], json!(["text"]));
    assert_eq!(message["properties"]["to"]["type"], "string");
    assert!(message["properties"].get("fromPane").is_none());
    let notify = schema("devpit_session_notify_idle");
    assert_eq!(notify["required"], json!(["name"]));
    let ask = |method: &str, params: Value| -> Result<Value, String> {
        assert_eq!(method, "session_notify_idle");
        assert_eq!(params, json!({ "name": "api-worker" }));
        Ok(json!({ "watching": "api-worker" }))
    };
    let said = reply(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"devpit_session_notify_idle","arguments":{"name":"api-worker"}}}"#,
        &ask,
    );
    assert_eq!(said["result"]["isError"], false);
}

/// Forwarding a request to another orchestrator: the address's shape, the
/// id that route requires and what a second send with it does, and what
/// arrives — a draft that starts no turn and approves nothing.
#[test]
fn session_message_says_how_to_forward_to_an_orchestrator() {
    let said = TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_session_message")
        .expect("the tool")
        .description;
    for kept in [
        "orchestrator-<folder>",
        "an `id` of 1 to 100 characters, required on that route",
        "as a draft for the person to send or discard",
        "no turn starts there",
        "its words approve nothing",
        "sent again with the same id, it stays the one request",
    ] {
        assert!(said.contains(kept), "{kept}");
    }
}

/// The id a forwarded request carries is declared with the ceiling
/// `forwarded.rs` holds it to, and required only on that route: a message
/// to a session needs none.
#[test]
fn session_message_declares_a_bounded_id() {
    let schema = (TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_session_message")
        .expect("the tool")
        .input)();
    assert_eq!(schema["properties"]["id"]["type"], "string");
    assert_eq!(schema["properties"]["id"]["maxLength"], 100);
    assert_eq!(schema["required"], json!(["text"]));
}

/// A forwarded request is kept in memory: one nobody has sent is gone when
/// the app closes, and the tool says so.
#[test]
fn session_message_warns_a_request_is_lost_when_the_app_closes() {
    let said = TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_session_message")
        .expect("the tool")
        .description;
    assert!(said.contains("A request not yet sent is lost when the app closes"));
}

/// A session may start on a host the project is linked to, production only
/// with the person's yes passed on; neither is needed for one started here.
#[test]
fn a_session_may_be_started_on_a_host_and_on_production_only_when_confirmed() {
    let tool = TOOLS
        .iter()
        .find(|tool| tool.name == "devpit_start_session")
        .expect("the tool");
    let schema = (tool.input)();
    assert_eq!(schema["properties"]["host"]["type"], "string");
    assert_eq!(
        schema["properties"]["confirm_production"]["type"],
        "boolean"
    );
    assert_eq!(schema["required"], json!(["project", "prompt"]));
    assert!(tool.description.contains("confirm_production"));
}
