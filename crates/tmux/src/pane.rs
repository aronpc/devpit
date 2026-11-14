//! What one pane does, as opposed to how sessions are arranged.

use crate::{Server, TmuxError};

/// Sends a copy of everything a pane prints to `command`'s stdin.
///
/// The point is that it does not need a client: tmux keeps piping with
/// nobody attached, which is what lets a build be watched in a tab you are
/// not looking at. Without `-o`, which is a toggle: arming a pane still piped
/// from the app's last run closed its pipe instead. Plain, any old pipe is
/// replaced.
///
/// Measured before relying on it: a reader that dies leaves the pane
/// running, and a reader that stalls does not stall the pane — 200,000
/// lines with nobody draining, and the pane still answered.
/// The `pipe-pane` command that copies a pane's output to `path`, appended
/// when `append`. `exec` spares a shell per pane where there is one; psmux
/// reads the redirection itself and opens the file, so it gets it bare.
pub fn copy_to(path: &std::path::Path, append: bool) -> String {
    let path = path.display().to_string().replace('\'', r"'\''");
    let into = if append { ">>" } else { ">" };
    if crate::naming::GROUPED {
        format!("exec cat {into} '{path}'")
    } else {
        format!("cat {into} '{path}'")
    }
}

pub(crate) fn pipe_pane(server: &Server, target: &str, command: &str) -> Result<(), TmuxError> {
    server.require(&["pipe-pane", "-t", target, command])?;
    Ok(())
}

/// One of a pane's own formats, asked of the window's pane in the project's
/// session — which exists as long as the window does, unlike its client.
fn pane_format(server: &Server, session: &str, window: &str, format: &str) -> Option<String> {
    let target = format!("{session}:{window}");
    let out = server
        .require(&["display-message", "-p", "-t", &target, format])
        .ok()?;
    let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!said.is_empty()).then_some(said)
}

impl Server {
    /// The folder a pane's shell stands in, as the kernel says — known before
    /// the shell has said anything about itself.
    pub fn pane_path(&self, session: &str, window: &str) -> Option<std::path::PathBuf> {
        pane_format(self, session, window, "#{pane_current_path}").map(Into::into)
    }

    /// Where a pane's cursor stands, as (column, row) of its screen.
    pub fn cursor(&self, target: &str) -> Option<(u16, u16)> {
        let out = self
            .require(&[
                "display-message",
                "-p",
                "-t",
                target,
                "#{cursor_x} #{cursor_y}",
            ])
            .ok()?;
        let said = String::from_utf8_lossy(&out.stdout);
        let (column, row) = said.trim().split_once(' ')?;
        Some((column.parse().ok()?, row.parse().ok()?))
    }

    /// The pid of the shell a window was started with: the leader of the
    /// session everything typed into it runs in.
    pub fn pane_pid(&self, session: &str, window: &str) -> Option<u32> {
        pane_format(self, session, window, "#{pane_pid}")?
            .parse()
            .ok()
    }
}

/// Stops the pipe. A pane that has none is not an error.
pub(crate) fn unpipe(server: &Server, target: &str) -> Result<(), TmuxError> {
    let _ = server.run(&["pipe-pane", "-t", target]);
    Ok(())
}

/// A key pressed on a pane's program, from a closed list: moving through a
/// prompt's choices and answering it is all this is for, and a free-form key
/// name is how `C-c` gets sent by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Enter,
    Escape,
}

impl Key {
    /// tmux's own name for it.
    pub(crate) fn named(self) -> &'static str {
        match self {
            Key::Up => "Up",
            Key::Down => "Down",
            Key::Enter => "Enter",
            Key::Escape => "Escape",
        }
    }
}

impl Server {
    /// Types a line into a pane and presses Enter.
    ///
    /// `-l`, and Enter apart: without it tmux reads each word of the line as a
    /// possible key name, so a command containing `Enter`, `Space` or `C-c`
    /// typed something other than what it said.
    pub fn send_keys(&self, target: &str, keys: &str) -> Result<(), TmuxError> {
        self.require(&["send-keys", "-t", target, "-l", "--", keys])?;
        self.require(&["send-keys", "-t", target, "Enter"])?;
        Ok(())
    }

    /// Presses `keys` on the program in a pane, in order, in one call.
    pub fn press(&self, target: &str, keys: &[Key]) -> Result<(), TmuxError> {
        let mut argv = vec!["send-keys", "-t", target];
        argv.extend(keys.iter().map(|key| key.named()));
        self.require(&argv)?;
        Ok(())
    }
}

pub(crate) fn capture_pane(server: &Server, target: &str) -> Result<String, TmuxError> {
    let output = server.require(&["capture-pane", "-t", target, "-p"])?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Records which profile devpit started in a pane.
///
/// A pane-scoped user option, which is tmux's own place for this and outlives
/// the app exactly as long as the pane does — a terminal survives a restart,
/// and so should the answer to what is in it.
///
/// Its own storage rather than a map in this process for that reason alone.
/// The caller treats a failure as cosmetic: the row falls back to reading the
/// process, which is what it did before there were profiles.
pub(crate) fn name_pane(server: &Server, target: &str, profile: &str) -> Result<(), TmuxError> {
    server.require(&["set-option", "-p", "-t", target, "@devpit_profile", profile])?;
    Ok(())
}
