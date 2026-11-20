use std::collections::HashMap;

use super::*;

fn pane(profile: &str) -> devpit_tmux::Running {
    devpit_tmux::Running {
        leaf_id: "w1".to_owned(),
        command: "node".to_owned(),
        tty: "/dev/pts/7".to_owned(),
        profile: profile.to_owned(),
    }
}

fn names(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(id, label)| ((*id).to_owned(), (*label).to_owned()))
        .collect()
}

/*
 * What a pane is called.
 *
 * Two sources, and which wins is the point. Reading the process answers for
 * every terminal, including the ones devpit never touched, and it cannot tell
 * `glm` from `claude2` — same binary, same arguments, different environment.
 * The record answers only for panes devpit started, and for those it is exact.
 */

#[test]
fn a_pane_devpit_started_is_called_what_the_person_called_it() {
    let said = named(&[], &names(&[("01JGLM", "GLM")]), pane("01JGLM"));
    assert_eq!(said.label, "GLM");
}

#[test]
fn a_pane_devpit_did_not_start_falls_back_to_reading_the_process() {
    // No record, so the answer is whatever the process looks like — which is
    // what every row did before profiles existed.
    let said = named(&[], &names(&[("01JGLM", "GLM")]), pane(""));
    assert_eq!(said.label, "node");
}

#[test]
fn a_record_pointing_at_a_profile_that_is_gone_falls_back_too() {
    // Deleting a profile is refused while a step uses it, but nothing stops
    // somebody deleting one with a terminal still open on it.
    let said = named(&[], &names(&[]), pane("01JDELETED"));
    assert_eq!(said.label, "node");
}

#[test]
fn the_record_does_not_make_an_idle_shell_look_busy() {
    // A profile's terminal that has been quit back to a prompt is idle, and
    // the record outlives the agent that was in it.
    let mut quiet = pane("01JGLM");
    quiet.command = "zsh".to_owned();
    let said = named(&[], &names(&[("01JGLM", "GLM")]), quiet);
    assert!(!said.busy);
}

#[test]
fn only_a_bare_name_is_asked_of_the_shell() {
    // The probe pastes these into a line of shell, so anything needing
    // quoting is left out rather than escaped.
    assert!(is_a_bare_name("claude"));
    assert!(is_a_bare_name("cursor-agent"));
    assert!(is_a_bare_name("python3.11"));
    assert!(!is_a_bare_name("rm -rf /"));
    assert!(!is_a_bare_name("a;b"));
    assert!(!is_a_bare_name(""));
}

#[test]
fn the_program_of_a_launch_line_is_its_first_word() {
    assert_eq!(program_of("cursor-agent"), "cursor-agent");
    assert_eq!(program_of("orca claude-teams"), "orca");
}

fn known_agent(id: &str, label: &str) -> KnownAgent {
    KnownAgent {
        id: id.to_owned(),
        label: label.to_owned(),
        launch: id.to_owned(),
        installed: true,
        enabled: true,
        homepage: String::new(),
    }
}

#[test]
fn an_override_replaces_the_built_in_it_was_opened_from() {
    // One id, one row: a menu listing `claude` twice starts the override from
    // one of them and the bare CLI from the other.
    let all = joined(
        vec![known_agent("claude", "Claude Code (mine)")],
        vec![
            known_agent("claude", "Claude Code"),
            known_agent("codex", "Codex"),
        ]
        .into_iter(),
    );
    let ids: Vec<&str> = all.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["claude", "codex"]);
    assert_eq!(all[0].label, "Claude Code (mine)");
}

/// Every line devpit types to start an agent carries the session's language
/// in front, as the assignment a profile's own variables already are: the
/// agent's process is born with it, and the MCP server it starts inherits it.
#[test]
fn a_launch_line_carries_the_sessions_language_in_front() {
    let locale = crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::PtBr);
    let line = launch_line(locale, "claude", "claude", None, None, None);
    assert!(line.starts_with("DEVPIT_LOCALE='pt-BR' claude"), "{line}");
}

/// A profile that sets `DEVPIT_LOCALE` itself does not choose the session's
/// language: the name is devpit's, and the line carries the one captured at
/// the session's birth and no other.
#[test]
fn a_profiles_own_locale_never_speaks_over_the_sessions() {
    let mut found = devpit_rpc::Profile {
        id: "01JGLM".to_owned(),
        label: "GLM".to_owned(),
        command: "claude".to_owned(),
        driver: "claude".to_owned(),
        path: None,
        reach: devpit_rpc::Reach::Runnable,
        base: "claude".to_owned(),
        args: Vec::new(),
        env: Vec::new(),
        mine: true,
        models: Vec::new(),
        own_models: Vec::new(),
        offered: Vec::new(),
        efforts: Vec::new(),
        effort_default: None,
        metered: true,
        enabled: true,
    };
    found.env.push(devpit_rpc::EnvVar {
        name: "DEVPIT_LOCALE".to_owned(),
        value: "en".to_owned(),
    });
    found.env.push(devpit_rpc::EnvVar {
        name: "ANTHROPIC_BASE_URL".to_owned(),
        value: "https://example.test".to_owned(),
    });
    let said = devpit_agentcli::running::line(&devpit_agentcli::running::runner(&found));
    let locale = crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::PtBr);
    let line = launch_line(locale, &said, "claude", None, None, None);
    assert!(line.contains("DEVPIT_LOCALE='pt-BR'"), "{line}");
    assert!(!line.contains("DEVPIT_LOCALE='en'"), "{line}");
    assert!(
        line.contains("ANTHROPIC_BASE_URL='https://example.test'"),
        "{line}"
    );
}
