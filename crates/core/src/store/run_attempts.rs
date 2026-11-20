//! Attempts: a step's tries on a card, numbered and chained.
//!
//! A step can be tried more than once on a card: an account runs dry and the
//! run pauses, a person switches to another and it goes again, a project with
//! the option on switches on its own. Each try is an attempt — the first is
//! 1, a continuation is the last one's number plus one, and the chain is a
//! straight line: one attempt, one successor.
//!
//! Which transition an ask is — a fresh `start` or a `retry` of the last
//! attempt — is decided here, against the history only the store holds. The
//! callers say what happened; they do not say what it means, because the same
//! play is a `start` after an `ok` and a `retry` after a `lost`, and no screen
//! should have to know which.
//!
//! Apart from `runs.rs`, which is already the shape of a run's life; this is
//! the shape of its tries.

use rusqlite::{OptionalExtension, Transaction};

use crate::store::{Store, StoreError};

/// What asks for the next attempt, as the person or the project sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunAsk {
    /// The step was asked for: a drag, a play, the auto chain.
    Step { card: String, step: String },
    /// A move away from a paused attempt, to what the caller carries: chosen
    /// by a person's click, or by the project's own option.
    Switch { from: String, how: SwitchKind },
}

/// How a switch away from a pause came about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchKind {
    /// A person chose another account or model on the pause.
    Click,
    /// The project's own option moved it on.
    Automatic,
}

/// What a new attempt runs under, when the caller is the one choosing: the
/// profile snapshot and the model asked for.
///
/// A `retry` carries the last attempt's own instead, because a retry is the
/// same account trying again — what a caller passed for a start was never
/// meant for the retry the history turned it into.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttemptCarry {
    pub pinned: Option<String>,
    pub requested_model: Option<String>,
}

/// One attempt's facts, read back by run id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRow {
    pub id: String,
    pub state: String,
    pub attempt: Option<i64>,
    pub previous_attempt: Option<String>,
    pub cause: Option<String>,
    pub paused_because: Option<String>,
    pub pinned: Option<String>,
    pub requested_model: Option<String>,
    pub effective_model: Option<String>,
}

/// The transition the history said an ask was.
struct Transition {
    card: String,
    step: String,
    cause: &'static str,
    attempt: i64,
    previous: Option<String>,
    pinned: Option<String>,
    requested_model: Option<String>,
}

impl Store {
    /// Begins the next attempt of a step on a card, deciding the transition.
    ///
    /// The decision and the row it writes share one transaction, so another
    /// ask landing between them cannot read the history the decision was made
    /// against. `blocked`, when the start's own check refused it, births the
    /// run `paused` with that reason instead of `running` — the transition it
    /// was is kept, because a pause is a state, not a different event.
    pub fn begin_attempt(
        &self,
        ask: RunAsk,
        from_column: Option<&str>,
        carry: AttemptCarry,
        blocked: Option<&str>,
    ) -> Result<String, StoreError> {
        let tx = self.writing()?;
        let transition = match ask {
            RunAsk::Step { card, step } => asked(&tx, &card, &step, &carry)?,
            RunAsk::Switch { from, how } => switched(&tx, &from, how, &carry)?,
        };
        // The chain is a line: whatever named an attempt to continue, the
        // database holds the one-successor rule (`run_one_successor`) and
        // this check turns it into an answer rather than a constraint error.
        if let Some(previous) = &transition.previous {
            let taken: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM run WHERE previous_attempt = ?1)",
                [previous],
                |row| row.get(0),
            )?;
            if taken {
                return Err(StoreError::AlreadyContinued);
            }
        }

        let id = format!("run_{}", ulid::Ulid::generate());
        let state = if blocked.is_some() {
            "paused"
        } else {
            "running"
        };
        let inserted = tx.execute(
            "INSERT INTO run (id, card_id, step_id, state, started_at, from_column, \
             pinned, requested_model, attempt, previous_attempt, cause, paused_because) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                id,
                transition.card,
                transition.step,
                state,
                super::runs::now(),
                from_column,
                transition.pinned,
                transition.requested_model,
                transition.attempt,
                transition.previous,
                transition.cause,
                blocked
            ],
        );
        match inserted {
            // Only the card's one-run index can fire here: the successor rule
            // was checked above, inside this same write-locked transaction.
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                Err(StoreError::AlreadyRunning)
            }
            Err(err) => Err(err.into()),
            Ok(_) => {
                tx.commit()?;
                Ok(id)
            }
        }
    }

    /// One attempt's facts, if the run is known.
    pub fn attempt_of(&self, run_id: &str) -> Result<Option<AttemptRow>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, state, attempt, previous_attempt, cause, paused_because, \
                 pinned, requested_model, effective_model FROM run WHERE id = ?1",
                [run_id],
                |row| {
                    Ok(AttemptRow {
                        id: row.get(0)?,
                        state: row.get(1)?,
                        attempt: row.get(2)?,
                        previous_attempt: row.get(3)?,
                        cause: row.get(4)?,
                        paused_because: row.get(5)?,
                        pinned: row.get(6)?,
                        requested_model: row.get(7)?,
                        effective_model: row.get(8)?,
                    })
                },
            )
            .optional()?)
    }
}

/// The transition for an ask of the step: a `start`, or a `retry` of the
/// step's last attempt on the card.
fn asked(
    tx: &Transaction,
    card: &str,
    step: &str,
    carry: &AttemptCarry,
) -> Result<Transition, StoreError> {
    let last = tx
        .query_row(
            "SELECT id, state, attempt, pinned, requested_model FROM run \
             WHERE card_id = ?1 AND step_id = ?2 \
             ORDER BY started_at DESC, rowid DESC LIMIT 1",
            [card, step],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    Ok(match last {
        // No attempt yet, or the last one closed the cycle — `ok`, or walked
        // away from as `cancelled`: asking again is a first attempt under
        // what the caller chose.
        None => start(card, step, carry),
        Some((_, state, _, _, _)) if state == "ok" || state == "cancelled" => {
            start(card, step, carry)
        }
        // Paused, failed or lost: the same step trying again, carrying what
        // the last attempt carried. (`running` lands here too, and never
        // reaches the insert — the card's one-run index refuses it first.)
        Some((id, _, attempt, pinned, requested_model)) => Transition {
            card: card.to_owned(),
            step: step.to_owned(),
            cause: "retry",
            attempt: attempt.unwrap_or(1) + 1,
            previous: Some(id),
            pinned,
            requested_model,
        },
    })
}

/// The transition for a switch away from a pause: the attempt after the
/// paused one, under what the caller carries.
fn switched(
    tx: &Transaction,
    from: &str,
    how: SwitchKind,
    carry: &AttemptCarry,
) -> Result<Transition, StoreError> {
    // The run being switched away from must be there and must be paused: the
    // pause is what offers the switch, and a run that is not there cannot be
    // switched away from either.
    let Some((card, step, attempt)) = tx
        .query_row(
            "SELECT card_id, step_id, attempt FROM run WHERE id = ?1 AND state = 'paused'",
            [from],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()?
    else {
        return Err(StoreError::NotPaused);
    };
    Ok(Transition {
        card,
        step,
        cause: match how {
            SwitchKind::Click => "click",
            SwitchKind::Automatic => "automatic",
        },
        attempt: attempt.unwrap_or(1) + 1,
        previous: Some(from.to_owned()),
        pinned: carry.pinned.clone(),
        requested_model: carry.requested_model.clone(),
    })
}

/// A first attempt: number 1, continuing nothing.
fn start(card: &str, step: &str, carry: &AttemptCarry) -> Transition {
    Transition {
        card: card.to_owned(),
        step: step.to_owned(),
        cause: "start",
        attempt: 1,
        previous: None,
        pinned: carry.pinned.clone(),
        requested_model: carry.requested_model.clone(),
    }
}

#[cfg(test)]
#[path = "run_attempts_tests.rs"]
mod tests;
