//! Starting an agent again in a pane tmux or devpit-ptyd lost.
//!
//! Both keep a pane's processes while the app is closed, so most of the time
//! there is nothing to do. After a reboot, or a server that died, the layout
//! still names the pane and it comes back as a fresh shell. The agent
//! devpit had started there is typed in again, continuing its session when the
//! CLI can resume one.

use std::path::Path;

use devpit_agentcli::{Event, Happening};
use devpit_core::store::pane_agents::PaneAgent;
use devpit_core::Store;

use crate::shell_launch::{profile_id, to_start, Ready};

/// What restoring the pane later needs, from the agent's own hooks.
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

/// Types the agent back into a pane just made again for this leaf: a window
/// tmux lost, or a ptyd pane whose daemon died. `home` is the daemon's, for
/// a ptyd target.
///
/// On its own thread: a fresh shell takes a second or more to reach its
/// prompt, and the layout it belongs to is being opened while someone waits.
pub(crate) fn start_again(store: &Store, home: Option<&Path>, target: &str, leaf_id: &str) {
    let Ok(Some(agent)) = store.pane_agent(leaf_id) else {
        return;
    };
    let Ok(line) = resume_line(&agent) else {
        return;
    };
    let profile = profile_id(&agent.launch);
    let (home, target) = (home.map(Path::to_path_buf), target.to_owned());
    std::thread::spawn(move || type_again(home.as_deref(), &target, &line, &profile));
}

/// Types `line` at the pane once its shell settles, unless something else
/// already runs there. Answers whether it typed it.
pub(crate) fn type_again(home: Option<&Path>, target: &str, line: &str, profile: &str) -> bool {
    if let Ready::Busy(_) = crate::pane_io::settled(target) {
        return false;
    }
    let typed = crate::pane_io::send_line_at(home, target, line).is_ok();
    if typed {
        crate::pane_io::label(target, profile);
    }
    typed
}

fn resume_line(agent: &PaneAgent) -> Result<String, devpit_rpc::RpcError> {
    let start = to_start(&agent.launch)?;
    // The account it ran in, whatever its project says now: a session keeps
    // its account, and resuming elsewhere would find no conversation.
    let start = match agent
        .transcript_path
        .as_deref()
        .and_then(crate::claude_accounts::dir_of_transcript)
        .filter(|dir| crate::claude_accounts::is_account_dir(dir))
    {
        Some(dir) => crate::claude_accounts::in_dir(&start, &dir),
        None => start,
    };
    // Resuming a session that never wrote a line fails in the CLI; starting fresh does not.
    let written = agent
        .transcript_path
        .as_deref()
        .is_some_and(|path| std::fs::metadata(path).is_ok_and(|meta| meta.len() > 0));
    Ok(with_resume(
        &start,
        resume_flag_of(&agent.launch),
        agent.session_id.as_deref().filter(|_| written),
    ))
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
    use super::{base_or, type_again, with_resume};

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

    /// The resume line, typed into a ptyd shell the daemon started afresh,
    /// once the tap hears its shell in front, and never over an agent.
    #[cfg(unix)]
    #[test]
    fn an_agent_is_typed_again_into_a_fresh_ptyd_shell_and_never_over_one_that_runs() {
        use devpit_ptyd::frame::{self, Frame, Kind};

        let daemon = crate::ptyd_link::tests::serving();
        let home = daemon.home.path();
        let leaf = "leaf_typed_again";
        let pane = crate::ptyd_link::pane_number(leaf);
        // Prints what it is given instead of running it: the line is an agent's.
        let script = r#"printf 'ready\r\n'; read line; printf 'typed:%s\r\n' "$line"; sleep 30"#;
        let asked = devpit_ptyd::pane::Asked {
            rows: 24,
            cols: 80,
            cwd: home.to_string_lossy().into_owned(),
            argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
            env: Vec::new(),
        };
        crate::ptyd_link::spawn(home, pane, &asked).expect("spawn");
        let target = crate::ptyd_link::target(leaf);
        // What the tap is pushed once the pane goes quiet.
        let in_front = |program: &str| {
            let told = Frame::new(pane, Kind::Program, program.as_bytes().to_vec());
            let _ = crate::ptyd_tap::tap()
                .fronts
                .lock()
                .expect("fronts")
                .heard(&told);
        };
        let line = "claude --resume 94e3457c";

        in_front("claude");
        assert!(!type_again(Some(home), &target, line, "claude"));
        in_front("sh");
        assert!(type_again(Some(home), &target, line, "claude"));

        let mut watching = crate::ptyd_link::connect(home).expect("connect");
        frame::write(&mut watching, &Frame::new(pane, Kind::Attach, Vec::new())).expect("attach");
        watching
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("timeout");
        let wanted = format!("typed:{line}");
        let mut seen = Vec::new();
        while !String::from_utf8_lossy(&seen).contains(&wanted) {
            match frame::read(&mut watching) {
                Ok(Some(one)) if matches!(one.kind(), Some(Kind::Output | Kind::Replay)) => {
                    seen.extend(one.bytes)
                }
                Ok(Some(_)) => {}
                _ => break,
            }
        }
        crate::ptyd_link::kill(home, pane);
        assert!(
            String::from_utf8_lossy(&seen).contains(&wanted),
            "{}",
            String::from_utf8_lossy(&seen)
        );
    }
}
