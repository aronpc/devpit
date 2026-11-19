//! Settings → Hosts: the host profiles, and what is refused on the way in.

use devpit_core::{HostRow, Store};
use devpit_rpc::{ErrorCode, Host, HostDraft, HostEnvironment, Hosts, RpcError};

/// The longest name, and the longest path either field holds.
const NAME_MOST: usize = 40;
const PATH_MOST: usize = 1024;

/// Why a profile cannot be saved, or nothing when it can. Each field holds
/// only what ssh is given for it, so no value reads as a flag or as shell.
pub(crate) fn refusal(draft: &HostDraft) -> Option<String> {
    let name = draft.name.trim();
    if name.is_empty() || name.chars().count() > NAME_MOST || name.chars().any(char::is_control) {
        return Some(format!(
            "a host's name is 1 to {NAME_MOST} characters on one line"
        ));
    }
    if !word(draft.target.trim(), 253, |ch| {
        ch.is_ascii_alphanumeric() || "._:-".contains(ch)
    }) {
        return Some("the SSH target is an ssh config alias or a host name: letters, digits, '.', '_', ':' and '-', not starting with '-'".to_owned());
    }
    if let Some(user) = &draft.user {
        if !user.trim().is_empty()
            && !word(user.trim(), 32, |ch| {
                ch.is_ascii_alphanumeric() || "._-".contains(ch)
            })
        {
            return Some("the user is a login name: letters, digits, '.', '_' and '-'".to_owned());
        }
    }
    if draft.port == Some(0) {
        return Some("the port is a number from 1 to 65535".to_owned());
    }
    if let Some(file) = &draft.identity_file {
        if !file.trim().is_empty() && !path(file.trim(), false) {
            return Some("the identity file is the path of a key on this computer, starting with / or ~/; devpit never reads it".to_owned());
        }
    }
    folder_refusal(draft.work_root.trim(), "the work root")
}

/// Why `folder`, named `what`, cannot be a folder on a host: it starts at
/// `/` or `~` and holds nothing a shell would read as more than a path.
pub(crate) fn folder_refusal(folder: &str, what: &str) -> Option<String> {
    (!path(folder, true) || folder.chars().any(|ch| SHELL.contains(ch))).then(|| {
        format!("{what} is a folder on the host, starting with / or ~, with no shell characters")
    })
}

/// What a remote shell would read as more than a path.
const SHELL: &str = ";&|<>$`'\"\\(){}[]*?!#";

/// A value ssh takes as one word: no flag, no shell.
fn word(value: &str, most: usize, allowed: impl Fn(char) -> bool) -> bool {
    !value.is_empty()
        && value.len() <= most
        && !value.starts_with('-')
        && value.chars().all(allowed)
}

/// An absolute path, or one from home (`~` alone when `bare_home`).
fn path(value: &str, bare_home: bool) -> bool {
    let rooted = value.starts_with('/') || value.starts_with("~/") || (bare_home && value == "~");
    rooted && value.len() <= PATH_MOST && !value.chars().any(char::is_control)
}

/// An optional field as kept: trimmed, and nothing when empty.
fn kept(value: Option<String>) -> Option<String> {
    value
        .map(|one| one.trim().to_owned())
        .filter(|one| !one.is_empty())
}

pub(crate) fn environment_of(word: &str) -> HostEnvironment {
    match word {
        "production" => HostEnvironment::Production,
        "staging" => HostEnvironment::Staging,
        "dev" => HostEnvironment::Dev,
        // A label it cannot read is taken as the most careful one.
        _ => HostEnvironment::Production,
    }
}

fn word_of(environment: HostEnvironment) -> &'static str {
    match environment {
        HostEnvironment::Dev => "dev",
        HostEnvironment::Staging => "staging",
        HostEnvironment::Production => "production",
    }
}

fn host_of(row: HostRow) -> Host {
    Host {
        environment: environment_of(&row.environment),
        id: row.id,
        name: row.name,
        target: row.target,
        user: row.user,
        port: row.port,
        identity_file: row.identity_file,
        work_root: row.work_root,
    }
}

/// The profile with `id`, or why there is none.
pub(crate) fn host_by_id(store: &Store, id: &str) -> Result<HostRow, RpcError> {
    store
        .hosts()?
        .into_iter()
        .find(|one| one.id == id)
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "that host is no longer there"))
}

pub(crate) fn listed(store: &Store) -> Result<Hosts, RpcError> {
    Ok(Hosts {
        hosts: store.hosts()?.into_iter().map(host_of).collect(),
    })
}

pub(crate) fn saved(store: &Store, draft: HostDraft) -> Result<Hosts, RpcError> {
    if let Some(why) = refusal(&draft) {
        return Err(RpcError::new(ErrorCode::Invalid, why));
    }
    let name = draft.name.trim().to_owned();
    let kept_now = store.hosts()?;
    if let Some(id) = &draft.id {
        if !kept_now.iter().any(|one| &one.id == id) {
            return Err(RpcError::new(
                ErrorCode::NotFound,
                "that host is no longer there",
            ));
        }
    }
    let taken = kept_now
        .into_iter()
        .any(|one| one.name.eq_ignore_ascii_case(&name) && Some(&one.id) != draft.id.as_ref());
    if taken {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            format!("a host is already called {name}"),
        ));
    }
    store.save_host(&HostRow {
        id: draft.id.unwrap_or_else(|| {
            format!("host_{}", ulid::Ulid::generate().to_string().to_lowercase())
        }),
        name,
        target: draft.target.trim().to_owned(),
        user: kept(draft.user),
        port: draft.port,
        identity_file: kept(draft.identity_file),
        work_root: draft.work_root.trim().to_owned(),
        environment: word_of(draft.environment).to_owned(),
    })?;
    listed(store)
}

pub(crate) fn removed(store: &Store, id: &str) -> Result<Hosts, RpcError> {
    store.remove_host(id)?;
    listed(store)
}

/// `hosts.list` — the host profiles, by name.
#[tauri::command]
#[specta::specta]
pub async fn hosts_list() -> Result<Hosts, RpcError> {
    crate::off_main::blocking(|| listed(&Store::open_default()?)).await
}

/// `host.save` — adds a profile, or edits the one with its id.
#[tauri::command]
#[specta::specta]
pub async fn host_save(host: HostDraft) -> Result<Hosts, RpcError> {
    crate::off_main::blocking(move || saved(&Store::open_default()?, host)).await
}

/// `host.remove` — forgets a profile. Nothing on the host is touched.
#[tauri::command]
#[specta::specta]
pub async fn host_remove(id: String) -> Result<Hosts, RpcError> {
    crate::off_main::blocking(move || removed(&Store::open_default()?, &id)).await
}

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;
