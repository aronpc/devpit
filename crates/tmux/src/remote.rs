//! A tmux server on another machine, reached through a command that runs one
//! line there: ssh, with the line it is handed last.

use crate::{Server, TmuxError};

/// The server devpit runs on a host, by name: never the person's own there.
pub const HOST_SERVER: &str = "devpit";

/// `words` as the one line a POSIX shell gives back word for word: ssh
/// joins its arguments into a line the host's shell reads again.
pub fn shell_line(words: &[&str]) -> String {
    words
        .iter()
        .map(|word| quoted(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A word in single quotes, a quote inside it closed and escaped. A word of
/// plain characters is left as it is, for a line a person can read; one
/// starting with `=` is not plain, since zsh reads it as a program's path.
pub fn quoted(word: &str) -> String {
    let plain = !word.is_empty()
        && !word.starts_with('=')
        && word
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "@%+=:,./_-".contains(ch));
    match plain {
        true => word.to_owned(),
        false => format!("'{}'", word.replace('\'', r"'\''")),
    }
}

/// The argv that runs tmux `args` on the server `via` reaches.
pub(crate) fn argv(via: &[String], args: &[&str]) -> Vec<String> {
    let mut words = vec!["tmux", "-L", HOST_SERVER];
    words.extend_from_slice(args);
    via.iter().cloned().chain([shell_line(&words)]).collect()
}

impl Server {
    /// A server on another machine: `via` runs the line it is handed last.
    pub fn over(via: Vec<String>) -> Self {
        Server {
            via: Some(via),
            ..Server::new(std::path::PathBuf::new())
        }
    }

    /// Whether the server has a session of exactly this name, not one it
    /// starts.
    pub fn has_exactly(&self, session: &str) -> Result<bool, TmuxError> {
        let output = self.run(&["has-session", "-t", &format!("={session}")])?;
        Ok(output.status.success())
    }
}

impl Server {
    /// What tmux says of `target` in `format`: `#{pane_current_command}`.
    pub fn format_of(&self, target: &str, format: &str) -> Result<String, TmuxError> {
        let output = self.require(&["display-message", "-p", "-t", target, format])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
