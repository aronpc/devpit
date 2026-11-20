//! Every run in a project, a page at a time.
//!
//! Apart from `runs.rs`, which answers for one card: this one crosses cards,
//! filters, and pages by position rather than offset, so a run starting while
//! somebody reads page two does not shift page three.

use crate::store::{Store, StoreError};

/// The most one page may hold, whatever was asked for.
pub const MOST: u32 = 200;

/// What the runs view asks for. `before` is the last row of the page before.
#[derive(Debug, Default)]
pub struct RunQuery<'a> {
    pub project_id: &'a str,
    pub step_id: Option<&'a str>,
    pub state: Option<&'a str>,
    /// Unix seconds, inclusive.
    pub since: Option<i64>,
    /// Unix seconds, exclusive.
    pub until: Option<i64>,
    pub before: Option<(i64, &'a str)>,
    pub limit: u32,
}

/// One run, with the card it ran on.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedRun {
    pub id: String,
    pub card_id: String,
    pub card_title: String,
    pub step_id: String,
    pub state: String,
    /// The first 400 characters: a list is not where a log is read.
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

impl Store {
    /// Runs newest first, `limit` of them at most (itself at most [`MOST`]).
    ///
    /// The tie goes to `id` here, and it has to: the cursor above compares
    /// `r.id < ?7`, so the order and the cursor must be the same thing or a
    /// page boundary lands between rows that were never adjacent — which is
    /// how this was briefly broken by a fix meant for `card_links`.
    ///
    /// A ULID is not a clock (see `runs::now`), so within one second this
    /// order is arbitrary. That is fine for a page and not fine for "the
    /// latest run": a list needs an order that is total and stable, and this
    /// one is both.
    pub fn project_runs(&self, query: &RunQuery<'_>) -> Result<Vec<ListedRun>, StoreError> {
        let (before_at, before_id) = query.before.unzip();
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.card_id, c.title, r.step_id, r.state, substr(r.output, 1, 400), \
             r.exit_code, r.cost_usd, r.duration_ms, r.pinned, r.requested_model, \
             r.effective_model, r.attempt, r.started_at \
             FROM run r JOIN card c ON c.id = r.card_id \
             WHERE c.project_id = ?1 \
             AND (?2 IS NULL OR r.step_id = ?2) \
             AND (?3 IS NULL OR r.state = ?3) \
             AND (?4 IS NULL OR r.started_at >= ?4) \
             AND (?5 IS NULL OR r.started_at < ?5) \
             AND (?6 IS NULL OR r.started_at < ?6 OR (r.started_at = ?6 AND r.id < ?7)) \
             ORDER BY r.started_at DESC, r.id DESC LIMIT ?8",
        )?;
        let rows = stmt
            .query_map(
                rusqlite::params![
                    query.project_id,
                    query.step_id,
                    query.state,
                    query.since,
                    query.until,
                    before_at,
                    before_id,
                    query.limit.clamp(1, MOST),
                ],
                |row| {
                    Ok(ListedRun {
                        id: row.get(0)?,
                        card_id: row.get(1)?,
                        card_title: row.get(2)?,
                        step_id: row.get(3)?,
                        state: row.get(4)?,
                        output: row.get(5)?,
                        exit_code: row.get(6)?,
                        cost_usd: row.get(7)?,
                        duration_ms: row.get(8)?,
                        pinned: row.get(9)?,
                        requested_model: row.get(10)?,
                        effective_model: row.get(11)?,
                        attempt: row.get(12)?,
                        started_at: row.get(13)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "project_runs_tests.rs"]
mod tests;
