//! The `--open-pane` a launch carries: the pane it asks to be brought
//! forward on. A second launch's arguments reach the running copy, which
//! opens the pane straight away; the first's are kept here until the
//! webview asks for them, once, as it mounts.

use std::sync::Mutex;

use devpit_rpc::{LivePane, RpcError};

/// The leaf `--open-pane` names, when it names one.
pub(crate) fn leaf_from(argv: &[String]) -> Option<&str> {
    // The handover's argv carries the binary's own argv[0].
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--open-pane" {
            // The leaf comes from outside the app: only a whole leaf is one.
            return args
                .next()
                .map(String::as_str)
                .filter(|value| is_leaf(value));
        }
    }
    None
}

/// `leaf_` and a ULID's 26 uppercase letters or digits, nothing around them.
fn is_leaf(value: &str) -> bool {
    value.strip_prefix("leaf_").is_some_and(|body| {
        body.len() == 26
            && body
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    })
}

/// What the `--open-pane` in `argv` opens: the project that holds the leaf,
/// and the leaf. `Ok(None)` when the arguments name no pane, or the leaf is
/// no project's; the holder's word for a store that would not answer.
pub(crate) fn opening<H>(argv: &[String], holder: H) -> Result<Option<(String, String)>, String>
where
    H: FnOnce(&str) -> Result<Option<String>, String>,
{
    let Some(leaf) = leaf_from(argv) else {
        return Ok(None);
    };
    holder(leaf).map(|held| held.map(|project| (project, leaf.to_owned())))
}

/// The pane a first launch asked to open, kept for the webview to ask for
/// as it mounts: the ask happens before any document exists that could
/// hear the island's event, which is what the keeping is for.
#[derive(Default)]
pub struct PendingPane(Mutex<Option<String>>);

impl PendingPane {
    /// What the launch's arguments asked to open, when they name a leaf.
    pub(crate) fn from(argv: &[String]) -> Self {
        let pending = Self::default();
        pending.asked(argv);
        pending
    }

    /// Another ask before the webview has taken one replaces it: the pane
    /// opened is the one asked for last. Arguments that name no leaf are
    /// no ask, and leave what there was.
    pub(crate) fn asked(&self, argv: &[String]) {
        if let Some(leaf) = leaf_from(argv) {
            *self.held() = Some(leaf.to_owned());
        }
    }

    /// The pane asked for, once: a second ask — a reload, a remount — is
    /// told there is nothing.
    pub(crate) fn take(&self) -> Option<String> {
        self.held().take()
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        // Never held across anything that takes time.
        self.0
            .lock()
            .expect("the pending pane is only ever pointed at")
    }
}

/// The pane a pending ask opens: the project that holds its leaf, and the
/// leaf. `Ok(None)` when nothing is pending or the leaf is no project's;
/// the holder's word for a store that would not answer.
pub(crate) fn pending_opening<H>(
    asked: Option<String>,
    holder: H,
) -> Result<Option<LivePane>, String>
where
    H: FnOnce(&str) -> Result<Option<String>, String>,
{
    let Some(leaf) = asked else {
        return Ok(None);
    };
    holder(&leaf).map(|held| {
        held.map(|project_id| LivePane {
            project_id,
            pane_id: leaf,
        })
    })
}

/// `island.pending_pane` — the pane this launch asked to open, for the
/// main webview to open as it mounts, on the same path a click on the
/// island takes. Answered once, so a reload opens it no second time.
#[tauri::command]
#[specta::specta]
pub async fn island_pending_pane(
    pending: tauri::State<'_, PendingPane>,
) -> Result<Option<LivePane>, RpcError> {
    let asked = pending.take();
    crate::off_main::blocking(move || {
        pending_opening(asked, |leaf| {
            crate::sessions::store()
                .map_err(|err| err.to_string())
                .and_then(|store| crate::sessions::project_holding(&store, leaf))
        })
        .map_err(RpcError::internal)
    })
    .await
}

#[cfg(test)]
#[path = "open_pane_arg_tests.rs"]
mod tests;
