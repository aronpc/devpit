//! A profile, as something that can be started.
//!
//! Three fields, and which of them a caller can honour is the whole reason
//! this type exists rather than a command string:
//!
//!   - **Spawned** — the board's headless turns and the chat. `Command::new`
//!     takes the program, the arguments and the environment separately and
//!     hands each over untouched. Nothing is parsed, so nothing can be
//!     misparsed.
//!   - **Typed** — a terminal, where tmux sends a line of text to a shell.
//!     There the three have to become one string, and `line` is the only place
//!     in the product that does it.
//!
//! A command string would have collapsed those two into the weaker one, and
//! would have been unable to express the case that started all of this: `glm`
//! is `claude` with seven variables in front of it and no shell involved.

use devpit_rpc::Profile;

use crate::declaring::quoted;

/// The variable a session's language rides in, which devpit sets itself at
/// each session's birth: a profile's own would speak over it.
pub const LOCALE_ENV: &str = "DEVPIT_LOCALE";

/// What a profile becomes when something runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runner {
    pub program: String,
    /// What the profile adds, before whatever the caller needs.
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl Runner {
    /// The program and its arguments, as one argument vector.
    pub fn argv(&self, then: &[String]) -> Vec<String> {
        let mut argv = self.args.clone();
        argv.extend_from_slice(then);
        argv
    }
}

/// The profile, ready to start.
pub fn runner(profile: &Profile) -> Runner {
    Runner {
        program: profile.command.clone(),
        args: profile.args.clone(),
        env: profile
            .env
            .iter()
            .filter(|var| var.name != LOCALE_ENV)
            .map(|var| (var.name.clone(), var.value.clone()))
            .collect(),
    }
}

/// The `NAME='value' ` assignments a typed line starts with, ready to be put
/// in front of any command the profile runs — the launch line, or an attach.
///
/// Every **value** is quoted, because a value is data and may be a token, a
/// URL or a path with a space in it. Nothing else is quoted, because nothing
/// else is allowed to need it.
pub fn assignments(runner: &Runner) -> String {
    let mut said = String::new();
    for (name, value) in &runner.env {
        said.push_str(name);
        said.push('=');
        said.push_str(&quoted(value));
        said.push(' ');
    }
    said
}

/// The line a terminal is asked to type.
///
/// Variables go in front as shell assignments, which is what the person's own
/// `.zshrc` was already doing by hand. Every **value** is quoted, because a
/// value is data and may be a token, a URL or a path with a space in it.
/// Nothing else is quoted, because nothing else is allowed to need it —
/// `declaring::allowed` refuses a program or an argument carrying anything a
/// shell would read.
///
/// **The line is visible.** It is typed into a terminal, so a token in a
/// variable is in that terminal's scrollback — where a shell *function* would
/// have hidden it behind its own name. Nothing here can avoid that: the
/// process needs the variable, the shell is already running, and writing the
/// value to a file to source would only move it somewhere it stays. The
/// spawned paths have no such problem; they never build a line at all.
pub fn line(runner: &Runner) -> String {
    let mut said = assignments(runner);
    said.push_str(&runner.program);
    for arg in &runner.args {
        said.push(' ');
        said.push_str(arg);
    }
    said
}

#[cfg(test)]
#[path = "running_tests.rs"]
mod tests;
