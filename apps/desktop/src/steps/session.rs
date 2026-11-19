//! The session step: a detached session for the card, attached on request.
use devpit_core::Store;
use devpit_rpc::Step;
use serde::Deserialize;

use super::Finished;

/// What a `session` step needs to know, out of `step.config`.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SessionConfig {
    model: Option<String>,
    /// Which profile starts it. The session outlives this step and is attached
    /// later, so the profile is written down with the link rather than looked
    /// up again from a step that may have changed by then.
    profile: Option<String>,
}

/// Reads a session step's config, or says why it cannot be read.
///
/// Shared with the rule that refuses a step when it is saved: what runs it and
/// what accepts it read the same way, so a step that saves is a step that
/// starts.
pub(crate) fn readable(config: &str) -> Result<SessionConfig, String> {
    serde_json::from_str(config).map_err(|err| format!("this step's config is not readable: {err}"))
}

/// Starts the card's session in the card's terminal tab.
///
/// In a terminal of the project, where the person watches it and answers it,
/// and started by the profile's own command line — the same mode their own
/// sessions run in. It used to start in the background, under the CLI's own
/// supervisor: nothing showed it, and the first permission it asked for was a
/// question nobody could see.
///
/// `session_id` is new for this run and chosen before it starts: it names the
/// transcript, and the transcript is where a session reports what it spent.
pub fn start(
    app: &tauri::AppHandle,
    store: &Store,
    card_id: &str,
    step: &Step,
    session_id: &str,
) -> Result<Finished, String> {
    let config = readable(&step.config)?;

    // The card's own checkout, created here if this is the first step that
    // needs one. The CLI's `--worktree` is deliberately not used: it puts the
    // checkout inside the repository, where it shows up in the file tree, in
    // ripgrep, and one day in a commit.
    let cwd = crate::checkout::cwd_for(store, card_id, step, |_| {})?;
    let project_id = store
        .project_id_of_card(card_id)
        .map_err(|err| err.to_string())?
        .ok_or("that card is on no project")?;
    let title = store
        .card(card_id)
        .map_err(|err| err.to_string())?
        .map(|card| card.title)
        .unwrap_or_default();

    let profile = config
        .profile
        .clone()
        .unwrap_or_else(|| crate::agent_choice::default_id(store));
    let line = launch_line(
        &crate::shell_launch::to_start_in(
            crate::session_locale::SessionLocale::capture(),
            &profile,
            Some(&project_id),
        )
        .map_err(|err| err.message)?,
        &crate::handing::session_name(&title, card_id),
        session_id,
        config.model.as_deref(),
    );

    let card_tab = crate::sessions::tab_for_card(card_id);
    let taken = store
        .pane_layout(&project_id, &card_tab)
        .map_err(|err| err.to_string())?
        .is_some();
    let tab_id = crate::handing::tab_to_open(card_tab, taken);
    let pane_id = crate::opening::typed_in(app, &project_id, &tab_id, &cwd, &line, Some(&profile))
        .map_err(|err| err.message)?;
    let _ = tauri::Emitter::emit(
        app,
        crate::opening::TAB_OPENED,
        serde_json::json!({ "projectId": project_id, "tabId": tab_id, "paneId": pane_id }),
    );

    Ok(Finished {
        ok: true,
        output: "the session is running in the card's terminal tab".to_owned(),
        cost_usd: 0.0,
        duration_ms: 0,
        exit_code: None,
    })
}

/// The profile's command, named and given its session id and model.
pub(crate) fn launch_line(
    start: &str,
    name: &str,
    session_id: &str,
    model: Option<&str>,
) -> String {
    use devpit_agentcli::declaring::quoted;
    let mut line = format!(
        "{start} --name {} --session-id {}",
        quoted(name),
        quoted(session_id)
    );
    if let Some(model) = model.filter(|one| !one.trim().is_empty()) {
        line.push_str(&format!(" --model {}", quoted(model)));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::launch_line;

    #[test]
    fn the_step_starts_the_profile_named_with_its_session_and_model() {
        assert_eq!(
            launch_line("claude --dangerously-skip-permissions", "fix-login-ab12", "s-1", Some("opus")),
            "claude --dangerously-skip-permissions --name 'fix-login-ab12' --session-id 's-1' --model 'opus'"
        );
        assert_eq!(
            launch_line("claude", "x", "s-1", Some(" ")),
            "claude --name 'x' --session-id 's-1'"
        );
    }
}
