//! A pane opened in tmux: how a terminal is reached over ssh.
//!
//! A tmux pane already is one, and its attach line is all there is to say. A
//! ptyd pane is shown in a tmux session of its own running `devpit
//! ptyd-attach <leaf>`, on devpit's tmux server, made once and reused.

use std::path::Path;

use devpit_rpc::{ErrorCode, PaneInTmux, RpcError};

use crate::pane_io::Holder;

/// What a view session's name starts with. Never `devpit_`: that prefix is a
/// project's, and the pane snapshot, the session list and the leftovers each
/// read every session that has it as one of its own.
pub(crate) const VIEW_PREFIX: &str = "ptyd-view_";

/// The tmux session ptyd pane `leaf` is viewed through.
pub(crate) fn view_session(leaf: &str) -> String {
    let mut name = String::from(VIEW_PREFIX);
    for ch in leaf.chars() {
        let kept = ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
        name.push(if kept { ch } else { '_' });
    }
    name.truncate(120);
    name
}

/// The line that attaches to `target` on the tmux server at `socket`, each
/// word quoted only where a shell would read it as more than a word.
pub(crate) fn attach_line(socket: &Path, target: &str) -> String {
    let socket = socket.display().to_string();
    ["tmux", "-S", &socket, "attach", "-t", target]
        .iter()
        .map(|word| shell_word(word))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "_-./:=@%+,".contains(ch));
    match plain {
        true => word.to_owned(),
        false => devpit_agentcli::declaring::quoted(word),
    }
}

/// What the attach client needs to find this devpit's daemon, from a tmux
/// server that may have been started with another environment.
fn view_env() -> Vec<(String, String)> {
    ["DEVPIT_HOME", "XDG_RUNTIME_DIR"]
        .into_iter()
        .filter_map(|key| {
            let value = std::env::var(key).ok().filter(|value| !value.is_empty())?;
            Some((key.to_owned(), value))
        })
        .collect()
}

/// `pane.open_in_tmux` — a pane made reachable from tmux, and the line that
/// attaches to it.
#[tauri::command]
#[specta::specta]
pub async fn pane_open_in_tmux(
    project_id: String,
    pane_id: String,
) -> Result<PaneInTmux, RpcError> {
    crate::off_main::blocking(move || open_in_tmux(&project_id, &pane_id)).await
}

fn open_in_tmux(project_id: &str, pane_id: &str) -> Result<PaneInTmux, RpcError> {
    let target = crate::pane_io::target_of(project_id, pane_id)?;
    let server = crate::sessions::tmux_server()?;
    let session = match crate::pane_io::holder(&target) {
        Holder::Tmux { .. } => target.clone(),
        Holder::Host(on_host) => {
            let session = crate::host_panes::parsed(on_host).map_or("", |(_, session)| session);
            return Err(RpcError::new(
                    ErrorCode::Unsupported,
                    format!("this terminal runs on a host: reach it there with ssh and `tmux -L devpit attach -t ={session}`"),
                ));
        }
        Holder::Ptyd(_) => {
            let exe = crate::agent_reach::exe().ok_or_else(|| {
                RpcError::new(ErrorCode::Unsupported, "devpit cannot find its own binary")
            })?;
            let session = view_session(pane_id);
            let argv = [
                exe.display().to_string(),
                "ptyd-attach".to_owned(),
                pane_id.to_owned(),
            ];
            server
                .ensure_view(&session, &view_env(), &argv)
                .map_err(|err| RpcError::internal(err.to_string()))?;
            session
        }
    };
    let attach = attach_line(&crate::sessions::socket_path()?, &session);
    Ok(PaneInTmux { session, attach })
}

/// `devpit ptyd-attach <leaf>`: the leaf's pane on this terminal, from the
/// daemon of this devpit's home. Never starts one: a daemon that is not there
/// holds no pane to show.
#[cfg(unix)]
pub(crate) fn attach_here(leaf: Option<&str>) -> i32 {
    let Some(leaf) = leaf.filter(|leaf| !leaf.is_empty()) else {
        eprintln!("usage: devpit ptyd-attach <leaf>");
        return 2;
    };
    let home = match devpit_core::Store::root() {
        Ok(home) => home,
        Err(err) => {
            eprintln!("devpit: cannot find where devpit keeps its state: {err}");
            return 1;
        }
    };
    devpit_ptyd::attach::run(
        &crate::ptyd_link::socket_path(&home),
        crate::ptyd_link::pane_number(leaf),
    )
}

#[cfg(not(unix))]
pub(crate) fn attach_here(_leaf: Option<&str>) -> i32 {
    eprintln!("devpit ptyd-attach runs on Linux and macOS only");
    2
}

#[cfg(test)]
#[path = "pane_tmux_tests.rs"]
mod tests;
