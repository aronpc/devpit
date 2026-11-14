use super::{adopted, keeps, rows_in, Kept, Looked};
use crate::card_activity::Doing;

/// Codex's screen at its prompt, and in a turn, as 0.160 draws them.
const CODEX_AT_PROMPT: &str =
    "  done\n  Worked for 12s\n› Ask Codex to do anything\n  medium · ~/work";
const CODEX_IN_A_TURN: &str =
    "› Count to 60\n• Working (6s • esc to interrupt)\n› Ask Codex to do anything";

fn kept(profile: &str, name: &str, leaf: &str) -> Kept {
    Kept {
        profile: profile.to_owned(),
        name: name.to_owned(),
        project_id: "prj_api".to_owned(),
        project_name: Some("api".to_owned()),
        card_id: None,
        cwd: "/home/ana/work/api".to_owned(),
        pane_id: leaf.to_owned(),
    }
}

fn looked(screen: &str, hook: Option<Doing>) -> Option<Looked> {
    Some(Looked {
        agent: true,
        screen: Some(screen.to_owned()),
        hook,
    })
}

/// A session on a CLI with no registry is listed by the name devpit gave it,
/// busy or idle as its pane shows, at the pane devpit typed it into.
#[test]
fn a_session_on_another_cli_is_listed_by_its_pane() {
    let mut held = vec![kept("codex", "api-worker", "leaf_api")];
    let rows = rows_in(&mut held, "codex", |_| looked(CODEX_IN_A_TURN, None), 0.0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "api-worker");
    assert_eq!(rows[0].status, "busy");
    assert_eq!(
        rows[0].pane.as_ref().map(|pane| pane.pane_id.as_str()),
        Some("leaf_api")
    );
    let rows = rows_in(&mut held, "codex", |_| looked(CODEX_AT_PROMPT, None), 0.0);
    assert_eq!(rows[0].status, "idle");
    // A hook speaks for the agent over its screen.
    let rows = rows_in(
        &mut held,
        "codex",
        |_| looked(CODEX_AT_PROMPT, Some(Doing::Working)),
        0.0,
    );
    assert_eq!(rows[0].status, "busy");
}

/// Once its pane is gone or its agent left it, the session has ended and is
/// let go; another orchestrator's are neither shown nor let go.
#[test]
fn a_session_whose_agent_left_its_pane_is_let_go() {
    let mut held = vec![
        kept("codex", "api-worker", "leaf_api"),
        kept("claude", "theirs", "leaf_other"),
    ];
    let shell = |_: &Kept| {
        Some(Looked {
            agent: false,
            screen: Some("$ ".to_owned()),
            hook: None,
        })
    };
    assert!(rows_in(&mut held, "codex", shell, 0.0).is_empty());
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].name, "theirs");
    assert!(rows_in(&mut held, "claude", |_| None, 0.0).is_empty());
    assert!(held.is_empty());
}

/// Only a Claude Code orchestrator's Claude Code sessions are listed by
/// Claude Code's registry; devpit keeps every other pairing itself.
#[test]
fn devpit_keeps_what_no_registry_of_the_orchestrators_lists() {
    assert!(!keeps(true, true));
    assert!(keeps(true, false));
    assert!(keeps(false, true));
    assert!(keeps(false, false));
}

/// A switch of profile keeps the sessions the orchestrator reached in a devpit
/// pane: listed under the profile it speaks as now, by the same names, rather
/// than announced as ended. One outside devpit's panes cannot be reached so.
#[test]
fn a_session_running_before_a_switch_is_still_listed_after_it() {
    let row = |name: &str, pane: Option<&str>| {
        let mut one = rows_in(
            &mut vec![kept("claude", name, pane.unwrap_or("leaf_x"))],
            "claude",
            |_| looked(CODEX_IN_A_TURN, None),
            0.0,
        )
        .remove(0);
        if pane.is_none() {
            one.pane = None;
        }
        one
    };
    let mut held = adopted(
        "opencode",
        vec![row("api-worker", Some("leaf_api")), row("outside", None)],
    );
    assert_eq!(held.len(), 1);
    let rows = rows_in(
        &mut held,
        "opencode",
        |_| looked(CODEX_AT_PROMPT, None),
        0.0,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "api-worker");
    assert_eq!(
        rows[0].pane.as_ref().map(|pane| pane.pane_id.as_str()),
        Some("leaf_api")
    );
}
