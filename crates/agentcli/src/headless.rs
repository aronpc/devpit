//! One headless turn: run it, read the stream, report what it cost.
//!
//! This is the cheap half of the product. It takes no terminal, it declares a
//! spending cap before it starts, and the last line of its output says what it
//! actually spent — which is how a card stops saying "the agent is doing
//! something" and starts saying a number.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Stdio;

use crate::headless_stream::read_stream;
use crate::{headless_argv, AgentError, PROGRAM};

/// What one turn produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The final text, which is JSON when a schema was asked for.
    pub result: String,
    pub cost_usd: f64,
    pub duration_ms: i64,
    pub turns: i64,
    /// True when the CLI itself reported failure, budget included.
    pub is_error: bool,
    /// Why it ended: the CLI's own word, kept rather than mapped so a new one
    /// arrives intact instead of being flattened into "failed".
    pub stop_reason: Option<String>,
    /// The session the CLI says it ran, from its `init` line.
    pub session_id: Option<String>,
    /// The model the CLI says it used, from the same `init` line. Empty when
    /// the line or the field is missing; what was asked for is kept apart.
    pub model: Option<String>,
}

/// How a turn is configured.
pub struct Turn<'a> {
    pub prompt: &'a str,
    pub cwd: &'a Path,
    /// The `--agents` argument, when the step names an agent.
    pub agents: Option<&'a str>,
    /// A JSON Schema the answer has to satisfy.
    pub schema: Option<&'a str>,
    pub budget_usd: Option<f64>,
    pub model: Option<&'a str>,
    /// A settings file for this turn — the hooks that report what it is doing.
    pub settings: Option<&'a str>,
    /// The session this turn speaks in: new for every run, so two runs of one
    /// card are two conversations.
    pub session_id: Option<&'a str>,
    /// Context the step declared, as environment variables.
    ///
    /// Variables and never interpolation: a branch named `fix;rm -rf /` has to
    /// become a value, not shell syntax.
    pub env: &'a [(String, String)],
    /// Which profile runs it — the program, the arguments it always carries,
    /// and the environment that picks the account.
    ///
    /// `None` runs whatever `PROGRAM` names, which is what every turn did
    /// before profiles existed and what a board with no profile chosen still
    /// does.
    pub runner: Option<&'a crate::running::Runner>,
}

/// Runs the turn and reports the outcome.
///
/// `on_partial` is called with each assistant fragment as it arrives, so a
/// card can show work in progress rather than a spinner. A turn that produces
/// no `result` line is a failure with the stderr attached: it means the CLI
/// died rather than answered.
pub fn run_turn(turn: &Turn<'_>, on_partial: impl FnMut(&str)) -> Result<Outcome, AgentError> {
    run_turn_cancellable(turn, on_partial, |_| {})
}

/// The same, handing the caller a way to stop it.
///
/// `on_start` is given the process id the moment the turn begins. Without it a
/// turn that hangs is a run stuck at `running` for the life of the app, and a
/// card stuck behind it — a step you cannot stop is a step you learn not to
/// start.
pub fn run_turn_cancellable(
    turn: &Turn<'_>,
    on_partial: impl FnMut(&str),
    mut on_start: impl FnMut(u32),
) -> Result<Outcome, AgentError> {
    let argv = headless_argv(
        turn.agents,
        turn.schema,
        turn.budget_usd,
        turn.model,
        turn.settings,
        turn.session_id,
    );
    let mut child =
        devpit_pty::host_env::command(turn.runner.map_or(PROGRAM, |one| one.program.as_str()))
            .args(
                turn.runner
                    .map_or_else(|| argv[1..].to_vec(), |one| one.argv(&argv[1..])),
            )
            .current_dir(turn.cwd)
            // The profile first, then the step's own: the profile says which
            // account this runs under, and the step's context is about this one
            // turn. A collision is the turn's to win.
            .envs(
                turn.runner
                    .map(|one| one.env.clone())
                    .unwrap_or_default()
                    .iter()
                    .chain(turn.env.iter())
                    .map(|(key, value)| (key, value)),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| AgentError::NotInstalled)?;

    on_start(child.id());

    {
        let mut stdin = child.stdin.take().ok_or(AgentError::NotInstalled)?;
        let message = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": [{ "type": "text", "text": turn.prompt }] }
        });
        writeln!(stdin, "{message}").map_err(|err| AgentError::Unreadable(err.to_string()))?;
    }

    let stdout = child.stdout.take().ok_or(AgentError::NotInstalled)?;
    let outcome = read_stream(
        BufReader::new(stdout).lines().map_while(Result::ok),
        on_partial,
    );

    let status = child
        .wait()
        .map_err(|err| AgentError::Unreadable(err.to_string()))?;

    outcome.ok_or_else(|| AgentError::Failed {
        command: "headless turn".to_owned(),
        stderr: format!("the run ended with {status} before reporting a result"),
    })
}

#[cfg(test)]
#[path = "headless_tests.rs"]
mod tests;
