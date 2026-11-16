//! The `--open-pane` a launch carries: the pane it asks to be brought
//! forward on. Until the webview has mounted and asked once, every ask —
//! the first launch's and a handover's alike — is kept for it, the last
//! one winning; once it has asked, a handover opens the pane straight
//! away, for by then there is a document to hear the island's event.

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

/// The pane a launch asked to open, kept for the webview to ask for as it
/// mounts: the ask happens before any document exists that could hear the
/// island's event, which is what the keeping is for. Once the webview has
/// asked once, there is nobody left to keep a pane for — a later ask is
/// answered with nothing, and a handover opens the pane itself.
#[derive(Default)]
pub struct PendingPane(Mutex<Ask>);

/// Where an ask stands: kept for a webview that has yet to mount, or spent
/// on one that has. Under the one mutex, so a handover keeping an ask and
/// the webview taking it cannot pass each other by.
enum Ask {
    /// No webview has asked yet; the last kept leaf, if any, waits for it.
    Waiting(Option<String>),
    /// The webview has asked once — a mount, a reload, a remount.
    Mounted,
}

impl Default for Ask {
    fn default() -> Self {
        Self::Waiting(None)
    }
}

impl PendingPane {
    /// What the launch's arguments asked to open, when they name a leaf.
    pub(crate) fn from(argv: &[String]) -> Self {
        let pending = Self::default();
        pending.kept(argv);
        pending
    }

    /// Keep the pane a handover asks to open, for the webview to be given
    /// as it mounts: a later ask replaces an earlier one. True when it was
    /// kept — the webview has not asked yet, and need only be brought
    /// forward — and false, keeping nothing, when the arguments name no
    /// leaf or the webview has already asked.
    pub(crate) fn kept(&self, argv: &[String]) -> bool {
        // The leaf comes from outside the app: only a whole leaf is kept.
        let Some(leaf) = leaf_from(argv) else {
            return false;
        };
        match &mut *self.held() {
            Ask::Mounted => false,
            Ask::Waiting(kept) => {
                *kept = Some(leaf.to_owned());
                true
            }
        }
    }

    /// The pane asked for, once: a second ask — a reload, a remount — is
    /// told there is nothing. The first ask mounts the webview whether
    /// anything was kept for it or not.
    pub(crate) fn take(&self) -> Option<String> {
        match std::mem::replace(&mut *self.held(), Ask::Mounted) {
            Ask::Waiting(kept) => kept,
            Ask::Mounted => None,
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Ask> {
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
