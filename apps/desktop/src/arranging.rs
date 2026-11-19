//! Changing the shape of the pane tree.
//!
//! Apart from [`crate::sessions`], which opens a session and reads it: a split,
//! a close, a rename and a dragged boundary all rewrite the tree and persist
//! it, and they share the project lock that keeps two windows from arranging
//! the same project at once.

use devpit_rpc::{ErrorCode, LayoutNode, RpcError, SessionLayout, SplitDirection};
use tauri::State;
use ulid::Ulid;

use crate::sessions::{layout_of, locate_cwd, persist, store, tmux_err, tmux_server, SessionState};

/// `session.split` — a new pane and a split node in the tree.
///
/// Off the main thread: it waits on tmux, and a split that held the window
/// still for it looked like a window that had hung.
#[tauri::command]
#[specta::specta]
pub async fn session_split(
    app: tauri::AppHandle,
    project_id: String,
    tab_id: String,
    leaf_id: String,
    direction: SplitDirection,
    worktree_id: Option<String>,
) -> Result<SessionLayout, RpcError> {
    crate::off_main::blocking(move || {
        let state = tauri::Manager::state::<SessionState>(&app);
        session_split_now(&state, project_id, tab_id, leaf_id, direction, worktree_id)
    })
    .await
}

/// [`session_split`], on the calling thread.
pub(crate) fn session_split_now(
    state: &State<SessionState>,
    project_id: String,
    tab_id: String,
    leaf_id: String,
    direction: SplitDirection,
    worktree_id: Option<String>,
) -> Result<SessionLayout, RpcError> {
    let lock = state.project_lock(&project_id)?;
    let _guard = lock
        .lock()
        .map_err(|_| RpcError::internal("project session lock"))?;
    let session = devpit_tmux::Server::session_name(&project_id);

    let current = layout_of(&project_id, &tab_id)?;
    if !current.tree.contains_leaf(&leaf_id) {
        return Err(RpcError::new(
            ErrorCode::NotFound,
            "that pane is not in this project's layout",
        ));
    }
    // A pane on a host splits into another there, in the project's folder.
    let split_from = crate::pane_io::target_in(&current, &leaf_id);
    if let Some((host, _)) = crate::host_panes::parsed(&split_from) {
        let new_id = format!("leaf_{}", Ulid::generate());
        let target = crate::host_panes::opened_leaf(&project_id, host, &new_id)?;
        return split_into(
            &project_id,
            &tab_id,
            current,
            &leaf_id,
            direction,
            &new_id,
            target,
        );
    }
    // Where the pane it splits from stands: a card's tab is in its worktree,
    // and the project root would put the new pane on another branch.
    let stood = crate::pane_io::cwd(&crate::pane_io::target_in(&current, &leaf_id))
        .or_else(|| crate::blocks::last_cwd(state.app(), &project_id, &leaf_id));
    let cwd = match stood.filter(|path| path.is_dir()) {
        Some(path) => path,
        None => locate_cwd(&project_id, worktree_id.as_deref())?,
    };

    let new_id = format!("leaf_{}", Ulid::generate());
    let store = store()?;
    let home = devpit_core::Store::root().ok();
    let target =
        crate::ptyd_link::new_pane(&store, home.as_deref(), &project_id, &new_id, &cwd, || {
            // tmux only now: a tab of ptyd panes may have no session to add to.
            let server = tmux_server()?.with_window_env(crate::checkout::shared_env_at(
                &store,
                &project_id,
                &cwd,
            ));
            let target = devpit_tmux::Server::target(&session, &new_id);
            // Tapped as it is made where there is a session to add it to, so
            // its first prompt, and the folder it announces, are not missed.
            if server.has_session(&session).unwrap_or(false) {
                let made = match state.taps.ahead_of(state.app(), &project_id, &new_id) {
                    Some(pipe) => server.new_window_piped(&session, &new_id, &cwd, &pipe),
                    None => server.new_window(&session, &new_id, &cwd),
                };
                if let Err(err) = made {
                    state.taps.forget(&server, &new_id, &target);
                    return Err(tmux_err(err));
                }
                return Ok(target);
            }
            server
                .ensure_session(&session, &new_id, &cwd)
                .map_err(tmux_err)?;
            // Tapped like every other leaf, or it has no blocks and never says
            // its folder.
            state
                .taps
                .watch(state.app(), &server, &project_id, &new_id, &target);
            Ok(target)
        })?;
    if let Some(blocks) = tauri::Manager::try_state::<crate::blocks::Blocks>(state.app()) {
        blocks.belongs(&new_id, &project_id);
    }
    split_into(
        &project_id,
        &tab_id,
        current,
        &leaf_id,
        direction,
        &new_id,
        target,
    )
}

/// The tree with `leaf_id` split and `new_id` beside it, kept and answered.
fn split_into(
    project_id: &str,
    tab_id: &str,
    current: SessionLayout,
    leaf_id: &str,
    direction: SplitDirection,
    new_id: &str,
    target: String,
) -> Result<SessionLayout, RpcError> {
    let new_leaf = LayoutNode::leaf(new_id.to_owned(), target);
    let tree = current
        .tree
        .split_leaf(leaf_id, direction, new_leaf)
        .ok_or_else(|| RpcError::internal("the leaf vanished while splitting"))?;

    let layout = SessionLayout {
        project_id: project_id.to_owned(),
        focused_id: new_id.to_owned(),
        tree,
    };
    persist(&store()?, tab_id, &layout)?;
    Ok(layout)
}

/// `session.close_leaf` — the pane goes, and its tmux window with it.
///
/// The hole this fills: `session.split` could only ever add. A tree that only
/// grows is a leak wearing a layout's clothes.
///
/// Refuses the last pane. A session with no pane is not a layout, and the
/// refusal says so rather than persisting an empty tree the screen cannot draw.
///
/// Off the main thread, for the same reason as [`session_split`].
#[tauri::command]
#[specta::specta]
pub async fn session_close_leaf(
    app: tauri::AppHandle,
    project_id: String,
    tab_id: String,
    leaf_id: String,
) -> Result<SessionLayout, RpcError> {
    crate::off_main::blocking(move || {
        let state = tauri::Manager::state::<SessionState>(&app);
        session_close_leaf_now(app.clone(), &state, project_id, tab_id, leaf_id)
    })
    .await
}

/// [`session_close_leaf`], on the calling thread.
pub(crate) fn session_close_leaf_now(
    app: tauri::AppHandle,
    state: &State<SessionState>,
    project_id: String,
    tab_id: String,
    leaf_id: String,
) -> Result<SessionLayout, RpcError> {
    let lock = state.project_lock(&project_id)?;
    let _guard = lock
        .lock()
        .map_err(|_| RpcError::internal("project session lock"))?;

    let current = layout_of(&project_id, &tab_id)?;
    if !current.tree.contains_leaf(&leaf_id) {
        return Err(RpcError::new(
            ErrorCode::NotFound,
            "that pane is not in this project's layout",
        ));
    }
    let tree = current.tree.close_leaf(&leaf_id).ok_or_else(|| {
        RpcError::new(
            ErrorCode::Conflict,
            "that is the only pane left — close the project instead",
        )
    })?;

    // The window goes after the tree is known to be closable, so a refusal
    // never leaves a session whose layout and tmux disagree.
    if crate::ptyd_link::leaf_is_ptyd(&current, &leaf_id) {
        crate::ptyd_link::close(std::slice::from_ref(&leaf_id));
        return closed_leaf(app, &project_id, &tab_id, current, &leaf_id, tree);
    }
    let target = crate::pane_io::target_in(&current, &leaf_id);
    if crate::host_panes::is_host(&target) {
        crate::host_panes::ended(vec![target]);
        return closed_leaf(app, &project_id, &tab_id, current, &leaf_id, tree);
    }
    let session = devpit_tmux::Server::session_name(&project_id);
    let server = tmux_server()?;
    // Stop listening before the window goes: the tap holds a fifo and a
    // thread, and a pane that no longer exists has nothing left to say.
    state.taps.forget(
        &server,
        &leaf_id,
        &devpit_tmux::Server::target(&session, &leaf_id),
    );
    crate::tap::stop_whatever_runs(&server, &session, &[leaf_id.as_str()]);
    server.kill_window(&session, &leaf_id).map_err(tmux_err)?;
    closed_leaf(app, &project_id, &tab_id, current, &leaf_id, tree)
}

/// What closing a leaf leaves behind once its terminal is gone, whichever
/// held it: its blocks, its agent, and the tree without it.
fn closed_leaf(
    app: tauri::AppHandle,
    project_id: &str,
    tab_id: &str,
    current: SessionLayout,
    leaf_id: &str,
    tree: LayoutNode,
) -> Result<SessionLayout, RpcError> {
    crate::blocks::closed(&app, project_id, leaf_id);
    // A pane closed on purpose is not one to start an agent in again.
    let _ = store()?.forget_pane_agent(leaf_id);
    crate::card_activity::panes_closed(&app, &[leaf_id.to_owned()]);

    // Focus follows the tree when it pointed at what just left.
    let focused_id = if current.focused_id == leaf_id {
        tree.first_leaf_id().to_owned()
    } else {
        current.focused_id
    };
    let layout = SessionLayout {
        project_id: project_id.to_owned(),
        focused_id,
        tree,
    };
    persist(&store()?, tab_id, &layout)?;
    Ok(layout)
}

/// `session.set_ratio` — where a boundary was dragged to.
///
/// Persisted because Orca's rule is the right one: boundaries stay where you
/// put them, and resizing the window does not shuffle a layout someone
/// arranged. The tree clamps, so neither side can be dragged out of reach.
#[tauri::command]
#[specta::specta]
pub async fn session_set_ratio(
    app: tauri::AppHandle,
    project_id: String,
    tab_id: String,
    split_id: String,
    ratio: f64,
) -> Result<SessionLayout, RpcError> {
    crate::off_main::blocking(move || {
        let state = tauri::Manager::state::<SessionState>(&app);
        session_set_ratio_now(state, project_id, tab_id, split_id, ratio)
    })
    .await
}

/// [`session_set_ratio`], on the calling thread.
pub(crate) fn session_set_ratio_now(
    state: State<SessionState>,
    project_id: String,
    tab_id: String,
    split_id: String,
    ratio: f64,
) -> Result<SessionLayout, RpcError> {
    let lock = state.project_lock(&project_id)?;
    let _guard = lock
        .lock()
        .map_err(|_| RpcError::internal("project session lock"))?;

    let current = layout_of(&project_id, &tab_id)?;
    let tree = current.tree.set_ratio(&split_id, ratio).ok_or_else(|| {
        RpcError::new(
            ErrorCode::NotFound,
            "no such boundary in this project's layout",
        )
    })?;
    let layout = SessionLayout {
        project_id,
        focused_id: current.focused_id,
        tree,
    };
    persist(&store()?, &tab_id, &layout)?;
    Ok(layout)
}

/// `session.close_tab` — the tab goes, and every window in its tree with it.
///
/// A tab owns a tree, so closing one is not closing a leaf: leaving the rest
/// running would leave shells nothing can reach again, which is the shape the
/// scrollback and the layout both keyed on.
#[tauri::command]
#[specta::specta]
pub async fn session_close_tab(
    app: tauri::AppHandle,
    project_id: String,
    tab_id: String,
) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        let state = tauri::Manager::state::<SessionState>(&app);
        session_close_tab_now(app.clone(), state, project_id, tab_id)
    })
    .await
}

/// [`session_close_tab`], on the calling thread.
pub(crate) fn session_close_tab_now(
    app: tauri::AppHandle,
    state: State<SessionState>,
    project_id: String,
    tab_id: String,
) -> Result<(), RpcError> {
    let closed = close_tab(&state, &project_id, &tab_id)?;
    crate::card_activity::panes_closed(&app, &closed);
    Ok(())
}

/// The body of `session.close_tab`, for a card that takes its own tab with it.
/// Answers the leaves it closed.
pub(crate) fn close_tab(
    state: &SessionState,
    project_id: &str,
    tab_id: &str,
) -> Result<Vec<String>, RpcError> {
    let lock = state.project_lock(project_id)?;
    let _guard = lock
        .lock()
        .map_err(|_| RpcError::internal("project session lock"))?;

    // A tab with no tree is already closed. Saying so is not an error: the
    // window asks on every close, including ones that never opened a session.
    let Ok(layout) = layout_of(project_id, tab_id) else {
        return Ok(Vec::new());
    };
    let store = store()?;
    let home = devpit_core::Store::root().ok();
    let closed = end_tab_at(&store, home.as_deref(), &layout, tab_id, |leaves| {
        close_tmux_leaves(state, project_id, leaves)
    })?;
    for leaf_id in &closed {
        crate::blocks::closed(state.app(), project_id, leaf_id);
    }
    Ok(closed)
}

/// Ends every leaf of a tab's `layout` and forgets the tab. The daemon's
/// panes are ended at `home`; the rest go to `in_tmux`, which is only asked
/// when there are any, so a tab of ptyd panes closes with no tmux at all.
pub(crate) fn end_tab_at(
    store: &devpit_core::Store,
    home: Option<&std::path::Path>,
    layout: &SessionLayout,
    tab_id: &str,
    in_tmux: impl FnOnce(&[&str]),
) -> Result<Vec<String>, RpcError> {
    let closed: Vec<String> = layout
        .tree
        .leaves()
        .into_iter()
        .map(|(leaf, _)| leaf.to_owned())
        .collect();
    let ptyd = crate::ptyd_link::ptyd_leaves(layout);
    crate::ptyd_link::close_at(home, &ptyd);
    let on_hosts: Vec<(String, String)> = layout
        .tree
        .leaves()
        .into_iter()
        .filter(|(_, target)| crate::host_panes::is_host(target))
        .map(|(leaf, target)| (leaf.to_owned(), target.to_owned()))
        .collect();
    crate::host_panes::ended(on_hosts.iter().map(|(_, target)| target.clone()).collect());
    let leaves: Vec<&str> = closed
        .iter()
        .filter(|leaf| !ptyd.contains(leaf) && !on_hosts.iter().any(|(one, _)| one == *leaf))
        .map(String::as_str)
        .collect();
    if !leaves.is_empty() {
        in_tmux(&leaves);
    }
    for leaf_id in &closed {
        let _ = store.forget_pane_agent(leaf_id);
    }
    store.forget_pane_layout(&layout.project_id, tab_id)?;
    Ok(closed)
}

/// Ends a tab's tmux windows. With no tmux to ask, they are not running.
fn close_tmux_leaves(state: &SessionState, project_id: &str, leaves: &[&str]) {
    let Ok(server) = tmux_server() else {
        return;
    };
    let session = devpit_tmux::Server::session_name(project_id);
    for leaf_id in leaves {
        let target = devpit_tmux::Server::target(&session, leaf_id);
        state.taps.forget(&server, leaf_id, &target);
    }
    crate::tap::stop_whatever_runs(&server, &session, leaves);
    for leaf_id in leaves {
        let _ = server.kill_window(&session, leaf_id);
    }
}

#[cfg(test)]
#[path = "arranging_tests.rs"]
mod tests;
