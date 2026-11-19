//! Terminals on a host: one tmux session of devpit's own there per leaf, on
//! the host's own tmux server (`tmux -L devpit`), driven over its ssh.
//!
//! A leaf on a host is the target `ssh:<host>:<session>`. Its window attaches
//! with `ssh -t`, in a local loop that runs the attach again while ssh says
//! the connection dropped: the session on the host lives on meanwhile.

use std::path::PathBuf;

use devpit_core::{HostRow, Store};
use devpit_rpc::{ErrorCode, HostTerminal, LayoutNode, RpcError, SessionLayout};

/// What every host target starts with.
pub(crate) const PREFIX: &str = "ssh:";

/// Seconds between two attaches after a dropped connection.
const RETRY_AFTER: &str = "3";

/// Starts the leaf's session in the folder `$1`, unless it is there: exits 3
/// when the folder is not.
/// The rest of the arguments are tmux's: devpit's options, then the session.
const START: &str = "cd -- \"$p\" || exit 3; s=$2; shift 2; tmux -L devpit has-session -t \"=$s\" 2>/dev/null || tmux -L devpit \"$@\"";

/// Attaches to the leaf's session, made again in the folder when the host
/// lost it.
const ATTACH: &str = "cd -- \"$p\" 2>/dev/null || cd; shift 2; exec tmux -u -L devpit \"$@\"";

/// Runs the attach in `$2…` again, `$1` seconds apart, while ssh ends with
/// 255: the connection dropped. Any other end is the terminal's own.
/// While it waits, Ctrl+C and the like never end it, the wait grows to a
/// minute while the host stays away, and what was typed meanwhile is dropped
/// rather than typed on the host once it is back.
const RETRY: &str = "trap '' INT QUIT TSTP; i=$1; shift; w=$i; while :; do t=$(date +%s); \"$@\"; s=$?; [ \"$s\" -eq 255 ] || exit \"$s\"; [ $(( $(date +%s) - t )) -lt 30 ] || w=$i; printf '\\r\\n[devpit] the connection to the host dropped; reconnecting in %ss\\r\\n' \"$w\"; sleep \"$w\"; o=$(stty -g 2>/dev/null) && stty -icanon min 0 time 0 2>/dev/null && cat >/dev/null; [ -n \"$o\" ] && stty \"$o\" 2>/dev/null; w=$((w * 2)); [ \"$w\" -le 60 ] || w=60; done";

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
    let script = format!(
        "{}; {} {START}",
        devpit_tmux::remote::HOST_PATH,
        crate::host_ssh::FOLDER_IN_P
    );
    let mut words = vec!["sh", "-c", &script, "sh", path, session];
    words.extend(devpit_tmux::remote::quieting());
    words.extend([";", "new-session", "-d", "-s", session]);
    let argv = reach.argv(host, &words, false);
    let output = crate::host_ssh::run(&argv, &[], crate::host_ssh::PATIENCE)
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
    let script = format!(
        "{}; {} {ATTACH}",
        devpit_tmux::remote::HOST_PATH,
        crate::host_ssh::FOLDER_IN_P
    );
    let mut words = vec!["sh", "-c", &script, "sh", path, session];
    words.extend(devpit_tmux::remote::quieting());
    words.extend([";", "new-session", "-A", "-s", session]);
    let attach = reach.argv(host, &words, true);
    ["sh", "-c", RETRY, "devpit-host", after]
        .into_iter()
        .map(str::to_owned)
        .chain(attach)
        .collect()
}

/// The host and the project's folder there, for a host target of `project_id`.
fn reached(store: &Store, project_id: &str, host_id: &str) -> Result<(HostRow, String), RpcError> {
    let host = crate::hosts::host_by_id(store, host_id)?;
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
    let host = crate::hosts::host_by_id(&Store::open_default()?, host_id)?;
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
    crate::off_main::blocking(move || opened_tab(&app, &project_id, &host_id)).await
}

/// A new tab of the project with one terminal on the host, said to the
/// window so it opens.
pub(crate) fn opened_tab(
    app: &tauri::AppHandle,
    project_id: &str,
    host_id: &str,
) -> Result<HostTerminal, RpcError> {
    let tab_id = crate::opening::fresh_tab();
    let leaf_id = format!("leaf_{}", ulid::Ulid::generate());
    let target = opened_leaf(project_id, host_id, &leaf_id)?;
    let layout = SessionLayout {
        project_id: project_id.to_owned(),
        focused_id: leaf_id.clone(),
        tree: LayoutNode::leaf(leaf_id.clone(), target),
    };
    crate::sessions::persist(&Store::open_default()?, &tab_id, &layout)?;
    let _ = tauri::Emitter::emit(
        app,
        crate::opening::TAB_OPENED,
        serde_json::json!({ "projectId": project_id, "tabId": tab_id, "paneId": leaf_id }),
    );
    Ok(HostTerminal {
        tab_id,
        pane_id: leaf_id,
    })
}

/// Whether a target is a host's.
pub(crate) fn is_host(target: &str) -> bool {
    parsed(target).is_some()
}

#[cfg(test)]
#[path = "host_panes_tests.rs"]
mod tests;
