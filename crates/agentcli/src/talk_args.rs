//! The command line a chat turn starts with.
//!
//! Apart from `talk.rs` so the flags are tested rather than trusted: the CLI
//! rejects an unknown one before doing any work, and a turn that fails on its
//! first line looks, from the window, like an agent that never answered.

use crate::talk::Say;

/// The arguments a chat turn is started with.
///
/// Its own function so the flags are tested rather than trusted: this one used
/// to pass `--max-cost`, which the CLI rejects with "unknown option" before
/// doing anything — every capped chat turn failed on its first line.
pub(crate) fn argv(turn: &Say<'_>) -> Vec<String> {
    let mut argv = vec![
        "--print".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--input-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        // Text as it is written, not a block at a time.
        "--include-partial-messages".to_owned(),
        // A steer is taken in once the CLI echoes it back.
        "--replay-user-messages".to_owned(),
    ];
    if let Some(model) = turn.model {
        argv.push("--model".to_owned());
        argv.push(model.to_owned());
    }
    if let Some(cap) = turn.budget_usd {
        argv.push("--max-budget-usd".to_owned());
        argv.push(cap.to_string());
    }
    if let Some(session) = turn.session_id {
        argv.push("--resume".to_owned());
        argv.push(session.to_owned());
        // Measured on 2.1.270: the loaded history stops at that message and the
        // turn writes a new session, leaving the one it forked from untouched.
        if let Some(at) = turn.fork_at {
            argv.push("--fork-session".to_owned());
            argv.push(format!("--resume-session-at={at}"));
        }
    }
    if let Some(settings) = turn.settings {
        // `=`, because the flag takes several files: written as two words it
        // would swallow whatever came after it.
        argv.push(format!("--settings={settings}"));
        // Without a prompt tool a printing turn is not offered AskUserQuestion
        // or ExitPlanMode; the hook answers them, and `control` refuses the rest.
        argv.push("--permission-prompt-tool".to_owned());
        argv.push("stdio".to_owned());
    }
    if let Some(sources) = turn.setting_sources {
        argv.push(format!("--setting-sources={sources}"));
    }
    if let Some(config) = turn.mcp_config {
        argv.push(format!("--mcp-config={config}"));
    }
    if let Some(mode) = turn.permission {
        argv.push("--permission-mode".to_owned());
        argv.push(mode.to_owned());
    }
    // One `=` each, like the folders below: a rule can hold a space.
    for tool in turn.disallowed {
        argv.push(format!("--disallowedTools={tool}"));
    }
    if let Some(effort) = turn.effort {
        argv.push("--effort".to_owned());
        argv.push(effort.to_owned());
    }
    // One `=` each: the flag takes several folders, and written apart it would
    // swallow whatever came after it.
    for dir in turn.add_dirs {
        argv.push(format!("--add-dir={dir}"));
    }
    if let Some(name) = turn.name {
        argv.push("--name".to_owned());
        argv.push(name.to_owned());
    }

    argv
}

#[cfg(test)]
#[path = "talk_tests.rs"]
mod tests;
