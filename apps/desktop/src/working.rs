//! Carrying out one step, on the thread that owns it.
//!
//! Apart from `runs.rs` because that file opens the row and hands it over, and
//! this is the work itself — which is most of the volume and none of the
//! bookkeeping.
//!
//! Everything here happens off the thread that draws. A step is a process, or
//! several; doing it inline would freeze the drag that started it for exactly
//! as long, and a board that locks up while it thinks is a board nobody drags
//! things onto.

use std::sync::Arc;

use devpit_core::Store;
use devpit_rpc::{Step, StepKind};
use tauri::{AppHandle, Emitter};

use crate::card_activity::{run_heard, run_reference, state_of_run};
use crate::in_flight::InFlight;
use crate::{chaining, notices, steps};

/// Everything the thread needs, taken by value before it starts.
pub struct Carrying {
    pub app: AppHandle,
    pub in_flight: Arc<InFlight>,
    pub chained: Arc<InFlight>,
    pub run_id: String,
    pub card_id: String,
    /// The session an agent or session step speaks in, new for this run.
    pub session_id: String,
    pub step: Step,
    pub fixed_agent: Option<crate::run_start::FixedAgent>,
    pub hops: u8,
}

/// Runs the step, records how it ended, and lets the board react.
pub fn carry_out(carrying: Carrying, store: &Store) {
    let Carrying {
        app,
        in_flight,
        chained,
        run_id: id,
        card_id: card,
        session_id,
        step,
        fixed_agent,
        hops,
    } = carrying;

    // Both kinds that run a process here register it, so the card's stop and an
    // update told to stop the work reach either one.
    let watching = Arc::clone(&in_flight);
    let watched = id.clone();

    // The card shows work as it happens rather than a spinner that ends in a
    // wall of text. Both kinds that produce output get this: a command step
    // went without it for a year while the docs promised otherwise, which is
    // the whole reason it is built once here rather than twice below.
    let progress = app.clone();
    let run = id.clone();
    let on_progress = move |text: &str| {
        let _ = progress.emit("run:progress", (run.clone(), text.to_owned()));
    };

    let outcome = match step.kind {
        StepKind::Agent => steps::agent::run(
            store,
            &card,
            &step,
            steps::agent::Starting {
                run_id: &id,
                session_id: &session_id,
                fixed: fixed_agent.as_ref(),
            },
            on_progress,
            |pid| watching.watch(&watched, pid),
        ),
        StepKind::Session => steps::session::start(&app, store, &card, &step, &session_id),
        StepKind::Command => steps::command::run(store, &card, &step, &id, on_progress, |pid| {
            watching.watch(&watched, pid)
        }),
    };

    // On the way to disk, not on the way to the window: a line that already
    // reached the screen cannot be unseen, and this is the copy that gets read
    // back tomorrow. It hides what devpit handed out, and nothing else.
    let secrets = crate::kept_out::what_devpit_gave(store);
    let outcome = match outcome {
        Ok(finished) => Ok(crate::steps::Finished {
            output: crate::kept_out::kept_out(&finished.output, &secrets),
            ..finished
        }),
        Err(reason) => Err(crate::kept_out::kept_out(&reason, &secrets)),
    };

    let answered = match &outcome {
        Ok(finished) if finished.ok => Some(finished.output.clone()),
        _ => None,
    };
    let ended = end_state(in_flight.was_cancelled(&id), answered.is_some());
    let closed = match outcome {
        Ok(finished) => store.finish_run(
            &id,
            ended,
            Some(&finished.output),
            Some(finished.cost_usd),
            Some(finished.duration_ms),
            finished.exit_code.map(i64::from),
        ),
        Err(reason) => store.finish_run(&id, ended, Some(&reason), None, None, None),
    };

    // A failure to record is worth saying out loud: the run finished and
    // the screen would otherwise show it running forever.
    match closed {
        Err(err) => devpit_core::reports::background(
            "run registration",
            &format!("could not record the end of run {id}: {err}"),
        ),
        // Closed already: a person stopped it, and the card heard that then.
        Ok(false) => {}
        Ok(true) => run_heard(&app, &card, &run_reference(store, &id), state_of_run(ended)),
    }
    in_flight.forget(&id);

    // The bell. A run ending is the thing people leave the window for,
    // so it is the first thing the bell had to know. Only the ending —
    // starting one was a click they had just made.
    notices::run_ended(&app, store, &card, &step.name, answered.is_some());

    // What the board does about it. Backward on a refusal, forward on a
    // pass when the lane says so and is allowed to — see `advancing`.
    chaining::after(
        &app,
        &chained,
        store,
        &card,
        &step,
        answered.as_deref(),
        hops,
    );
    let _ = app.emit("run:changed", &card);
}

/// The thread's own store, or nothing and the run closed as failed.
///
/// Decided before the thread starts: a thread that could not open the store
/// had nothing to close the row with, and the run read `running` for ever.
pub fn store_for_thread(
    store: &Store,
    run_id: &str,
    opened: Result<Store, devpit_core::store::StoreError>,
) -> Option<Store> {
    let err = match opened {
        Ok(own) => return Some(own),
        Err(err) => err,
    };
    let why = format!("could not open the store to carry out this run: {err}");
    if let Err(err) = store.finish_run(run_id, "failed", Some(&why), None, None, None) {
        devpit_core::reports::background(
            "run registration",
            &format!("could not record the end of run {run_id}: {err}"),
        );
    }
    None
}

/// How a run ended, as its row says it. A person's stop wins over whatever the
/// killed process looked like on its way out.
fn end_state(cancelled: bool, ok: bool) -> &'static str {
    match (cancelled, ok) {
        (true, _) => "cancelled",
        (false, true) => "ok",
        (false, false) => "failed",
    }
}

#[cfg(test)]
#[path = "working_tests.rs"]
mod tests;
