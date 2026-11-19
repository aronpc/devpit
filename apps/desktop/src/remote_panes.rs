//! The terminals a viewer watches: a tmux pane through a tmux client of its
//! own, a ptyd pane over the daemon's socket. A ptyd watcher never resizes
//! (a phone must not shrink the desk's) nor acks ([`crate::ptyd_tap`] does).

use std::collections::HashMap;
use std::sync::mpsc::{Sender, SyncSender};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use devpit_rpc::RemoteOut;
use portable_pty::{CommandBuilder, PtySize};

/// The most terminals one connection watches at once.
const MOST: usize = 4;
/// Raw reads queued for a viewer that has not taken them: past it, a viewer
/// too slow for its pane has the watch closed rather than memory fill up.
pub(crate) const MOST_RAW_QUEUED: usize = 256;

/// A terminal's bytes for a viewer that reads them raw, under the handle it
/// gave the pane.
pub(crate) struct Raw {
    pub handle: u32,
    pub bytes: Vec<u8>,
}

/// Where a watched pane's bytes go: base64 in JSON for a viewer that reads
/// only that, raw for one that gave the pane a handle.
#[derive(Clone)]
pub(crate) enum Sink {
    Json(Sender<RemoteOut>),
    Raw(SyncSender<Raw>, u32),
}

impl Sink {
    /// False once the connection is gone, or too far behind a raw pane.
    pub(crate) fn bytes(&self, pane: &str, bytes: Vec<u8>) -> bool {
        match self {
            Sink::Json(out) => out
                .send(RemoteOut::PaneBytes {
                    pane: pane.to_owned(),
                    b64: BASE64.encode(&bytes),
                })
                .is_ok(),
            Sink::Raw(out, handle) => out
                .try_send(Raw {
                    handle: *handle,
                    bytes,
                })
                .is_ok(),
        }
    }
}

/// The panes one connection has open.
#[derive(Default)]
pub(crate) struct Watching {
    viewer: String,
    open: HashMap<String, Open>,
    /// Set for a connection that carries raw bytes.
    pub(crate) raw: Option<SyncSender<Raw>>,
    /// The pane each raw handle names.
    handles: HashMap<u32, String>,
}

enum Open {
    Tmux {
        session: String,
        pty: devpit_pty::Session,
    },
    #[cfg(unix)]
    Ptyd {
        home: std::path::PathBuf,
        number: u32,
        stream: std::os::unix::net::UnixStream,
    },
}

impl Watching {
    pub(crate) fn new(viewer: u64) -> Watching {
        Watching {
            viewer: viewer.to_string(),
            open: HashMap::new(),
            raw: None,
            handles: HashMap::new(),
        }
    }

    /// The sink for a pane the viewer gave `handle`, or none.
    pub(crate) fn sink(&self, out: &Sender<RemoteOut>, handle: Option<u32>) -> Sink {
        match (handle, &self.raw) {
            (Some(handle), Some(raw)) => Sink::Raw(raw.clone(), handle),
            _ => Sink::Json(out.clone()),
        }
    }

    /// Starts watching `pane` of `project`, typing only when `typing`.
    pub(crate) fn open(
        &mut self,
        project: &str,
        pane: &str,
        typing: bool,
        size: (u16, u16),
        out: Sender<RemoteOut>,
        handle: Option<u32>,
    ) -> Result<(), String> {
        self.close(pane);
        let sink = self.sink(&out, handle);
        let raw = match &sink {
            Sink::Raw(_, handle) => Some(*handle),
            Sink::Json(_) => None,
        };
        self.watch(project, pane, typing, size, sink, out)?;
        if let Some(handle) = raw {
            self.handles.insert(handle, pane.to_owned());
        }
        Ok(())
    }

    fn watch(
        &mut self,
        project: &str,
        pane: &str,
        typing: bool,
        size: (u16, u16),
        sink: Sink,
        out: Sender<RemoteOut>,
    ) -> Result<(), String> {
        if self.open.len() >= MOST {
            return Err(format!("at most {MOST} terminals at once"));
        }
        #[cfg(unix)]
        if let Some(number) = ptyd_number(project, pane) {
            let home = devpit_core::Store::root().map_err(|err| err.to_string())?;
            return self.open_ptyd_at(&home, pane, number, sink, out);
        }
        let server = crate::sessions::tmux_server().map_err(|err| err.message)?;
        let session = devpit_tmux::Server::session_name(project);
        let running = server.running(&session).map_err(|err| err.to_string())?;
        if !running.iter().any(|one| one.leaf_id == pane) {
            return Err("that terminal is not open".to_owned());
        }
        let argv = server
            .viewer_argv(&session, pane, &self.viewer, typing)
            .map_err(|err| err.to_string())?;
        let mut command = CommandBuilder::new(&argv[0]);
        command.args(&argv[1..]);
        devpit_pty::host_env::scrub_pty(&mut command);
        command.env("TERM", "xterm-256color");
        let size = PtySize {
            cols: size.0.clamp(20, 400),
            rows: size.1.clamp(5, 200),
            pixel_width: 0,
            pixel_height: 0,
        };
        let mut pty = devpit_pty::spawn(command, size).map_err(|err| err.to_string())?;
        let (_, empty) = tokio::sync::mpsc::channel(1);
        let mut frames = std::mem::replace(&mut pty.frames, empty);
        let named = pane.to_owned();
        std::thread::spawn(move || {
            while let Some(bytes) = frames.blocking_recv() {
                if !sink.bytes(&named, bytes) {
                    break;
                }
            }
            let _ = out.send(RemoteOut::PaneClosed { pane: named });
        });
        self.open
            .insert(pane.to_owned(), Open::Tmux { session, pty });
        Ok(())
    }

    /// Watches ptyd pane `number`, leaf `pane`, of the daemon of `home`:
    /// attached, and asked nothing else.
    #[cfg(unix)]
    pub(crate) fn open_ptyd_at(
        &mut self,
        home: &std::path::Path,
        pane: &str,
        number: u32,
        sink: Sink,
        out: Sender<RemoteOut>,
    ) -> Result<(), String> {
        use devpit_ptyd::frame::{self, Frame, Kind};
        let socket = crate::ptyd_link::socket_path(home);
        let mut stream = devpit_ptyd::socket::connect(&socket).map_err(|err| {
            format!("devpit-ptyd, which holds that terminal, did not answer: {err}")
        })?;
        let reading = stream.try_clone().map_err(|err| err.to_string())?;
        frame::write(&mut stream, &Frame::new(number, Kind::Attach, Vec::new()))
            .map_err(|err| err.to_string())?;
        let named = pane.to_owned();
        std::thread::spawn(move || forward(reading, number, &named, &sink, &out));
        let home = home.to_path_buf();
        self.open.insert(
            pane.to_owned(),
            Open::Ptyd {
                home,
                number,
                stream,
            },
        );
        Ok(())
    }

    /// Keys typed into a watched pane. Refused by tmux itself when the
    /// viewer was attached read-only.
    pub(crate) fn input(&mut self, pane: &str, bytes: &[u8]) -> Result<(), String> {
        let open = self
            .open
            .get_mut(pane)
            .ok_or("that terminal is not open here")?;
        match open {
            Open::Tmux { pty, .. } => pty.write(bytes).map_err(|err| err.to_string()),
            #[cfg(unix)]
            Open::Ptyd { home, number, .. } => {
                crate::pane_io::write_at(home, *number, bytes).map_err(|err| err.message)
            }
        }
    }

    /// The pane a viewer's raw handle names.
    pub(crate) fn pane_of(&self, handle: u32) -> Option<&str> {
        self.handles.get(&handle).map(String::as_str)
    }

    pub(crate) fn close(&mut self, pane: &str) {
        self.handles.retain(|_, named| named != pane);
        match self.open.remove(pane) {
            Some(Open::Tmux { session, pty }) => {
                if let Ok(server) = crate::sessions::tmux_server() {
                    server.end_viewer(&session, pane, &self.viewer);
                }
                drop(pty);
            }
            // Detached, then closed: the forwarding thread hears the end
            // and tells the viewer the pane is closed, as a tmux one's does.
            #[cfg(unix)]
            Some(Open::Ptyd {
                number, mut stream, ..
            }) => {
                use devpit_ptyd::frame::{self, Frame, Kind};
                let _ = frame::write(&mut stream, &Frame::new(number, Kind::Detach, Vec::new()));
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
            None => {}
        }
    }

    pub(crate) fn close_all(&mut self) {
        let panes: Vec<String> = self.open.keys().cloned().collect();
        for pane in panes {
            self.close(&pane);
        }
    }
}

/// The daemon's number for `pane` of `project`, when the daemon holds it.
#[cfg(unix)]
fn ptyd_number(project: &str, pane: &str) -> Option<u32> {
    let target = crate::pane_io::target_of(project, pane).ok()?;
    match crate::pane_io::holder(&target) {
        crate::pane_io::Holder::Ptyd(number) => Some(number),
        crate::pane_io::Holder::Tmux { .. } | crate::pane_io::Holder::Host(_) => None,
    }
}

/// Pane `number`'s frames to the viewer as `pane`'s bytes, in the shapes a
/// tmux pane's take, until it ends or the connection does. Nothing goes back.
#[cfg(unix)]
pub(crate) fn forward(
    mut reading: impl std::io::Read,
    number: u32,
    pane: &str,
    sink: &Sink,
    out: &Sender<RemoteOut>,
) {
    use devpit_ptyd::frame::{self, Kind};
    while let Ok(Some(one)) = frame::read(&mut reading) {
        if one.pane != number {
            continue;
        }
        match one.kind() {
            Some(Kind::Output | Kind::Replay) => {
                if !sink.bytes(pane, one.bytes) {
                    break;
                }
            }
            Some(Kind::Exited | Kind::Refused) => break,
            _ => {}
        }
    }
    let _ = out.send(RemoteOut::PaneClosed {
        pane: pane.to_owned(),
    });
}

#[cfg(all(test, unix))]
#[path = "remote_panes_tests.rs"]
mod tests;
