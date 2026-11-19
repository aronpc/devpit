//! Terminals on a host: one tmux session of devpit's own there per leaf, on
//! the host's own tmux server (`tmux -L devpit`), driven over its ssh.
//!
//! A leaf on a host is the target `ssh:<host>:<session>`. Its window attaches
//! with `ssh -t`, in a local loop that runs the attach again while ssh says
//! the connection dropped: the session on the host lives on meanwhile.

use std::path::PathBuf;
use std::time::Duration;

use devpit_core::{HostRow, Store};
use devpit_rpc::{ErrorCode, HostTerminal, LayoutNode, RpcError, SessionLayout};

/// What every host target starts with.
pub(crate) const PREFIX: &str = "ssh:";

/// How long a start waits for the host.
const PATIENCE: Duration = Duration::from_secs(25);

/// Seconds between two attaches after a dropped connection.
const RETRY_AFTER: &str = "3";

/// The PATH a non-interactive login on a host often lacks, and `~` read as
/// the login's home, before either script below.
const PRELUDE: &str = "PATH=\"$HOME/.local/bin:$HOME/bin:/usr/local/bin:$PATH\"; p=$1; case $p in \"~\") p=$HOME ;; \"~/\"*) p=$HOME/${p#\"~/\"} ;; esac;";

/// Starts the leaf's session in the folder `$1`, unless it is there: exits 3
/// when the folder is not.
const START: &str = "cd -- \"$p\" || exit 3; tmux -L devpit has-session -t \"=$2\" 2>/dev/null || tmux -L devpit new-session -d -s \"$2\"";

/// Attaches to the leaf's session, made again in the folder when the host
/// lost it.
const ATTACH: &str =
    "cd -- \"$p\" 2>/dev/null || cd; exec tmux -u -L devpit new-session -A -s \"$2\"";

/// Runs the attach in `$2…` again, `$1` seconds apart, while ssh ends with
/// 255: the connection dropped. Any other end is the terminal's own.
const RETRY: &str = "w=$1; shift; while :; do \"$@\"; s=$?; [ \"$s\" -eq 255 ] || exit \"$s\"; printf '\\r\\n[devpit] the connection to the host dropped; reconnecting\\r\\n'; sleep \"$w\"; done";

/// How devpit reaches hosts: the ssh it runs, and where its sockets live.
pub(crate) struct Reach {
    pub ssh: PathBuf,
    pub sockets: PathBuf,
}

impl Reach {
    pub(crate) fn here() -> Result<Self, RpcError> {
        Ok(Self {
            ssh: PathBuf::from("ssh"),
            sockets: crate::host_ssh::sockets().map_err(RpcError::internal)?,
        })
    }

    fn argv(&self, host: &HostRow, words: &[&str], tty: bool) -> Vec<String> {
        let mut argv = match tty {
            true => crate::host_ssh::argv_tty(host, &self.sockets, words),
            false => crate::host_ssh::argv(host, &self.sockets, words),
        };
        argv[0] = self.ssh.display().to_string();
        argv
    }

    /// The tmux server on `host`, driven over its connection.
    pub(crate) fn server(&self, host: &HostRow) -> devpit_tmux::Server {
        let mut via = crate::host_ssh::via(host, &self.sockets, false);
        via[0] = self.ssh.display().to_string();
        devpit_tmux::Server::over(via)
    }
}

pub(crate) fn target(host_id: &str, session: &str) -> String {
    format!("{PREFIX}{host_id}:{session}")
}

/// The host and the session a target names, when it is a host's.
pub(crate) fn parsed(target: &str) -> Option<(&str, &str)> {
    let (host, session) = target.strip_prefix(PREFIX)?.split_once(':')?;
    (!host.is_empty() && !session.is_empty()).then_some((host, session))
}

/// The session a leaf has on a host: one per leaf, so two tabs never share
/// a window there.
pub(crate) fn session_of(project_id: &str, leaf_id: &str) -> String {
    devpit_tmux::Server::session_name(&format!("{project_id}__{leaf_id}"))
}

/// Starts the leaf's session on `host`, in the folder `path` there.
pub(crate) fn started(
    reach: &Reach,
    host: &HostRow,
    path: &str,
    session: &str,
) -> Result<(), RpcError> {
    let script = format!("{PRELUDE} {START}");
    let argv = reach.argv(host, &["sh", "-c", &script, "sh", path, session], false);
    let output = crate::host_ssh::run(&argv, &[], PATIENCE)
        .map_err(|why| RpcError::new(ErrorCode::Busy, format!("{}: {why}", host.name)))?;
    let why = crate::host_ssh::last_line(&output.stderr).unwrap_or_default();
    match output.status.code() {
        Some(0) => Ok(()),
        Some(3) => Err(RpcError::new(
            ErrorCode::NotFound,
            format!("there is no folder {path} on {} any more", host.name),
        )),
        Some(crate::host_ssh::SSH_FAILED) => Err(RpcError::new(
            ErrorCode::Busy,
            format!("ssh could not reach {}: {why}", host.name),
        )),
        _ => Err(RpcError::new(
            ErrorCode::Busy,
            format!("{} could not start a terminal: {why}", host.name),
        )),
    }
}

/// The argv a window's pty runs to attach to `session` on `host`, again
/// `after` seconds once a connection drops.
pub(crate) fn attach_argv_after(
    reach: &Reach,
    host: &HostRow,
    path: &str,
    session: &str,
    after: &str,
) -> Vec<String> {
    let script = format!("{PRELUDE} {ATTACH}");
    let attach = reach.argv(host, &["sh", "-c", &script, "sh", path, session], true);
    ["sh", "-c", RETRY, "devpit-host", after]
        .into_iter()
        .map(str::to_owned)
        .chain(attach)
        .collect()
}

/// The host and the project's folder there, for a host target of `project_id`.
fn reached(store: &Store, project_id: &str, host_id: &str) -> Result<(HostRow, String), RpcError> {
    let host = store
        .hosts()?
        .into_iter()
        .find(|one| one.id == host_id)
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "that host is no longer there"))?;
    let path = store
        .host_links(project_id)?
        .into_iter()
        .find(|one| one.host_id == host_id)
        .map(|one| one.path)
        .ok_or_else(|| {
            RpcError::new(
                ErrorCode::NotFound,
                format!("this project is no longer linked to {}", host.name),
            )
        })?;
    Ok((host, path))
}

/// [`attach_argv_after`] for a leaf of `project_id` on a host.
pub(crate) fn attach_argv(
    project_id: &str,
    host_id: &str,
    session: &str,
) -> Result<Vec<String>, RpcError> {
    let (host, path) = reached(&Store::open_default()?, project_id, host_id)?;
    Ok(attach_argv_after(
        &Reach::here()?,
        &host,
        &path,
        session,
        RETRY_AFTER,
    ))
}

/// The tmux server of the host a target is on, with the target tmux knows
/// there.
pub(crate) fn server_of(target: &str) -> Result<(devpit_tmux::Server, String), RpcError> {
    let (host_id, session) = parsed(target)
        .ok_or_else(|| RpcError::new(ErrorCode::Invalid, "not a terminal on a host"))?;
    let host = Store::open_default()?
        .hosts()?
        .into_iter()
        .find(|one| one.id == host_id)
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "that host is no longer there"))?;
    Ok((Reach::here()?.server(&host), format!("{session}:")))
}

/// Starts a new leaf's session on the host, in the project's folder there,
/// and answers its target.
pub(crate) fn opened_leaf(
    project_id: &str,
    host_id: &str,
    leaf_id: &str,
) -> Result<String, RpcError> {
    let (host, path) = reached(&Store::open_default()?, project_id, host_id)?;
    let session = session_of(project_id, leaf_id);
    started(&Reach::here()?, &host, &path, &session)?;
    Ok(target(&host.id, &session))
}

/// Ends the sessions of host `targets` there, off the caller's thread: a
/// host that does not answer never holds a closing tab.
pub(crate) fn ended(targets: Vec<String>) {
    if targets.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for one in targets {
            if let Ok((server, at)) = server_of(&one) {
                let session = at.trim_end_matches(':');
                let _ = server.kill_session(session);
            }
        }
    });
}

/// `host.terminal_open` — a new tab of the project with one terminal on the
/// host, in the project's folder there.
#[tauri::command]
#[specta::specta]
pub async fn host_terminal_open(
    app: tauri::AppHandle,
    project_id: String,
    host_id: String,
) -> Result<HostTerminal, RpcError> {
    crate::off_main::blocking(move || {
        let tab_id = crate::opening::fresh_tab();
        let leaf_id = format!("leaf_{}", ulid::Ulid::generate());
        let target = opened_leaf(&project_id, &host_id, &leaf_id)?;
        let layout = SessionLayout {
            project_id: project_id.clone(),
            focused_id: leaf_id.clone(),
            tree: LayoutNode::leaf(leaf_id.clone(), target),
        };
        crate::sessions::persist(&Store::open_default()?, &tab_id, &layout)?;
        let _ = tauri::Emitter::emit(
            &app,
            crate::opening::TAB_OPENED,
            serde_json::json!({ "projectId": project_id, "tabId": tab_id, "paneId": leaf_id }),
        );
        Ok(HostTerminal {
            tab_id,
            pane_id: leaf_id,
        })
    })
    .await
}

/// Whether a target is a host's.
pub(crate) fn is_host(target: &str) -> bool {
    parsed(target).is_some()
}

#[cfg(test)]
#[path = "host_panes_tests.rs"]
mod tests;
