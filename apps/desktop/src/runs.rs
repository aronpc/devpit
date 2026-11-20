//! Starting a step, and recording what it did.
//!
//! A run is opened before the work begins and closed when it ends, so a card
//! never has work happening with nothing on screen to show for it. When the
//! process dies mid-run the row stays `running` and says so — which is honest,
//! and better than a row that silently reports success.
//!
//! **The work happens off the command's thread.** A turn takes seconds; doing
//! it inline would freeze the drag that started it for exactly as long, and a
//! board that locks up while it thinks is a board nobody drags things onto.
//! The command returns a `running` row and the thread finishes it, announcing
//! itself on `run:changed` so the card can catch up.
//!
//! Nothing here retries. A failed run leaves the card where it is with the
//! reason on it, and the next move is a person's.

use std::sync::Arc;

use devpit_core::Store;
use devpit_rpc::{RpcError, Run, Step};
use tauri::AppHandle;

use crate::in_flight::InFlight;
use crate::run_from::Asking;

/// Runs a step against a card, and returns the run it opened.
pub fn start(
    app: AppHandle,
    in_flight: Arc<InFlight>,
    store: &Store,
    card_id: &str,
    step: &Step,
    from: Asking<'_>,
) -> Result<Run, RpcError> {
    start_chained(app, in_flight, store, card_id, step, from)
}

/// The same, for a run the chain is carrying on rather than a person starting.
pub fn start_chained(
    app: AppHandle,
    in_flight: Arc<InFlight>,
    store: &Store,
    card_id: &str,
    step: &Step,
    from: Asking<'_>,
) -> Result<Run, RpcError> {
    crate::run_launch::starting_refused(&app)?;
    let begun = crate::run_start::prepare(store, card_id, step, from.came_from)?;
    crate::run_launch::launch(app, in_flight, store, card_id, step, from, begun)
}

#[cfg(test)]
#[path = "runs_tests.rs"]
mod tests;
