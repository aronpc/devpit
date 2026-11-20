//! Runs: a step executing on a card, and what it left behind.
//!
//! Apart from `board.rs` because that file is the *shape* of the board —
//! lanes, cards, where a card sits — and a run is an event that happened to
//! one. The two also fail differently: a column that cannot be read is a
//! board that cannot be drawn, and a run that cannot be read is one row of
//! history missing.

use crate::store::{Store, StoreError};

pub struct RunRow {
    pub id: String,
    pub card_id: String,
    pub step_id: String,
    pub state: String,
    pub output: Option<String>,
    pub exit_code: Option<i64>,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<i64>,
    pub pinned: Option<String>,
    pub requested_model: Option<String>,
    pub effective_model: Option<String>,
    pub attempt: Option<i64>,
    pub started_at: i64,
}

/// Seconds, which is why every query that orders by it breaks the tie on
/// `rowid`: two runs of the same card in the same second are an ordinary
/// thing — a step replayed, a lane that advanced — and SQLite is free to
/// return tied rows in any order it likes. It returned a different one on a
/// Mac than on Linux, and "the latest run" came back stale.
///
/// **Not on `id`.** A run's id is a ULID from `Ulid::generate`, which is a
/// millisecond and eighty random bits — no monotonic generator between calls.
/// Two ids made in the same millisecond order by their random half, so
/// `ORDER BY started_at DESC, id DESC` is a coin toss wearing the clothes of
/// a tiebreak. It was in three queries and passed on Linux for a year.
/// `rowid` is insertion order and cannot tie.
pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

impl Store {
    /// Opens a run, recording where the card stood when it started.
    ///
    /// `from_column` is on the row rather than only in the thread's memory: a
    /// verdict that sends the card back needs somewhere to send it, and a run
    /// in flight when the app quits used to take that answer with it.
    pub fn start_run(
        &self,
        card_id: &str,
        step_id: &str,
        from_column: Option<&str>,
    ) -> Result<String, StoreError> {
        let id = format!("run_{}", ulid::Ulid::generate());
        let inserted = self.conn.execute(
            "INSERT INTO run (id, card_id, step_id, state, started_at, from_column) \
             VALUES (?1, ?2, ?3, 'running', ?4, ?5)",
            rusqlite::params![id, card_id, step_id, now(), from_column],
        );
        match inserted {
            Ok(_) => Ok(id),
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                Err(StoreError::AlreadyRunning)
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Where the card stood when this run started, if it was recorded.
    pub fn run_came_from(&self, run_id: &str) -> Result<Option<String>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row(
                "SELECT from_column FROM run WHERE id = ?1",
                [run_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// How a run ended, if the run is known. `running` until it does.
    pub fn run_state(&self, run_id: &str) -> Result<Option<String>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row("SELECT state FROM run WHERE id = ?1", [run_id], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// The card a run belongs to, if the run is known.
    pub fn run_card(&self, run_id: &str) -> Result<Option<String>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row("SELECT card_id FROM run WHERE id = ?1", [run_id], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// The card, step and origin column a run was opened for.
    pub fn run_context(
        &self,
        run_id: &str,
    ) -> Result<Option<(String, String, Option<String>)>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row(
                "SELECT card_id, step_id, from_column FROM run WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?)
    }

    /// Closes every run left open by a process that is gone.
    ///
    /// The runs going right now, by what a person would call them.
    ///
    /// Read-only, unlike `close_abandoned_runs`: this one is asked while the
    /// app is up, by anything that has to know whether interrupting would cost
    /// somebody their work.
    pub fn running_runs(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, c.title FROM run r JOIN card c ON c.id = r.card_id \
             WHERE r.state = 'running' AND r.ended_at IS NULL \
             ORDER BY r.started_at, r.rowid",
        )?;
        let found = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<(String, String)>, _>>()?;
        Ok(found)
    }

    /// Called once at launch. A `running` row with no `ended_at` after a
    /// restart is not a run still going — nothing survives the process that
    /// spawned its thread — so leaving it says the card is working when it is
    /// not, forever. Answers with what it closed, so the launch can say so.
    pub fn close_abandoned_runs(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, card_id FROM run WHERE state = 'running' AND ended_at IS NULL")?;
        let stranded = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<(String, String)>, _>>()?;
        drop(stmt);

        if stranded.is_empty() {
            return Ok(stranded);
        }
        self.conn.execute(
            "UPDATE run SET state = 'lost', ended_at = ?1, \
             output = COALESCE(output, 'the app closed while this was running') \
             WHERE state = 'running' AND ended_at IS NULL",
            [now()],
        )?;
        Ok(stranded)
    }

    /// Closes a run that is still open, answering whether it did.
    ///
    /// Once only: a run a person stopped says `cancelled`, and the thread whose
    /// process that stop killed finds it closed instead of writing `failed`
    /// over it.
    pub fn finish_run(
        &self,
        run_id: &str,
        state: &str,
        output: Option<&str>,
        cost_usd: Option<f64>,
        duration_ms: Option<i64>,
        exit_code: Option<i64>,
    ) -> Result<bool, StoreError> {
        let closed = self.conn.execute(
            "UPDATE run SET state = ?2, output = ?3, cost_usd = ?4, duration_ms = ?5, \
             exit_code = ?6, ended_at = ?7 WHERE id = ?1 AND ended_at IS NULL",
            rusqlite::params![
                run_id,
                state,
                output,
                cost_usd,
                duration_ms,
                exit_code,
                now()
            ],
        )?;
        Ok(closed == 1)
    }

    /// A card's runs, most recent first.
    pub fn runs(&self, card_id: &str) -> Result<Vec<RunRow>, StoreError> {
        self.runs_reading(card_id, true)
    }

    /// [`Store::runs`] without their output, for a board that lists every
    /// card's runs and shows none of it.
    pub fn runs_brief(&self, card_id: &str) -> Result<Vec<RunRow>, StoreError> {
        self.runs_reading(card_id, false)
    }

    fn runs_reading(&self, card_id: &str, with_output: bool) -> Result<Vec<RunRow>, StoreError> {
        let output = if with_output { "output" } else { "NULL" };
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id, card_id, step_id, state, {output}, exit_code, cost_usd, duration_ms, \
             pinned, requested_model, effective_model, attempt, started_at FROM run WHERE card_id = ?1 \
             ORDER BY started_at DESC, rowid DESC"
        ))?;
        let rows = stmt
            .query_map([card_id], |row| {
                Ok(RunRow {
                    id: row.get(0)?,
                    card_id: row.get(1)?,
                    step_id: row.get(2)?,
                    state: row.get(3)?,
                    output: row.get(4)?,
                    exit_code: row.get(5)?,
                    cost_usd: row.get(6)?,
                    duration_ms: row.get(7)?,
                    pinned: row.get(8)?,
                    requested_model: row.get(9)?,
                    effective_model: row.get(10)?,
                    attempt: row.get(11)?,
                    started_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "runs_tests.rs"]
mod tests;
