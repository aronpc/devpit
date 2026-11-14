//! Typing into a pane and asking where it stands, whichever holds it.
//!
//! A pane is tmux's or devpit-ptyd's, told apart by its target
//! ([`crate::ptyd_link::is_ptyd`]). What types into a pane comes through
//! here, so nothing hands tmux a target it has never heard of.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use devpit_rpc::{ErrorCode, RpcError, SessionLayout};
use devpit_tmux::Key;

use crate::shell_launch::Ready;

/// Who holds a pane, read off its target.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Holder<'a> {
    Tmux {
        session: &'a str,
        window: &'a str,
    },
    /// devpit-ptyd, by its number for the pane.
    Ptyd(u32),
}

pub(crate) fn holder(target: &str) -> Holder<'_> {
    match target.strip_prefix(crate::ptyd_link::TARGET_PREFIX) {
        Some(leaf) => Holder::Ptyd(crate::ptyd_link::pane_number(leaf)),
        None => {
            let (session, window) = target.split_once(':').unwrap_or((target, ""));
            Holder::Tmux { session, window }
        }
    }
}

/// The leaf a target names: tmux's window, or the ptyd pane's leaf.
pub(crate) fn leaf_of(target: &str) -> &str {
    match target.strip_prefix(crate::ptyd_link::TARGET_PREFIX) {
        Some(leaf) => leaf,
        None => target.split_once(':').map_or("", |(_, window)| window),
    }
}

/// The target of `leaf_id`, in whichever of the project's tabs holds it.
pub(crate) fn target_of(project_id: &str, leaf_id: &str) -> Result<String, RpcError> {
    let layout = crate::sessions::holding(project_id, leaf_id)?;
    Ok(target_in(&layout, leaf_id))
}

/// [`target_of`], answered without the store for a pane the tap has placed.
/// A project switch asks it for every pane it attaches and redraws, and
/// opening the store each time was most of what an attach cost the backend.
pub(crate) fn placed_target_of(project_id: &str, leaf_id: &str) -> Result<String, RpcError> {
    #[cfg(unix)]
    if crate::ptyd_watch::holds(project_id, leaf_id) {
        return Ok(crate::ptyd_link::target(leaf_id));
    }
    target_of(project_id, leaf_id)
}

/// A tmux leaf's target is rebuilt from its names rather than read back, so
/// a layout stored by an older build still reaches its window.
pub(crate) fn target_in(layout: &SessionLayout, leaf_id: &str) -> String {
    match crate::ptyd_link::leaf_is_ptyd(layout, leaf_id) {
        true => crate::ptyd_link::target(leaf_id),
        false => devpit_tmux::Server::target(
            &devpit_tmux::Server::session_name(&layout.project_id),
            leaf_id,
        ),
    }
}

/// Runs `line` at a pane's prompt: the line and the screen cleared first.
pub(crate) fn submit(target: &str, line: &str) -> Result<(), RpcError> {
    match holder(target) {
        Holder::Tmux { .. } => tmux()?.submit(target, line).map_err(internal),
        Holder::Ptyd(pane) => write(pane, submitted(line, bracketing(modes(pane), line))),
    }
}

/// Types `line` at a pane, as it is, and presses Enter.
pub(crate) fn send_line(target: &str, line: &str) -> Result<(), RpcError> {
    send_line_at(None, target, line)
}

/// [`send_line`], a ptyd pane's to the daemon of `home` when one is given.
pub(crate) fn send_line_at(
    home: Option<&std::path::Path>,
    target: &str,
    line: &str,
) -> Result<(), RpcError> {
    match (holder(target), home) {
        (Holder::Tmux { .. }, _) => tmux()?
            .send_keys(target, line)
            .map_err(crate::sessions::tmux_err),
        #[cfg(unix)]
        (Holder::Ptyd(pane), Some(home)) => write_at(home, pane, &typed(line)),
        (Holder::Ptyd(pane), _) => write(pane, typed(line)),
    }
}

/// Records which profile devpit started in a pane, where it is recorded:
/// a tmux pane option. Nothing on a ptyd pane; the store keeps the launch
/// for both, and a missing label is cosmetic.
pub(crate) fn label(target: &str, profile: &str) {
    if let (Holder::Tmux { .. }, Ok(server)) = (holder(target), tmux()) {
        let _ = server.name_pane(target, profile);
    }
}

/// Whether a pane is ready to be typed into.
pub(crate) fn settled(target: &str) -> Ready {
    match holder(target) {
        Holder::Tmux { session, window } => crate::shell_launch::settled(session, window),
        Holder::Ptyd(_) => settled_on(|| front(target), crate::shell_launch::AT_MOST),
    }
}

/// A ptyd pane's [`settled`]: the daemon names the front once output goes
/// quiet, so an idle shell named on consecutive looks is at its prompt.
pub(crate) fn settled_on(mut front: impl FnMut() -> Option<String>, at_most: Duration) -> Ready {
    let deadline = Instant::now() + at_most;
    let mut settling = devpit_pty::Settling::new();
    while Instant::now() < deadline {
        match front() {
            Some(program) if !devpit_pty::agents::idle_shell(&program) => {
                return Ready::Busy(program)
            }
            named => {
                if settling.looked(named.is_some()) {
                    return Ready::Prompt;
                }
            }
        }
        std::thread::sleep(crate::shell_launch::BETWEEN_LOOKS);
    }
    Ready::Unknown
}

/// Hands `text` to whatever runs in a pane as one paste, and Enter after it
/// when `enter`.
pub(crate) fn paste(target: &str, text: &str, enter: bool) -> Result<(), RpcError> {
    match holder(target) {
        Holder::Tmux { .. } if enter => tmux()?.paste_and_send(target, text).map_err(internal),
        Holder::Tmux { .. } => tmux()?.paste(target, text).map_err(internal),
        Holder::Ptyd(pane) => write(pane, pasted(text, enter, bracketing(modes(pane), text))),
    }
}

/// Presses `keys` on the program in a pane, in order.
pub(crate) fn press(target: &str, keys: &[Key]) -> Result<(), RpcError> {
    match holder(target) {
        Holder::Tmux { .. } => tmux()?.press(target, keys).map_err(internal),
        Holder::Ptyd(pane) => {
            let application = modes(pane).is_some_and(|modes| modes.application_cursor);
            write(pane, pressed(keys, application))
        }
    }
}

/// Whether a pane's shell is what is in front, rather than a program it runs.
pub(crate) fn shell_in_front(target: &str) -> Result<bool, RpcError> {
    match holder(target) {
        Holder::Tmux { .. } => tmux()?.shell_in_front(target).map_err(internal),
        Holder::Ptyd(_) => Ok(front(target)
            .as_deref()
            .is_some_and(devpit_pty::agents::idle_shell)),
    }
}

/// An Enter where the shell is idle in front, so it draws its prompt again.
/// Answers whether it pressed one.
pub(crate) fn nudge(target: &str) -> Result<bool, RpcError> {
    match holder(target) {
        Holder::Tmux { .. } => tmux()?.nudge(target).map_err(internal),
        Holder::Ptyd(_) => {
            if !shell_in_front(target)? {
                return Ok(false);
            }
            press(target, &[Key::Enter])?;
            Ok(true)
        }
    }
}

/// The folder a pane's shell stands in: tmux asks the kernel, the daemon
/// says what the shell last told it.
pub(crate) fn cwd(target: &str) -> Option<PathBuf> {
    match holder(target) {
        Holder::Tmux { session, window } => tmux().ok()?.pane_path(session, window),
        Holder::Ptyd(pane) => told_cwd(pane).map(PathBuf::from),
    }
}

/// Scrolls a pane's history. Nothing for a ptyd pane: no tmux client draws
/// it, so xterm holds the history and its own wheel scrolls it.
pub(crate) fn scroll(target: &str, lines: i32) -> Result<bool, RpcError> {
    match holder(target) {
        Holder::Tmux { .. } => tmux()?
            .scroll(target, lines)
            .map(|()| true)
            .map_err(internal),
        Holder::Ptyd(_) => Ok(false),
    }
}

/// A shell at its prompt, which its replay draws whole. A redraw is a resize,
/// and after a SIGWINCH zsh's line editor now and then fails a read and exits.
pub(crate) fn drawn_by_its_replay(front: Option<&str>) -> bool {
    front.is_some_and(devpit_pty::agents::idle_shell)
}

/// Has a pane's screen drawn again whole. Answers whether there was a window
/// on it to draw; the page asks again when there was not yet.
pub(crate) fn redraw(app: &tauri::AppHandle, target: &str) -> Result<bool, RpcError> {
    match holder(target) {
        Holder::Tmux { session, window } => tmux()?.redraw(session, window).map_err(internal),
        Holder::Ptyd(pane) if drawn_by_its_replay(told_front(pane).as_deref()) => Ok(true),
        Holder::Ptyd(_) => {
            let leaf = target
                .strip_prefix(crate::ptyd_link::TARGET_PREFIX)
                .unwrap_or_default();
            jiggle(app, leaf)
        }
    }
}

/// A ptyd pane drawn again: one column narrower and back is a SIGWINCH,
/// which every full-screen program answers by drawing itself whole. The
/// size is the attached window's; without one there is nothing to draw on.
fn jiggle(app: &tauri::AppHandle, leaf: &str) -> Result<bool, RpcError> {
    let state = tauri::Manager::state::<crate::sessions::SessionState>(app);
    let Ok(live) = state.claims.live(leaf) else {
        return Ok(false);
    };
    let Some(size) = live.ptyd_size() else {
        return Ok(false);
    };
    if size.cols > 1 {
        live.resize(size.rows, size.cols - 1)?;
    }
    live.resize(size.rows, size.cols)?;
    Ok(true)
}

/// What a pane runs in front: the program tmux finds on its tty, or the one
/// the daemon last named.
pub(crate) fn front(target: &str) -> Option<String> {
    match holder(target) {
        Holder::Tmux { session, window } => {
            let panes = tmux().ok()?.running(session).ok()?;
            let pane = panes.into_iter().find(|one| one.leaf_id == window)?;
            let fronts = devpit_pty::looking(std::slice::from_ref(&pane.tty));
            let program = devpit_pty::front_on(&fronts, &pane.tty)
                .and_then(|front| devpit_pty::agents::program_of(&front.argv));
            Some(program.unwrap_or(pane.command))
        }
        Holder::Ptyd(pane) => told_front(pane),
    }
}

/// A pane's screen as it is now, as text: tmux's capture, or what the ptyd
/// tap has drawn. Nothing when neither can say.
pub(crate) fn screen(target: &str) -> Option<String> {
    match holder(target) {
        Holder::Tmux { .. } => tmux().ok()?.capture_pane(target).ok(),
        Holder::Ptyd(pane) => drawn(pane),
    }
}

/// Where a pane's cursor stands, as (column, row) of its screen.
pub(crate) fn cursor(target: &str) -> Option<(u16, u16)> {
    match holder(target) {
        Holder::Tmux { .. } => tmux().ok()?.cursor(target),
        Holder::Ptyd(pane) => cursor_drawn(pane),
    }
}

#[cfg(unix)]
fn cursor_drawn(pane: u32) -> Option<(u16, u16)> {
    crate::ptyd_tap::tap().cursor(pane)
}

#[cfg(not(unix))]
fn cursor_drawn(_pane: u32) -> Option<(u16, u16)> {
    None
}

/// What a program asked of its terminal, which says how to type into it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Modes {
    pub(crate) application_cursor: bool,
    pub(crate) bracketed_paste: bool,
}

#[cfg(unix)]
fn told_front(pane: u32) -> Option<String> {
    crate::ptyd_watch::program(pane)
}

#[cfg(not(unix))]
fn told_front(_pane: u32) -> Option<String> {
    None
}

#[cfg(unix)]
fn drawn(pane: u32) -> Option<String> {
    crate::ptyd_tap::tap().screen(pane)
}

#[cfg(not(unix))]
fn drawn(_pane: u32) -> Option<String> {
    None
}

#[cfg(unix)]
fn modes(pane: u32) -> Option<Modes> {
    crate::ptyd_tap::tap().modes(pane)
}

#[cfg(not(unix))]
fn modes(_pane: u32) -> Option<Modes> {
    None
}

#[cfg(unix)]
fn told_cwd(pane: u32) -> Option<String> {
    crate::ptyd_watch::cwd(pane)
}

#[cfg(not(unix))]
fn told_cwd(_pane: u32) -> Option<String> {
    None
}

/// Kills what the shell's line holds and clears the screen: `C-e C-u C-l`,
/// as tmux's submit sends them.
const CLEAR: &[u8] = b"\x05\x15\x0c";
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// The bytes [`send_line`] types into a ptyd pane.
pub(crate) fn typed(line: &str) -> Vec<u8> {
    let mut bytes = line.as_bytes().to_vec();
    bytes.push(b'\r');
    bytes
}

/// The bytes [`submit`] types into a ptyd pane.
pub(crate) fn submitted(line: &str, bracketed: bool) -> Vec<u8> {
    let mut bytes = CLEAR.to_vec();
    bytes.extend(pasted(line, true, bracketed));
    bytes
}

/// Whether a paste goes bracketed: as the program asked, as tmux's
/// `paste-buffer -p` does. With no screen yet to say, when it is several
/// lines, so a shell does not run them one by one.
pub(crate) fn bracketing(modes: Option<Modes>, text: &str) -> bool {
    modes.map_or(text.contains('\n'), |modes| modes.bracketed_paste)
}

/// The bytes [`paste`] types into a ptyd pane.
pub(crate) fn pasted(text: &str, enter: bool, bracketed: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() + 16);
    // As tmux's paste-buffer does: a line ends in CR at a terminal.
    let lines = text.replace("\r\n", "\n").replace('\n', "\r");
    match bracketed {
        true => {
            // A pasted end marker would end the paste early and run the rest.
            bytes.extend_from_slice(PASTE_START);
            bytes.extend(lines.replace("\x1b[201~", "").bytes());
            bytes.extend_from_slice(PASTE_END);
        }
        false => bytes.extend(lines.bytes()),
    }
    if enter {
        bytes.push(b'\r');
    }
    bytes
}

/// The bytes a terminal sends for `keys`: the arrows in application mode
/// (`DECCKM`) when the program asked for it, as a full-screen one does.
pub(crate) fn pressed(keys: &[Key], application_cursor: bool) -> Vec<u8> {
    keys.iter()
        .flat_map(|key| match (key, application_cursor) {
            (Key::Up, false) => b"\x1b[A".as_slice(),
            (Key::Down, false) => b"\x1b[B",
            (Key::Up, true) => b"\x1bOA",
            (Key::Down, true) => b"\x1bOB",
            (Key::Enter, _) => b"\r",
            (Key::Escape, _) => b"\x1b",
        })
        .copied()
        .collect()
}

/// Bytes into a ptyd pane, on the tap's connection for typing.
#[cfg(unix)]
fn write(pane: u32, bytes: Vec<u8>) -> Result<(), RpcError> {
    write_at(&devpit_core::Store::root()?, pane, &bytes)
}

/// [`write`], to the daemon of `home`.
#[cfg(unix)]
pub(crate) fn write_at(home: &std::path::Path, pane: u32, bytes: &[u8]) -> Result<(), RpcError> {
    crate::ptyd_tap::tap()
        .type_into(home, pane, bytes)
        .map_err(|err| {
            RpcError::new(
                ErrorCode::Unsupported,
                format!("devpit-ptyd, which holds this terminal, did not take it: {err}"),
            )
        })
}

#[cfg(not(unix))]
fn write(_pane: u32, _bytes: Vec<u8>) -> Result<(), RpcError> {
    Err(RpcError::new(
        ErrorCode::Unsupported,
        "devpit-ptyd terminals are not on this platform",
    ))
}

fn tmux() -> Result<devpit_tmux::Server, RpcError> {
    crate::sessions::tmux_server()
}

fn internal(err: devpit_tmux::TmuxError) -> RpcError {
    RpcError::internal(err.to_string())
}

#[cfg(test)]
#[path = "pane_io_tests.rs"]
mod tests;
