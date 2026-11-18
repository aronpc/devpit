//! A turn of conversation, read through a driver and reported as it arrives.
//!
//! `headless::run_turn` answers once, at the end — right for a board step,
//! wrong for a chat, where the point is watching it happen.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;

use devpit_rpc::{Part, SessionInit, TurnEnd};

use crate::driver::Driver;
use crate::AgentError;

/// What a turn needs to start.
pub struct Say<'a> {
    /// The binary this conversation's profile names. Two accounts of the
    /// same CLI differ here and nowhere else.
    pub command: &'a str,
    /// The profile's environment. It is what says whose account this turn
    /// spends: a terminal carries it as shell assignments in the line it
    /// types, and a spawned turn is handed it here or spends the default one
    /// without saying so.
    pub env: &'a [(String, String)],
    pub prompt: &'a str,
    pub cwd: &'a Path,
    pub model: Option<&'a str>,
    pub budget_usd: Option<f64>,
    /// Carried forward so the CLI continues the same session.
    pub session_id: Option<&'a str>,
    /// A message in that session to fork at instead of resuming its end.
    pub fork_at: Option<&'a str>,
    /// What the agent may do without asking. The CLI's own word for it.
    pub permission: Option<&'a str>,
    /// Tools and commands refused whatever the profile's settings allow.
    pub disallowed: &'a [&'a str],
    /// devpit's hook settings. A supervised turn asks through the
    /// `PreToolUse` hook, and a turn without it has nobody to ask: the CLI,
    /// printing rather than talking, refuses every write instead.
    pub settings: Option<&'a str>,
    /// The CLI's own settings it loads (`user`, `project`, `local`); `None`, all.
    pub setting_sources: Option<&'a str>,
    /// devpit's MCP file, so a chat has the board's tools the way a terminal
    /// does, without depending on a plugin being installed.
    pub mcp_config: Option<&'a str>,
    /// How hard to think. One of what the driver's `efforts` lists.
    pub effort: Option<&'a str>,
    /// Folders beyond `cwd` the agent may read and work in: every project,
    /// for an orchestrator.
    pub add_dirs: &'a [String],
    /// The name other sessions message it by. Left to the CLI, it is new
    /// with every process, and a reply sent to the old one is lost.
    pub name: Option<&'a str>,
    /// Where the turn's stdin is kept while it runs, for control requests.
    pub control: Option<&'a crate::control::Control>,
    /// Told the CLI's session id as soon as the stream names it, while the
    /// turn is still running. A new conversation has no id until then, and
    /// whatever is keyed by it — being asked before a tool runs — would
    /// otherwise start only on the next turn.
    pub on_session: Option<&'a (dyn Fn(&str) + Sync)>,
}

/// A turn, and the thread it belongs to on the CLI's side.
pub struct Said {
    pub end: TurnEnd,
    /// The CLI's id for this conversation, to resume it next turn.
    pub session_id: Option<String>,
    /// The last self-description the CLI printed. A background completion can
    /// wake it for a second pass, which prints another.
    pub init: Option<SessionInit>,
    /// The last message the agent wrote, where a rewind to this turn forks.
    pub anchor: Option<String>,
}

/// A turn's CLI leads a process group of its own: a forced stop ends the
/// tools it started with it, which signalling the CLI alone would leave running.
pub(crate) fn own_group(command: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    command
}

/// Runs one turn, handing every part to `on_part` as it is read. `on_start`
/// gets the child's pid, so the turn can be stopped from another thread.
pub fn say(
    driver: &dyn Driver,
    turn: &Say<'_>,
    mut on_part: impl FnMut(Part),
    mut on_start: impl FnMut(u32),
) -> Result<Said, AgentError> {
    let wire = driver.wire();
    let argv = wire.argv(turn);
    let mut child = own_group(&mut devpit_pty::host_env::command(turn.command))
        .args(&argv)
        .current_dir(turn.cwd)
        .envs(turn.env.iter().map(|(key, value)| (key, value)))
        .envs(crate::apps_host::chat_switches(turn.env))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| AgentError::NotInstalled)?;

    on_start(child.id());
    let stderr = crate::talk_failure::read_apart(&mut child);

    let stdin = child.stdin.take().ok_or(AgentError::NotInstalled)?;
    let control = turn.control.cloned().unwrap_or_default();
    if !control.open(stdin, wire, turn, child.id()) || !control.say(turn.prompt) {
        return Err(AgentError::Unreadable(
            "could not send the prompt".to_owned(),
        ));
    }
    let stdout = child.stdout.take().ok_or(AgentError::NotInstalled)?;
    let started = std::time::Instant::now();
    let heard = crate::talk_stream::follow(
        driver,
        BufReader::new(stdout).lines().map_while(Result::ok),
        &control,
        turn.on_session,
        &mut on_part,
    );

    // Whatever ended the stream, stdin is not needed any more.
    control.close();
    let status = child
        .wait()
        .map_err(|err| AgentError::Unreadable(err.to_string()))?;

    let ending = crate::talk_failure::ending(heard.ended, status.success(), stderr, &mut on_part);

    Ok(Said {
        end: TurnEnd {
            turn_id: String::new(),
            cost_usd: ending.cost_usd,
            duration_ms: Some(started.elapsed().as_millis() as f64),
            stop_reason: ending.stop_reason,
            is_error: ending.is_error,
            context: ending.context,
            limit: heard.limit,
            ..Default::default()
        },
        session_id: heard.session_id,
        init: heard.init,
        anchor: heard.anchor,
    })
}
