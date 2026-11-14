use serde_json::{json, Value};

use super::*;
use crate::devflow::{Offer, Watch, SETUP_TOOLS, STAGED};
use crate::mcp::{handle_offering, TOOLS};

fn never(_: &str, _: Value) -> Result<Value, String> {
    panic!("the app should not have been asked")
}

fn tools(offer: &Offer, seat: &Seat) -> Vec<Value> {
    let said: Value = serde_json::from_str(
        &handle_offering(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            &never,
            true,
            offer,
            seat,
        )
        .expect("an answer"),
    )
    .expect("json");
    said["result"]["tools"].as_array().expect("tools").clone()
}

fn names(listed: &[Value]) -> Vec<&str> {
    listed
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default())
        .collect()
}

const PROJECT: Seat = Seat {
    orchestrator: Some(false),
    decisions: Some(false),
    assistant: None,
};

#[test]
fn every_tool_named_here_is_one_devpit_has() {
    let every: Vec<&str> = TOOLS
        .iter()
        .chain(&SETUP_TOOLS)
        .chain(STAGED.iter().map(|(_, tool)| tool))
        .chain(&crate::assistant::TOOLS)
        .chain(&crate::consult::TOOLS)
        .map(|tool| tool.name)
        .collect();
    for name in ORCHESTRATOR_ONLY
        .iter()
        .chain(&DECISIONS)
        .chain(&READ_ONLY)
        .chain(&DESTRUCTIVE)
    {
        assert!(every.contains(name), "{name}");
    }
}

/// A host approves a read without asking, and asks before what it cannot undo.
#[test]
fn a_read_and_a_loss_say_so_to_the_host() {
    let listed = tools(&Offer::Active(vec!["plan".into()]), &Seat::default());
    let hint = |name: &str| {
        listed
            .iter()
            .find(|tool| tool["name"] == name)
            .map(|tool| tool["annotations"].clone())
            .expect(name)
    };
    assert_eq!(hint("devpit_board"), json!({ "readOnlyHint": true }));
    assert_eq!(
        hint("devpit_devflow_skill"),
        json!({ "readOnlyHint": true })
    );
    assert_eq!(
        hint("devpit_stop_session"),
        json!({ "destructiveHint": true })
    );
    assert_eq!(hint("devpit_comment"), Value::Null);
}

/// An app that does not say who the session is gets the list it always had.
#[test]
fn an_app_too_old_to_say_lists_everything() {
    let all = tools(&Offer::Setup, &Seat::default());
    let on_demand = crate::devflow::ON_DEMAND.len();
    assert_eq!(all.len(), TOOLS.len() + SETUP_TOOLS.len() + on_demand);
    let board = all
        .iter()
        .find(|tool| tool["name"] == "devpit_board")
        .expect("the board");
    assert!(board["inputSchema"]["properties"]["project"].is_object());
}

#[test]
fn a_project_session_is_offered_neither_the_orchestrators_tools_nor_another_board() {
    let listed = tools(&Offer::Setup, &PROJECT);
    let names = names(&listed);
    for name in ORCHESTRATOR_ONLY.iter().chain(&DECISIONS) {
        assert!(!names.contains(name), "{name} was listed");
    }
    for name in ["devpit_board", "devpit_card", "devpit_devflow_status"] {
        assert!(names.contains(&name), "{name} was not listed");
    }
    for tool in &listed {
        assert!(
            tool["inputSchema"]["properties"].get("project").is_none(),
            "{} still takes another project",
            tool["name"]
        );
    }
}

#[test]
fn an_orchestrator_is_offered_its_own_tools_and_another_projects_board() {
    let seat = Seat {
        orchestrator: Some(true),
        decisions: Some(true),
        assistant: None,
    };
    // The app answers `off` for an orchestrator: its folder is notes.
    let listed = tools(&Offer::Off, &seat);
    let names = names(&listed);
    for name in ORCHESTRATOR_ONLY.iter().chain(&DECISIONS) {
        assert!(names.contains(name), "{name} was not listed");
    }
    // Of Devflow, only what passes the person's yes on to a linked project.
    let devflow: Vec<&&str> = names
        .iter()
        .filter(|name| name.starts_with("devpit_devflow"))
        .collect();
    assert_eq!(devflow, [&"devpit_devflow_plan"]);
    let board = listed
        .iter()
        .find(|tool| tool["name"] == "devpit_board")
        .expect("the board");
    assert!(board["inputSchema"]["properties"]["project"].is_object());
}

#[test]
fn an_active_project_lists_its_stages_but_not_what_it_cannot_use() {
    let listed = tools(&Offer::Active(vec!["plan".into()]), &PROJECT);
    let active = names(&listed);
    assert!(active.contains(&"devpit_devflow_plan"));
    assert!(!active.contains(&"devpit_start_session"));
    assert!(!active.contains(&"devpit_decide"));
    let with_key = Seat {
        decisions: Some(true),
        ..PROJECT
    };
    let keyed = tools(&Offer::Off, &with_key);
    assert!(names(&keyed).contains(&"devpit_decide"));
}

/// Off the list is not out of reach: a call by name still asks the app,
/// which refuses or answers as it always did.
#[test]
fn a_tool_off_the_list_is_still_answered_by_name() {
    let ask = |method: &str, _: Value| -> Result<Value, String> {
        assert_eq!(method, "sessions");
        Err("only an orchestrator sees past its own project".into())
    };
    let said: Value = serde_json::from_str(
        &handle_offering(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"devpit_sessions"}}"#,
            &ask,
            true,
            &Offer::Off,
            &PROJECT,
        )
        .expect("an answer"),
    )
    .expect("json");
    assert_eq!(said["result"]["isError"], true);
    assert!(said["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("orchestrator"));
}

#[test]
fn a_key_kept_or_forgotten_tells_the_client_to_list_again() {
    let root = tempfile::tempdir().expect("root");
    let mut watch = Watch::new(root.path());
    let looked = watch.due().expect("due");
    let without = json!({ "offer": "setup", "orchestrator": false, "decisions": false });
    assert!(watch.settle(looked.clone(), Ok(without.clone())).is_some());
    assert_eq!(watch.seat, PROJECT);
    assert!(watch.settle(looked.clone(), Ok(without)).is_none());
    let with = json!({ "offer": "setup", "orchestrator": false, "decisions": true });
    assert!(watch.settle(looked, Ok(with)).is_some());
    assert_eq!(watch.seat.decisions, Some(true));
}

/// The project agent answers the orchestrator: a project's session reads its
/// own project and is not offered it.
#[test]
fn the_project_agent_is_offered_to_the_orchestrator_alone() {
    let orchestrator = Seat {
        orchestrator: Some(true),
        decisions: Some(false),
        assistant: None,
    };
    for name in ["devpit_project_agent_ask", "devpit_project_agent_brief"] {
        let tool = TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("{name}"));
        assert!(orchestrator.offers(tool), "{name}");
        assert!(!PROJECT.offers(tool), "{name}");
        assert!(tool.description.contains("project agent"), "{name}");
    }
}

/// Assistant mode adds its tools to the orchestrator's list, and only it: with
/// the mode off, or an app too old to say, the list is the one it always was.
#[test]
fn the_assistants_tools_come_with_assistant_mode_alone() {
    let orchestrator = |assistant| Seat {
        orchestrator: Some(true),
        decisions: Some(false),
        assistant,
    };
    let before = names(&tools(&Offer::Off, &orchestrator(None))).join(",");
    let off = names(&tools(&Offer::Off, &orchestrator(Some(false)))).join(",");
    assert_eq!(off, before, "the list without the mode changed");
    let on = tools(&Offer::Off, &orchestrator(Some(true)));
    let on = names(&on);
    for tool in &crate::assistant::TOOLS {
        assert!(on.contains(&tool.name), "{} was not listed", tool.name);
        assert!(
            !before.split(',').any(|name| name == tool.name),
            "{}",
            tool.name
        );
    }
    assert_eq!(
        on.len(),
        before.split(',').count() + crate::assistant::TOOLS.len()
    );
}

/// The person turning the mode on or off changes the list the server offers,
/// and the client is told so.
#[test]
fn turning_assistant_mode_on_or_off_changes_the_list() {
    let root = tempfile::tempdir().expect("tmp");
    let mut watch = Watch::new(root.path());
    let looked = watch.due().expect("due");
    let off = json!({ "offer": "off", "orchestrator": true, "assistant": false });
    assert!(watch.settle(looked.clone(), Ok(off.clone())).is_some());
    assert!(watch.settle(looked.clone(), Ok(off)).is_none());
    let on = json!({ "offer": "off", "orchestrator": true, "assistant": true });
    assert!(watch.settle(looked, Ok(on)).is_some());
    assert_eq!(watch.seat.assistant, Some(true));
}

/// A project's own session is offered its project agent, to ask and to
/// propose, without `project`: its own is the only one it reaches. The
/// orchestrator keeps its own pair, and an app too old to say offers neither.
#[test]
fn a_projects_session_is_offered_its_own_project_agent_and_no_one_else_is() {
    const SESSION_TOOLS: [&str; 2] = [
        "devpit_project_agent_consult",
        "devpit_project_agent_propose",
    ];
    let orchestrator = Seat {
        orchestrator: Some(true),
        decisions: Some(false),
        assistant: None,
    };
    let session = tools(&Offer::Off, &PROJECT);
    for name in SESSION_TOOLS {
        let tool = session
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("{name} was not offered to a project's session"));
        assert!(
            tool["inputSchema"]["properties"].get("project").is_none(),
            "{name}"
        );
        for seat in [&orchestrator, &Seat::default()] {
            assert!(
                !names(&tools(&Offer::Off, seat)).contains(&name),
                "{name}: {seat:?}"
            );
        }
    }
}

/// A session of any project answers its orchestrator with the bus; only an
/// orchestrator is woken by a session going idle.
#[test]
fn a_session_may_message_and_only_an_orchestrator_hears_of_idle() {
    let listed = tools(&Offer::Off, &PROJECT);
    let session = names(&listed);
    assert!(session.contains(&"devpit_session_message"));
    assert!(!session.contains(&"devpit_session_notify_idle"));
    let orchestrator = Seat {
        orchestrator: Some(true),
        decisions: Some(false),
        assistant: None,
    };
    let listed = tools(&Offer::Off, &orchestrator);
    let orchestrating = names(&listed);
    assert!(orchestrating.contains(&"devpit_session_message"));
    assert!(orchestrating.contains(&"devpit_session_notify_idle"));
}
