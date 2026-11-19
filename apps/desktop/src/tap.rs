//! Listening to a pane nobody is looking at.
//!
//! [`devpit_pty::osc`] says why the parser is in Rust rather than in the
//! webview:
//!
//! > *recognition has to work with the window closed. A parser living in the
//! > webview is a parser that is asleep exactly when a notification would be
//! > worth sending.*
//!
//! It was half true. The parser is in Rust, and until this it only ran while a
//! pane was attached — so closing the tab stopped the watching, which is the
//! moment the watching is worth most.
//!
//! tmux keeps piping a pane's output with no client attached, so the tap arms
//! `pipe-pane` per window and reads the copy. Two failure modes were measured
//! before relying on it: a reader that dies leaves the pane running, and a
//! reader that stalls does not stall the pane — two hundred thousand lines
//! with nobody draining, and the pane still answered.
//!
//! On Windows psmux copies into a regular file rather than a fifo, and
//! `tap_file` follows it; everything below the source — the scanner,
//! the events, the screen — stays exactly as it is.

use std::collections::HashMap;
#[cfg(unix)]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use devpit_pty::Told;
use tauri::Emitter;

use crate::happening::said;
use crate::sessions::SessionState;
use devpit_rpc::SessionLayout;
use tauri::State;

/// One pane being listened to.
struct Tap {
    /// Cleared to stop the reader; it checks between reads.
    listening: Arc<AtomicBool>,
    /// Whose session the window is in, so tidying one project never ends
    /// another's taps.
    project: String,
}

/// Every pane this app is listening to, across projects.
#[derive(Default)]
pub struct Taps {
    open: Mutex<HashMap<String, Tap>>,
}

impl Taps {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts listening to a pane, if it is not already.
    ///
    /// Idempotent because the callers are: ensuring a session walks every leaf
    /// it has, and does so on every window that opens the project.
    pub fn watch(
        &self,
        app: &tauri::AppHandle,
        server: &devpit_tmux::Server,
        project_id: &str,
        leaf_id: &str,
        target: &str,
    ) {
        let Ok(mut open) = self.open.lock() else {
            return;
        };
        if open.contains_key(leaf_id) {
            return;
        }
        let Some(pipe) = start(&mut open, app, project_id, leaf_id) else {
            return;
        };
        if server.pipe_pane(target, &pipe).is_err() {
            if let Some(tap) = open.remove(leaf_id) {
                end(&tap, leaf_id);
            }
        }
    }

    /// Starts reading for a window about to be made, and answers the pipe to
    /// make it with, so nothing it prints first is missed.
    pub(crate) fn ahead_of(
        &self,
        app: &tauri::AppHandle,
        project_id: &str,
        leaf_id: &str,
    ) -> Option<String> {
        let mut open = self.open.lock().ok()?;
        start(&mut open, app, project_id, leaf_id)
    }

    /// Stops listening. The pane keeps running; only the copy ends.
    pub fn forget(&self, server: &devpit_tmux::Server, leaf_id: &str, target: &str) {
        let Ok(mut open) = self.open.lock() else {
            return;
        };
        let tap = open.remove(leaf_id);
        let _ = server.unpipe(target);
        if let Some(tap) = tap {
            end(&tap, leaf_id);
        }
    }

    /// Ends the taps of a project whose window has gone, and the fifos
    /// nothing reads.
    ///
    /// A shell that exits by itself takes its window with it and tells nobody,
    /// so its tap stayed — thread, fifo and entry — and the entry kept a
    /// window made again under the same leaf from ever being tapped.
    pub(crate) fn sweep(&self, server: &devpit_tmux::Server, project_id: &str, windows: &[String]) {
        let Ok(mut open) = self.open.lock() else {
            return;
        };
        let listed = open
            .iter()
            .map(|(leaf, tap)| (leaf.as_str(), tap.project.as_str()));
        for leaf in gone(listed, project_id, windows) {
            if let Some(tap) = open.remove(&leaf) {
                end(&tap, &leaf);
            }
        }
        let Some(root) = taps_root() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(&root) else {
            return;
        };
        let unread: Vec<(String, PathBuf)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let leaf = name.strip_suffix(COPY_SUFFIX)?.to_owned();
                (!open.contains_key(&leaf)).then(|| (leaf, entry.path()))
            })
            .collect();
        if unread.is_empty() {
            return;
        }
        // Every session's windows: a fifo of a project not opened since the
        // app started is still that window's, and is armed again when it is.
        let anywhere = server.all_windows().unwrap_or_default();
        for (leaf, path) in unread {
            if !anywhere.contains(&leaf) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// The leaves of `project_id` among `taps` whose window is not in `windows`.
pub(crate) fn gone<'a>(
    taps: impl Iterator<Item = (&'a str, &'a str)>,
    project_id: &str,
    windows: &[String],
) -> Vec<String> {
    taps.filter(|(leaf, project)| {
        *project == project_id && !windows.iter().any(|window| window == leaf)
    })
    .map(|(leaf, _)| leaf.to_owned())
    .collect()
}

/// Makes the fifo, starts its reader and answers what tmux pipes into it.
fn start(
    open: &mut HashMap<String, Tap>,
    app: &tauri::AppHandle,
    project_id: &str,
    leaf_id: &str,
) -> Option<String> {
    let path = fifo_for(leaf_id)?;
    #[cfg(unix)]
    make_fifo(&path).ok()?;
    #[cfg(windows)]
    let from = crate::tap_file::emptied(&path)?;
    let listening = Arc::new(AtomicBool::new(true));
    open.insert(
        leaf_id.to_owned(),
        Tap {
            listening: Arc::clone(&listening),
            project: project_id.to_owned(),
        },
    );
    let pipe = pipe_command(&path);
    #[cfg(unix)]
    spawn_reader(app.clone(), leaf_id.to_owned(), path, listening);
    #[cfg(windows)]
    spawn_follower(
        app.clone(),
        project_id,
        leaf_id.to_owned(),
        path,
        from,
        listening,
    );
    Some(pipe)
}

/// What tmux runs to copy a pane into its fifo: see `devpit_tmux::copy_to`.
pub(crate) fn pipe_command(path: &Path) -> String {
    devpit_tmux::copy_to(path, false)
}

/// Ends a tap's reader and removes its fifo.
fn end(tap: &Tap, leaf_id: &str) {
    tap.listening.store(false, Ordering::Relaxed);
    // The reader is blocked inside `read` and only looks at the flag between
    // reads, so a quiet pane would leave a thread waiting on a fifo forever.
    // One byte wakes it; it sees the flag and leaves. Non-blocking, so a
    // reader that never opened the fifo cannot hang this instead. A file's
    // follower looks on its own, and a byte would be written into the copy.
    if let Some(path) = fifo_for(leaf_id) {
        #[cfg(unix)]
        if let Ok(mut waking) = waking(&path) {
            use std::io::Write;
            let _ = waking.write_all(b"\0");
        }
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(unix)]
fn waking(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

/// Where a pane's copy arrives.
fn fifo_for(leaf_id: &str) -> Option<PathBuf> {
    // The leaf id is ours and is alphanumeric, so it needs no escaping — but
    // a path is built from it, so anything else is refused rather than
    // sanitised into a name that collides with another pane's.
    if leaf_id.is_empty()
        || !leaf_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(taps_root()?.join(format!("{leaf_id}{COPY_SUFFIX}")))
}

/// A fifo's name, or on Windows a regular file's: psmux writes a file.
const COPY_SUFFIX: &str = if cfg!(windows) { ".log" } else { ".fifo" };

fn taps_root() -> Option<PathBuf> {
    let root = devpit_core::Store::root().ok()?.join("taps");
    std::fs::create_dir_all(&root).ok()?;
    Some(root)
}

#[cfg(unix)]
fn make_fifo(path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    if path.exists() {
        return Ok(());
    }
    let name = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("a path with a nul in it"))?;
    // 0o600: the copy of a terminal's output is as private as the terminal.
    let made = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    if made == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Reads a pane's copy for as long as it is being listened to.
#[cfg(unix)]
fn spawn_reader(app: tauri::AppHandle, leaf_id: String, path: PathBuf, listening: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        // Opened read *and* write: a fifo opened read-only ends the moment the
        // writer does, and the writer here is one `cat` per arming. Holding a
        // writer end ourselves means the reader outlives them and never sees a
        // false end of stream.
        let Ok(mut fifo) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
        else {
            return;
        };
        let mut buffer = [0u8; 8192];
        while listening.load(Ordering::Relaxed) {
            let read = match fifo.read(&mut buffer) {
                Ok(0) => continue,
                Ok(read) => read,
                Err(_) => break,
            };
            // Cut into blocks there, and what it says about itself reported
            // from there — one scan of the stream, not two.
            crate::blocks::heard(&app, &leaf_id, &buffer[..read]);
        }
    });
}

/// How big a Windows copy grows before its pipe is armed again, which empties
/// it: psmux holds the file, so nothing here can cut it.
#[cfg(windows)]
const COPY_CEILING: u64 = 8 * 1024 * 1024;

/// Follows a pane's copy in a file, from `from`, for as long as it is being
/// listened to — the Windows reader. See `tap_file`.
#[cfg(windows)]
fn spawn_follower(
    app: tauri::AppHandle,
    project_id: &str,
    leaf_id: String,
    path: PathBuf,
    from: u64,
    listening: Arc<AtomicBool>,
) {
    let session = devpit_tmux::Server::session_name(project_id);
    let target = devpit_tmux::Server::target(&session, &leaf_id);
    std::thread::spawn(move || {
        let Ok(tail) = crate::tap_file::Tail::open(&path, from) else {
            return;
        };
        let rotate = || {
            if let Ok(server) = crate::sessions::tmux_server() {
                let _ = server.pipe_pane(&target, &pipe_command(&path));
            }
        };
        crate::tap_file::follow(
            tail,
            &listening,
            COPY_CEILING,
            |bytes| crate::blocks::heard(&app, &leaf_id, bytes),
            rotate,
        );
    });
}

/// What the window is told — the same event an attached pane sends, so the
/// screen has one thing to listen to whether or not it is looking.
pub(crate) fn report(app: &tauri::AppHandle, leaf_id: &str, told: Told) {
    if heard_by_tap(&told) {
        let _ = app.emit("terminal:happening", said(leaf_id, told));
    }
}

/// Which of the two streams speaks for what. The tap hears the pane itself —
/// prompts, commands, exits, titles, the cwd. A clipboard request is the one
/// thing tmux answers for its client (`set-clipboard`), so that one comes from
/// the attached side; heard on both, it would be copied twice.
pub(crate) fn heard_by_tap(told: &Told) -> bool {
    !matches!(told, Told::Clipboard(_))
}

/// Starts listening to every leaf the layout has.
///
/// Here rather than in `load_or_create` because that is also the read path:
/// arming a pipe is a side effect, and a function that only looks should not
/// have one. Idempotent, so calling it on every ensure is free.
pub(crate) fn listen(state: &State<SessionState>, project_id: &str, layout: &SessionLayout) {
    let session = devpit_tmux::Server::session_name(project_id);
    let blocks = tauri::Manager::try_state::<crate::blocks::Blocks>(state.app());
    // Asked for at the first tmux leaf: a tab of ptyd panes needs no tmux.
    let mut tmux = None;
    for (leaf_id, target) in layout.tree.leaves() {
        if let Some(blocks) = &blocks {
            blocks.belongs(leaf_id, project_id);
        }
        // Heard by `ptyd_tap` instead: there is no tmux window to pipe. A
        // host's window is not on this machine to pipe either.
        if crate::ptyd_link::is_ptyd(target) || crate::host_panes::is_host(target) {
            continue;
        }
        let server = tmux.get_or_insert_with(|| crate::sessions::tmux_server().ok());
        let Some(server) = server.as_ref() else {
            continue;
        };
        let target = devpit_tmux::Server::target(&session, leaf_id);
        state
            .taps
            .watch(state.app(), server, project_id, leaf_id, &target);
    }
}

/// Ends whatever a pane is running, before its window goes.
///
/// Closing the tmux window is a `SIGHUP`, and a TUI agent installs a handler
/// for exactly that so it survives a disconnected terminal. Measured: a
/// process that ignores `SIGHUP` outlived `kill-window` and was reparented to
/// init, still working, with nothing left able to reach it. The close prompt
/// says "this will stop the agent's current work", and it has to be true.
///
/// Everything the pane's shell started, not only what is in front: a job in
/// the background and an agent's `setsid` children outlived the foreground
/// group. Every leaf at once, so a tab of several pays one grace, not one
/// each. A shell with nothing under it is left to the window going.
pub(crate) fn stop_whatever_runs(server: &devpit_tmux::Server, session: &str, leaves: &[&str]) {
    let shells: Vec<u32> = leaves
        .iter()
        .filter_map(|leaf| server.pane_pid(session, leaf))
        .collect();
    match devpit_pty::session_members(&shells) {
        Some(members) => {
            devpit_pty::stop_members(&members, devpit_pty::AGENT_GRACE);
        }
        None => {
            for leaf in leaves {
                stop_the_front(server, session, leaf);
            }
        }
    }
}

/// [`stop_whatever_runs`] for the ptyd panes at `socket`: the daemon's children
/// started with each leaf's name, never a daemon one left. Without a process
/// table it stops nothing, and an agent that ignores `SIGHUP` outlives the pane.
#[cfg(unix)]
pub(crate) fn stop_whatever_runs_in_ptyd(socket: &Path, leaves: &[String]) {
    let Some(daemon) = devpit_ptyd::socket::connect(socket)
        .and_then(|stream| devpit_ptyd::socket::peer_pid(&stream))
        .ok()
    else {
        return;
    };
    let shells: Vec<u32> = leaves
        .iter()
        .filter_map(|leaf| devpit_pty::started_under(daemon, devpit_tmux::PANE_ENV, leaf))
        .flatten()
        .collect();
    if let Some(members) = devpit_pty::session_members(&shells) {
        devpit_pty::stop_members(&members, devpit_pty::AGENT_GRACE);
    }
}

/// Where the process table cannot be read: the foreground group only.
fn stop_the_front(server: &devpit_tmux::Server, session: &str, leaf_id: &str) {
    let Ok(panes) = server.running(session) else {
        return;
    };
    let Some(pane) = panes.into_iter().find(|one| one.leaf_id == leaf_id) else {
        return;
    };
    let fronts = devpit_pty::looking(std::slice::from_ref(&pane.tty));
    let Some(front) = devpit_pty::front_on(&fronts, &pane.tty) else {
        return;
    };
    let command = devpit_pty::agents::program_of(&front.argv).unwrap_or(pane.command);
    if devpit_pty::agents::idle_shell(&command) {
        return;
    }
    devpit_pty::stop_group(front.pgid, devpit_pty::GRACE);
}

#[cfg(test)]
#[path = "tap_tests.rs"]
mod tests;
