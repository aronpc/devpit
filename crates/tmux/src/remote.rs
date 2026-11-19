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

/// A word quoted for the login shell on a host. Plain characters stay as
/// they are, for a line a person can read; the rest goes in single quotes,
/// and a quote or a backslash outside them, in double quotes: fish reads
/// `\\` and `\'` inside single quotes, POSIX shells read nothing there, and
/// both read `"'"` and `"\\"` the same. A word starting with `=` is not plain,
/// since zsh reads it as a program's path.
pub fn quoted(word: &str) -> String {
    let plain = !word.is_empty()
        && !word.starts_with('=')
        && word
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "@+=:,./_-".contains(ch));
    if plain {
        return word.to_owned();
    }
    let mut quoted = String::new();
    let mut run = String::new();
    let close = |quoted: &mut String, run: &mut String| {
        if !run.is_empty() {
            quoted.push('\'');
            quoted.push_str(run);
            quoted.push('\'');
            run.clear();
        }
    };
    for ch in word.chars() {
        match ch {
            '\'' => {
                close(&mut quoted, &mut run);
                quoted.push_str("\"'\"");
            }
            '\\' => {
                close(&mut quoted, &mut run);
                quoted.push_str("\"\\\\\"");
            }
            _ => run.push(ch),
        }
    }
    close(&mut quoted, &mut run);
    if quoted.is_empty() {
        quoted.push_str("''");
    }
    quoted
}

/// The folders a host's login keeps its own programs in, which a login that
/// is not interactive often lacks: as a shell assignment.
pub const HOST_PATH: &str =
    "PATH=\"$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/bin:/usr/local/bin:$PATH\"";

/// The argv that runs tmux `args` on the server `via` reaches.
pub(crate) fn argv(via: &[String], args: &[&str]) -> Vec<String> {
    let mut words = vec!["tmux", "-L", HOST_SERVER];
    words.extend_from_slice(args);
    let line = format!("{HOST_PATH} {}", shell_line(&words));
    via.iter().cloned().chain([line]).collect()
}

/// The most of a host's answer read; the rest is drained unread.
pub const OUTPUT_MOST: usize = 4 << 20;

/// How long the answer waits, once the child has ended, for output something
/// it left behind still holds open: an ssh connection's master can.
const DRAIN_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

/// What the child said, up to [`OUTPUT_MOST`] of each stream, or nothing
/// once it has run for `patience`, when it is ended with its group.
pub fn within_bounded(
    mut child: std::process::Child,
    patience: std::time::Duration,
) -> Option<std::io::Result<std::process::Output>> {
    type Kept = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;
    /// What a pipe said so far, and word once it ended.
    fn read(
        pipe: Option<impl std::io::Read + Send + 'static>,
    ) -> (Kept, std::sync::mpsc::Receiver<()>) {
        let kept: Kept = Default::default();
        let (done, ended) = std::sync::mpsc::channel();
        let into = std::sync::Arc::clone(&kept);
        std::thread::spawn(move || {
            if let Some(mut pipe) = pipe {
                let mut chunk = [0u8; 8192];
                while let Ok(got) = pipe.read(&mut chunk) {
                    if got == 0 {
                        break;
                    }
                    if let Ok(mut kept) = into.lock() {
                        let room = OUTPUT_MOST.saturating_sub(kept.len());
                        kept.extend_from_slice(&chunk[..got.min(room)]);
                    }
                }
            }
            let _ = done.send(());
        });
        (kept, ended)
    }
    let stdout = read(child.stdout.take());
    let stderr = read(child.stderr.take());
    let deadline = std::time::Instant::now() + patience;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            Ok(None) => {
                #[cfg(unix)]
                devpit_pty::stop_group(child.id(), std::time::Duration::from_millis(200));
                devpit_pty::process::terminate(child.id() as i32);
                let _ = child.wait();
                return None;
            }
            Err(err) => return Some(Err(err)),
        }
    };
    let until = std::time::Instant::now() + DRAIN_AFTER;
    let taken = |(kept, ended): (Kept, std::sync::mpsc::Receiver<()>)| {
        let _ = ended.recv_timeout(until.saturating_duration_since(std::time::Instant::now()));
        let mut kept = kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut *kept)
    };
    Some(Ok(std::process::Output {
        status,
        stdout: taken(stdout),
        stderr: taken(stderr),
    }))
}

/// Spawns `command`, again for a moment while exec says the program is busy:
/// one written just now can still be open in a child another thread forked,
/// until that child runs its own program.
pub fn spawned(command: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    for _ in 0..50 {
        match command.spawn() {
            Err(err) if err.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            spawned => return spawned,
        }
    }
    command.spawn()
}

/// The options devpit's own server is quieted with, as tmux's words: put
/// before a host session's `new-session`, so its first pane has them.
pub fn quieting() -> Vec<&'static str> {
    quieted(&crate::chrome::QUIET)
}

/// `options` set with `-q`: a host's tmux may be older than one of them, and
/// an error would end the line before the session it starts.
pub(crate) fn quieted<'a>(options: &[[&'a str; 2]]) -> Vec<&'a str> {
    let mut argv = Vec::new();
    for [name, value] in options {
        argv.extend(["set-option", "-q", "-g", name, value, ";"]);
    }
    let [set, scope, name, value] = crate::chrome::TRUE_COLOUR;
    argv.extend([set, "-q", scope, name, value]);
    argv
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
