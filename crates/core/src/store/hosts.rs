//! Host profiles: the machines a project's terminals can run on, over SSH.
//!
//! A table, unlike the agent profiles' one preference: a project links to a
//! host by its id, and the link must go when the host does.

use crate::store::{Store, StoreError};

/// One host profile, as stored. Paths only: no key is ever kept here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRow {
    pub id: String,
    pub name: String,
    /// An ssh config alias or a host name: what `ssh` is given.
    pub target: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// The identity file's path on this machine, never its contents.
    pub identity_file: Option<String>,
    pub work_root: String,
    /// `dev`, `staging` or `production`.
    pub environment: String,
}

const COLUMNS: &str = "id, name, target, user, port, identity_file, work_root, environment";

fn row(found: &rusqlite::Row<'_>) -> rusqlite::Result<HostRow> {
    Ok(HostRow {
        id: found.get(0)?,
        name: found.get(1)?,
        target: found.get(2)?,
        user: found.get(3)?,
        port: found.get(4)?,
        identity_file: found.get(5)?,
        work_root: found.get(6)?,
        environment: found.get(7)?,
    })
}

impl Store {
    /// Every host profile, by name.
    pub fn hosts(&self) -> Result<Vec<HostRow>, StoreError> {
        let mut query = self
            .conn
            .prepare(&format!("SELECT {COLUMNS} FROM host ORDER BY name"))?;
        let rows = query.query_map([], row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Adds the profile, or replaces the one with its id.
    pub fn save_host(&self, host: &HostRow) -> Result<(), StoreError> {
        let at = super::board::now();
        self.conn.execute(
            &format!(
                "INSERT INTO host ({COLUMNS}, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9) \
                 ON CONFLICT(id) DO UPDATE SET name = ?2, target = ?3, user = ?4, port = ?5, \
                 identity_file = ?6, work_root = ?7, environment = ?8, updated_at = ?9"
            ),
            rusqlite::params![
                host.id,
                host.name,
                host.target,
                host.user,
                host.port,
                host.identity_file,
                host.work_root,
                host.environment,
                at,
            ],
        )?;
        Ok(())
    }

    pub fn remove_host(&self, id: &str) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM host WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;
