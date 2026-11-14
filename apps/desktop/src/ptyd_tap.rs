//! Listening to the panes devpit-ptyd holds, whether or not a window is on
//! them: [`crate::tap`]'s `pipe-pane`, for the daemon.
//!
//! One connection per home sends `Watch` — what each pane runs and where —
//! and attaches every ptyd leaf the layouts have, and each new one as it is
//! opened. What a pane prints is cut into blocks there as tmux's copy is, so
//! a long command's end is told with no window open.
//!
//! The daemon keeps one count of unacknowledged output per pane, and every
//! attached client's `Ack` takes from it, but a mirror's (`ptyd-attach`) while
//! another watcher is on. A window acks what its page parsed; the tap acks
//! only while no window is on the pane ([`acked`]).

use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use devpit_ptyd::frame::{self, Frame, Kind};

use crate::pane_io::Modes;
use crate::ptyd_link::pane_number;
use crate::ptyd_screens::Screens;
use crate::ptyd_watch::Fronts;

/// The first wait before asking for a daemon that was not there again, and
/// the longest it grows to.
const SOON: Duration = Duration::from_millis(200);
const AGAIN: Duration = Duration::from_secs(5);

/// How long the tap goes without a frame before it counts as having heard
/// everything printed so far.
const IDLE: Duration = Duration::from_millis(250);

/// How long typing waits on a daemon that stopped reading before its
/// connection is given up, so one stuck pane cannot hold every other's keys.
const TYPING_WAIT: Duration = Duration::from_secs(2);

/// A frame's worth of input at most, so a long paste is several frames
/// rather than one the daemon refuses as too long.
const INPUT_CHUNK: usize = 64 * 1024;

/// Where what the tap hears goes. The app's are the blocks, the window and
/// the claims; a test's are its own.
pub(crate) trait Ears: Send + Sync {
    /// Output pane `leaf` of `project` printed, as it printed it.
    fn heard(&self, project: &str, leaf: &str, bytes: &[u8]);
    /// Whether a window is attached to `leaf`, and acks its output itself.
    fn window_on(&self, leaf: &str) -> bool;
    /// What runs in a pane of `project` changed, or the daemon went.
    fn changed(&self, project: &str);
    /// Every ptyd leaf of the stored layouts, by the daemon's number.
    fn places(&self) -> HashMap<u32, (String, String)>;
}

/// What the tap acks of `bytes` a pane printed: all of it while no window is
/// on the pane, nothing while one is. Two ackers would each take from the
/// one count, and the pane would run ahead of the page it is drawn on.
pub(crate) fn acked(window_on: bool, bytes: usize) -> Option<u32> {
    (!window_on).then(|| u32::try_from(bytes).unwrap_or(u32::MAX))
}

/// The connection while there is one, and the panes attached over it.
struct Link {
    stream: UnixStream,
    attached: HashSet<u32>,
}

impl Link {
    fn attach(&mut self, pane: u32) {
        if self.attached.insert(pane) {
            let _ = frame::write(
                &mut self.stream,
                &Frame::new(pane, Kind::Attach, Vec::new()),
            );
        }
    }
}

#[derive(Default)]
pub(crate) struct Tap {
    pub(crate) fronts: Mutex<Fronts>,
    screens: Mutex<Screens>,
    link: Mutex<Option<Link>>,
    /// Typing's own connection, and the home it is to: input that blocks on
    /// a pane that is not reading must not hold up the acks on `link`.
    typing: Mutex<Option<(PathBuf, UnixStream)>>,
    /// A pane was opened while there was no connection: ask now, not later.
    asked: Mutex<bool>,
    wake: Condvar,
}

/// The app's tap.
pub(crate) fn tap() -> &'static Tap {
    static TAP: OnceLock<Tap> = OnceLock::new();
    TAP.get_or_init(Tap::default)
}

impl Tap {
    /// Hears `leaf` of `project` from now on: attached at once while
    /// connected, and otherwise the connection is tried at once.
    pub(crate) fn heed(&self, project: &str, leaf: &str) {
        let pane = pane_number(leaf);
        if let Ok(mut fronts) = self.fronts.lock() {
            fronts.place(HashMap::from([(
                pane,
                (project.to_owned(), leaf.to_owned()),
            )]));
        }
        let attached = self
            .link
            .lock()
            .is_ok_and(|mut link| link.as_mut().map(|link| link.attach(pane)).is_some());
        if !attached {
            if let Ok(mut asked) = self.asked.lock() {
                *asked = true;
                self.wake.notify_all();
            }
        }
    }

    /// A window let go of `leaf`: when it was the last, what it left
    /// unacknowledged is the tap's to ack, or the pane stays held for it.
    pub(crate) fn let_go(&self, leaf: &str, window_on: bool) {
        if let Some(bytes) = acked(window_on, u32::MAX as usize) {
            self.ack(pane_number(leaf), bytes);
        }
    }

    fn ack(&self, pane: u32, bytes: u32) {
        if let Ok(mut link) = self.link.lock() {
            if let Some(link) = link.as_mut() {
                let ack = Frame::new(pane, Kind::Ack, bytes.to_le_bytes().to_vec());
                let _ = frame::write(&mut link.stream, &ack);
            }
        }
    }

    /// Listens to the daemon of `home` for as long as the app runs: whenever
    /// it is there, and never starting it.
    pub(crate) fn serve(&self, home: &Path, ears: &dyn Ears) {
        if let Ok(mut screens) = self.screens.lock() {
            screens.keep_at(home.join(crate::ptyd_screens::SIZES));
        }
        let socket = crate::ptyd_link::socket_path(home);
        let mut wait = SOON;
        loop {
            if self.session(&socket, ears) {
                wait = SOON;
            }
            self.nap(wait);
            wait = (wait * 2).min(AGAIN);
        }
    }

    fn nap(&self, wait: Duration) {
        let Ok(asked) = self.asked.lock() else {
            return std::thread::sleep(wait);
        };
        if let Ok((mut asked, _)) = self.wake.wait_timeout_while(asked, wait, |asked| !*asked) {
            *asked = false;
        }
    }

    /// One connection, until the daemon goes. False when there was none.
    pub(crate) fn session(&self, socket: &Path, ears: &dyn Ears) -> bool {
        let Ok(stream) = devpit_ptyd::socket::connect(socket) else {
            return false;
        };
        let Ok(mut reading) = stream.try_clone() else {
            return false;
        };
        if self.begin(stream, ears).is_err() {
            return false;
        }
        // Panes acked since the tap last ran out of frames.
        let mut owed = HashSet::new();
        loop {
            if !owed.is_empty() && !readable_within(&reading, IDLE) {
                self.settle(&mut owed, ears);
                continue;
            }
            match frame::read(&mut reading) {
                Ok(Some(one)) => self.heard(one, ears, &mut owed),
                Ok(None) | Err(_) => break,
            }
        }
        if let Ok(mut link) = self.link.lock() {
            *link = None;
        }
        let gone = self
            .fronts
            .lock()
            .map(|mut fronts| fronts.forget_all())
            .unwrap_or_default();
        for project in gone {
            ears.changed(&project);
        }
        true
    }

    /// `Watch`, then every pane known attached. Under the link's lock, so a
    /// pane heeded meanwhile is attached here or by [`Tap::heed`], once.
    fn begin(&self, stream: UnixStream, ears: &dyn Ears) -> io::Result<()> {
        let mut held = self
            .link
            .lock()
            .map_err(|_| io::Error::other("ptyd tap lock"))?;
        let mut link = Link {
            stream,
            attached: HashSet::new(),
        };
        frame::write(&mut link.stream, &Frame::new(0, Kind::Watch, Vec::new()))?;
        let panes = self
            .fronts
            .lock()
            .map(|mut fronts| {
                fronts.place(ears.places());
                fronts.panes()
            })
            .unwrap_or_default();
        for pane in panes {
            link.attach(pane);
        }
        *held = Some(link);
        Ok(())
    }

    /// Acks what the daemon still counts for a pane no window is on: an ack can
    /// land before it counts the output, and the leftover would stop the pane.
    fn settle(&self, owed: &mut HashSet<u32>, ears: &dyn Ears) {
        for pane in owed.drain() {
            let leaf = self
                .fronts
                .lock()
                .ok()
                .and_then(|fronts| fronts.place_of(pane).map(|(_, leaf)| leaf.clone()));
            let window_on = leaf.is_some_and(|leaf| ears.window_on(&leaf));
            if let Some(bytes) = acked(window_on, u32::MAX as usize) {
                self.ack(pane, bytes);
            }
        }
    }

    fn heard(&self, one: Frame, ears: &dyn Ears, owed: &mut HashSet<u32>) {
        match one.kind() {
            Some(Kind::Output) => {
                if let Ok(mut screens) = self.screens.lock() {
                    screens.output(one.pane, &one.bytes);
                }
                if self.output(one.pane, &one.bytes, ears) {
                    owed.insert(one.pane);
                }
                return;
            }
            // Drawn, never heard again: it was cut into blocks when printed.
            Some(Kind::Replay) => {
                if let Ok(mut screens) = self.screens.lock() {
                    screens.replay(one.pane, &one.bytes);
                }
                return;
            }
            // Attached again when it is opened again.
            Some(kind @ (Kind::Exited | Kind::Refused)) => {
                if let Ok(mut link) = self.link.lock() {
                    if let Some(link) = link.as_mut() {
                        link.attached.remove(&one.pane);
                    }
                }
                if let (Kind::Exited, Ok(mut screens)) = (kind, self.screens.lock()) {
                    screens.ended(one.pane);
                }
            }
            _ => {}
        }
        let Ok(mut fronts) = self.fronts.lock() else {
            return;
        };
        let Some(pane) = fronts.heard(&one) else {
            return;
        };
        if !fronts.placed(pane) {
            fronts.place(ears.places());
        }
        if let Some(project) = fronts.project_of(pane).map(ToOwned::to_owned) {
            drop(fronts);
            ears.changed(&project);
        }
    }

    /// Hands output on, and answers whether the tap acked it.
    fn output(&self, pane: u32, bytes: &[u8], ears: &dyn Ears) -> bool {
        let place = self
            .fronts
            .lock()
            .ok()
            .and_then(|fronts| fronts.place_of(pane).cloned());
        let window_on = match &place {
            Some((project, leaf)) => {
                ears.heard(project, leaf, bytes);
                ears.window_on(leaf)
            }
            None => false,
        };
        let Some(bytes) = acked(window_on, bytes.len()) else {
            return false;
        };
        self.ack(pane, bytes);
        true
    }

    /// What pane `pane` shows, as text.
    pub(crate) fn screen(&self, pane: u32) -> Option<String> {
        self.screens.lock().ok()?.contents(pane)
    }

    /// Where pane `pane`'s cursor stands, as (column, row).
    pub(crate) fn cursor(&self, pane: u32) -> Option<(u16, u16)> {
        self.screens.lock().ok()?.cursor(pane)
    }

    /// How the program in pane `pane` wants its keys and pastes.
    pub(crate) fn modes(&self, pane: u32) -> Option<Modes> {
        self.screens.lock().ok()?.modes(pane)
    }

    /// The size devpit just told the daemon pane `pane` is: it never says.
    pub(crate) fn sized(&self, pane: u32, rows: u16, cols: u16) {
        if let Ok(mut screens) = self.screens.lock() {
            screens.sized(pane, rows, cols);
        }
    }

    /// Bytes into pane `pane` of the daemon of `home`, on the connection kept
    /// for typing. Connected only, never started: a daemon that is not there
    /// holds no pane to type into. Nothing is waited for — `Input` has no
    /// answer, and keys waited on are keys typed slowly.
    pub(crate) fn type_into(&self, home: &Path, pane: u32, bytes: &[u8]) -> io::Result<()> {
        let mut kept = self
            .typing
            .lock()
            .map_err(|_| io::Error::other("ptyd typing lock"))?;
        let reused = kept.as_ref().is_some_and(|(at, _)| at == home);
        if !reused {
            *kept = Some((home.to_path_buf(), typing_to(home)?));
        }
        let Some((_, stream)) = kept.as_mut() else {
            return Err(io::Error::other("no connection to type on"));
        };
        match input(stream, pane, bytes) {
            Ok(()) => Ok(()),
            // A daemon gone since the last keys: nothing reached it, so they
            // go again to the one there now.
            Err(err) if reused && gone(&err) => {
                let mut stream = typing_to(home)?;
                let typed = input(&mut stream, pane, bytes);
                *kept = typed.is_ok().then(|| (home.to_path_buf(), stream));
                typed
            }
            Err(err) => {
                *kept = None;
                Err(err)
            }
        }
    }
}

/// Whether `stream` has something to read within `wait`. An error says yes:
/// the read that follows is what reports it.
fn readable_within(stream: &UnixStream, wait: Duration) -> bool {
    use std::os::unix::io::AsRawFd;
    let mut poll = libc::pollfd {
        fd: stream.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = i32::try_from(wait.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: one pollfd on this stack, for an fd `stream` keeps open.
    unsafe { libc::poll(&mut poll, 1, millis) != 0 }
}

fn typing_to(home: &Path) -> io::Result<UnixStream> {
    let stream = devpit_ptyd::socket::connect(&crate::ptyd_link::socket_path(home))?;
    stream.set_write_timeout(Some(TYPING_WAIT))?;
    Ok(stream)
}

fn input(stream: &mut UnixStream, pane: u32, bytes: &[u8]) -> io::Result<()> {
    for chunk in bytes.chunks(INPUT_CHUNK) {
        frame::write(stream, &Frame::new(pane, Kind::Input, chunk.to_vec()))?;
    }
    Ok(())
}

fn gone(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset | io::ErrorKind::NotConnected
    )
}

/// The app's ears: blocks and notices, the window's events, the claims.
struct App(tauri::AppHandle);

impl Ears for App {
    fn heard(&self, project: &str, leaf: &str, bytes: &[u8]) {
        if let Some(blocks) = tauri::Manager::try_state::<crate::blocks::Blocks>(&self.0) {
            blocks.belongs(leaf, project);
        }
        crate::blocks::heard(&self.0, leaf, bytes);
    }

    fn window_on(&self, leaf: &str) -> bool {
        tauri::Manager::try_state::<crate::sessions::SessionState>(&self.0)
            .is_some_and(|state| state.claims.live(leaf).is_ok())
    }

    fn changed(&self, project: &str) {
        let _ = tauri::Emitter::emit(&self.0, "panes:changed", project);
    }

    fn places(&self) -> HashMap<u32, (String, String)> {
        crate::projects::store()
            .map(|store| crate::ptyd_watch::places(&store))
            .unwrap_or_default()
    }
}

/// Starts the app's tap, on a thread of its own for as long as the app runs.
pub(crate) fn watch(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        if let Ok(home) = devpit_core::Store::root() {
            tap().serve(&home, &App(app));
        }
    });
}

#[cfg(test)]
#[path = "ptyd_tap_tests.rs"]
mod tests;
