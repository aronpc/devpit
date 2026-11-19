//! Project → Hosts: where the project is on each host it is linked to.
//!
//! The path is the person's. devpit checks it is a folder there, over the
//! host's own connection, and never creates or moves anything on a host.

use std::path::Path;
use std::time::Duration;

use devpit_core::{HostRow, Store};
use devpit_rpc::{ErrorCode, HostLink, HostLinks, RpcError};

/// How long a link waits for the host to say whether the folder is there.
const PATIENCE: Duration = Duration::from_secs(25);

/// Whether `$1` is a folder, `~` read as the login's home. The path arrives
/// as an argument, never as part of the script.
/// It answers on stdout, so a banner on stderr never changes the answer.
pub(crate) const FOLDER: &str = "p=$1; case $p in \"~\") p=$HOME ;; \"~/\"*) p=$HOME/${p#\"~/\"} ;; esac; if test -d \"$p\"; then echo yes; else echo no; fi";

/// The project's links, with each host's name and environment.
pub(crate) fn listed(store: &Store, project_id: &str) -> Result<HostLinks, RpcError> {
    let hosts = store.hosts()?;
    let links = store
        .host_links(project_id)?
        .into_iter()
        .filter_map(|link| {
            let host = hosts.iter().find(|one| one.id == link.host_id)?;
            Some(HostLink {
                host_id: link.host_id,
                host_name: host.name.clone(),
                environment: crate::hosts::environment_of(&host.environment),
                path: link.path,
            })
        })
        .collect();
    Ok(HostLinks { links })
}

fn host_named(store: &Store, host_id: &str) -> Result<HostRow, RpcError> {
    store
        .hosts()?
        .into_iter()
        .find(|one| one.id == host_id)
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "that host is no longer there"))
}

/// Links the project to `path` on the host, once the host says it is a
/// folder there; `ssh` and `envs` are the program and variables it runs with.
#[allow(clippy::too_many_arguments)]
pub(crate) fn linked_with(
    ssh: &Path,
    envs: &[(&str, &str)],
    patience: Duration,
    store: &Store,
    sockets: &Path,
    project_id: &str,
    host_id: &str,
    path: &str,
) -> Result<HostLinks, RpcError> {
    let path = path.trim();
    if let Some(why) = crate::hosts::folder_refusal(path, "the project's path") {
        return Err(RpcError::new(ErrorCode::Invalid, why));
    }
    let host = host_named(store, host_id)?;
    let mut argv = crate::host_ssh::argv(&host, sockets, &["sh", "-c", FOLDER, "sh", path]);
    argv[0] = ssh.display().to_string();
    let output = crate::host_ssh::run(&argv, envs, patience)
        .map_err(|why| RpcError::new(ErrorCode::Busy, format!("{}: {why}", host.name)))?;
    let why = crate::host_ssh::last_line(&output.stderr).unwrap_or_default();
    let answer = crate::host_ssh::last_line(&output.stdout);
    // A host's failure is the host's: never devpit's own error, which an
    // error report would carry off the machine with the host's words.
    let failed = |said: String| Err(RpcError::new(ErrorCode::Busy, said));
    match (output.status.code(), answer.as_deref()) {
        (Some(0), Some("yes")) => {}
        (Some(0), Some("no")) => {
            return Err(RpcError::new(
                ErrorCode::NotFound,
                format!("there is no folder {path} on {}", host.name),
            ))
        }
        (Some(crate::host_ssh::SSH_FAILED), _) => {
            return failed(format!("ssh could not reach {}: {why}", host.name))
        }
        _ => return failed(format!("{} could not check {path}: {why}", host.name)),
    }
    store.link_host(project_id, &host.id, path)?;
    listed(store, project_id)
}

pub(crate) fn unlinked(
    store: &Store,
    project_id: &str,
    host_id: &str,
) -> Result<HostLinks, RpcError> {
    store.unlink_host(project_id, host_id)?;
    listed(store, project_id)
}

/// `project.hosts` — the project's links to hosts.
#[tauri::command]
#[specta::specta]
pub async fn project_hosts(project_id: String) -> Result<HostLinks, RpcError> {
    crate::off_main::blocking(move || listed(&Store::open_default()?, &project_id)).await
}

/// `project.host_link` — links the project to a folder on a host, checked
/// there first.
#[tauri::command]
#[specta::specta]
pub async fn project_host_link(
    project_id: String,
    host_id: String,
    path: String,
) -> Result<HostLinks, RpcError> {
    crate::off_main::blocking(move || {
        let sockets = crate::host_ssh::sockets().map_err(RpcError::internal)?;
        linked_with(
            Path::new("ssh"),
            &[],
            PATIENCE,
            &Store::open_default()?,
            &sockets,
            &project_id,
            &host_id,
            &path,
        )
    })
    .await
}

/// `project.host_unlink` — forgets the link; nothing on the host changes.
#[tauri::command]
#[specta::specta]
pub async fn project_host_unlink(
    project_id: String,
    host_id: String,
) -> Result<HostLinks, RpcError> {
    crate::off_main::blocking(move || unlinked(&Store::open_default()?, &project_id, &host_id))
        .await
}

#[cfg(test)]
#[path = "host_links_tests.rs"]
mod tests;
