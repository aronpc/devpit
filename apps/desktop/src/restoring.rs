//! The agent devpit started in a pane and the session it is in, and the
//! line that continues a session.
//!
//! A pane tmux or devpit-ptyd lost comes back as a shell and nothing more:
//! it comes back whenever its tab is drawn, and drawing one, as entering a
//! project does, starts no agent. A session is resumed when the person asks.

use devpit_agentcli::{Event, Happening};
use devpit_core::Store;

/// Where a session runs, from the agent's own hooks.
pub(crate) fn remember(store: &Store, pane: &str, happening: &Happening) {
    let _ = match &happening.event {
        Event::SessionStarted => store.remember_pane_session(
            pane,
            &happening.session_id,
            happening.transcript_path.as_deref(),
        ),
        // `/clear` ends a session and starts the next one in the same agent.
        Event::SessionEnded { reason } if reason.as_deref() != Some("clear") => {
            store.forget_pane_agent(pane)
        }
        _ => return,
    };
}

pub(crate) fn resume_flag_of(launch: &str) -> Option<&'static str> {
    let base = crate::projects::store()
        .ok()
        .and_then(|store| crate::agent_profiles::all(&store).ok())
        .and_then(|profiles| profiles.into_iter().find(|one| one.id == launch))
        .map(|profile| profile.base);
    devpit_pty::agents::known(base_or(base.as_deref(), launch)).and_then(|agent| agent.resume_flag)
}

/// The agent a profile behaves like: its base, or — for a discovered one,
/// whose base is empty — the profile itself, as `claude` is.
pub(crate) fn base_or<'a>(base: Option<&'a str>, launch: &'a str) -> &'a str {
    base.filter(|base| !base.is_empty()).unwrap_or(launch)
}

/// The launch line, continuing `session` when the agent can.
///
/// The id is typed into a shell, so anything but a session id's own
/// characters is refused rather than quoted: a UUID, or OpenCode's `ses_…`,
/// and never one that starts like an option of the CLI.
pub(crate) fn with_resume(start: &str, flag: Option<&str>, session: Option<&str>) -> String {
    let plain = |id: &&str| {
        id.len() <= 64
            && id.starts_with(|c: char| c.is_ascii_alphanumeric())
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    match (flag, session.filter(plain)) {
        (Some(flag), Some(id)) => format!("{start} {}", flag.replace("{}", id)),
        _ => start.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{base_or, with_resume};

    /// The account discovered as `claude` has no base of its own, and resumed nothing.
    #[test]
    fn a_discovered_profile_behaves_like_the_agent_it_is_named_for() {
        assert_eq!(base_or(Some(""), "claude"), "claude");
        assert_eq!(base_or(Some("claude"), "work"), "claude");
        assert_eq!(base_or(None, "claude"), "claude");
    }

    const START: &str = "claude --settings /s.json";

    #[test]
    fn a_known_session_is_continued() {
        assert_eq!(
            with_resume(
                START,
                Some("--resume {}"),
                Some("94e3457c-dcfa-4580-951a-dc9110a9580e")
            ),
            "claude --settings /s.json --resume 94e3457c-dcfa-4580-951a-dc9110a9580e"
        );
    }

    #[test]
    fn without_a_session_or_a_resume_flag_the_agent_starts_fresh() {
        assert_eq!(with_resume(START, Some("--resume {}"), None), START);
        assert_eq!(with_resume(START, None, Some("94e3457c")), START);
    }

    /// Each CLI continues its own session by id, the way its own help says.
    #[test]
    fn each_cli_continues_its_session_by_its_own_flag() {
        const UUID: &str = "94e3457c-dcfa-4580-951a-dc9110a9580e";
        let resumed = |agent: &str, id: &str| {
            let known = devpit_pty::agents::known(agent).expect("known");
            with_resume(known.launch, known.resume_flag, Some(id))
        };
        assert_eq!(resumed("claude", UUID), format!("claude --resume {UUID}"));
        assert_eq!(resumed("codex", UUID), format!("codex resume {UUID}"));
        assert_eq!(resumed("gemini", UUID), format!("gemini --resume {UUID}"));
        assert_eq!(
            resumed("opencode", "ses_e253a9d75ffejkRP4tKUxLPGEa"),
            "opencode --session ses_e253a9d75ffejkRP4tKUxLPGEa"
        );
        assert_eq!(
            resumed("cursor", UUID),
            format!("cursor-agent --resume {UUID}")
        );
    }

    /// The line is typed into a shell: an id that is not an id never reaches it.
    #[test]
    fn an_id_that_could_say_something_to_the_shell_is_refused() {
        for id in ["abc; rm -rf ~", "$(id)", "../x", "", "--force", "-x"] {
            assert_eq!(
                with_resume(START, Some("--resume {}"), Some(id)),
                START,
                "{id}"
            );
        }
    }
}
