//! Schema migrations, applied once each.
//!
//! Idempotency matters: the app opens the store on every start, and a
//! migration that reruns against real data loses data. The applied version is
//! read from the database itself (`user_version`), not tracked beside it.

use std::path::Path;

use rusqlite::Connection;

use crate::store::StoreError;

#[path = "migrations_list.rs"]
mod list;
use list::{BEFORE, MIGRATIONS};

pub(super) struct Migration {
    pub(super) version: i64,
    pub(super) sql: &'static str,
}

/// Work a migration needs done in Rust before its SQL, given the workspace
/// root. Inside the same transaction, so its failure leaves the version as it was.
pub(super) type Before = fn(&Connection, &Path) -> Result<(), StoreError>;

fn apply(conn: &Connection, root: &Path, migration: &Migration) -> Result<(), StoreError> {
    for (_, before) in BEFORE.iter().filter(|(at, _)| *at == migration.version) {
        before(conn, root)?;
    }
    conn.execute_batch(&format!(
        "{} PRAGMA user_version = {};",
        migration.sql, migration.version
    ))?;
    Ok(())
}

/// The newest schema this build knows.
///
/// Read from the list rather than written down beside it: a constant that has
/// to be edited whenever a migration is added is a constant that will one day
/// disagree with the list it describes.
///
/// `run` asks it to skip the write lock when there is nothing to apply.
pub fn latest() -> i64 {
    MIGRATIONS.iter().map(|one| one.version).max().unwrap_or(0)
}

/// Applies whatever has not been applied yet. Called on every start.
///
/// One transaction around the whole chain, opened `IMMEDIATE`, with the
/// applied version read **inside** it.
///
/// Both halves of that matter, and for the same reason: more than one
/// connection can open this file at the same moment. Startup alone does it —
/// the shell probe reads the store on its own thread while the main thread is
/// still setting up — and two copies of the app do it trivially. Reading the
/// version first and writing after meant both connections saw the same old
/// number and both applied the same migration; the second one arrives at an
/// `ALTER TABLE` for a column that now exists, and the app fails to start.
///
/// `IMMEDIATE` takes the write lock at `BEGIN` rather than at the first write,
/// so the second connection waits here instead of racing, and then reads the
/// version the first one just committed. `busy_timeout` is what makes that a
/// wait rather than a refusal.
///
/// All-or-nothing rather than one transaction each, which is stronger than
/// what was here before and keeps the reason it was written: a partial failure
/// must not leave half the tables standing with the version already bumped.
pub fn run(conn: &Connection, root: &Path) -> Result<(), StoreError> {
    // Every command opens a store, so the common case must not take the write
    // lock: with many terminals reporting at once it queued behind their writes
    // and came back "database is locked". A version only ever goes up.
    if up_to_date(conn)? {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let applied = match conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0)) {
        Ok(applied) => applied,
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err.into());
        }
    };

    for migration in MIGRATIONS.iter().filter(|m| m.version > applied) {
        if let Err(err) = apply(conn, root, migration) {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err);
        }
    }

    conn.execute_batch("COMMIT")?;
    Ok(())
}

/// Whether this file already holds every migration, read without a lock.
fn up_to_date(conn: &Connection) -> Result<bool, StoreError> {
    let applied: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(applied >= latest())
}

/// Applies only up to `version`, to build a database of an older shape.
///
/// Only the test asks: it is how "the migration is additive" is proved against
/// rows rather than against an empty file.
#[cfg(test)]
pub fn run_up_to(conn: &Connection, version: i64) -> Result<(), StoreError> {
    for migration in MIGRATIONS.iter().filter(|m| m.version <= version) {
        conn.execute_batch(&format!(
            "BEGIN; {} PRAGMA user_version = {}; COMMIT;",
            migration.sql, migration.version
        ))?;
    }
    Ok(())
}

/// One migration's SQL, for a test that runs it statement by statement.
#[cfg(test)]
pub fn sql_of(version: i64) -> &'static str {
    MIGRATIONS
        .iter()
        .find(|m| m.version == version)
        .map_or("", |m| m.sql)
}

/// Only the test asks. Same rule as the RPC contract: no caller, no code.
#[cfg(test)]
pub fn latest_version() -> i64 {
    MIGRATIONS.last().map_or(0, |m| m.version)
}
