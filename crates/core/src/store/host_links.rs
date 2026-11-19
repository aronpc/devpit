//! A project's links to hosts: where the project is on each one.
//!
//! The path is the person's: devpit never creates or moves a project on a
//! host. A link goes with its host or its project.

use crate::store::{Store, StoreError};

/// One link, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostLinkRow {
    pub project_id: String,
    pub host_id: String,
    /// The project's folder on the host, from `/` or `~`.
    pub path: String,
}

impl Store {
    /// A project's links, by host name.
    pub fn host_links(&self, project_id: &str) -> Result<Vec<HostLinkRow>, StoreError> {
        let mut query = self.conn.prepare(
            "SELECT l.project_id, l.host_id, l.path FROM project_host l \
             JOIN host h ON h.id = l.host_id WHERE l.project_id = ?1 ORDER BY h.name",
        )?;
        let rows = query.query_map([project_id], |found| {
            Ok(HostLinkRow {
                project_id: found.get(0)?,
                host_id: found.get(1)?,
                path: found.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Links the project to a host at `path`, or moves its link there.
    pub fn link_host(&self, project_id: &str, host_id: &str, path: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO project_host (project_id, host_id, path, created_at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(project_id, host_id) DO UPDATE SET path = ?3",
            rusqlite::params![project_id, host_id, path, super::board::now()],
        )?;
        Ok(())
    }

    pub fn unlink_host(&self, project_id: &str, host_id: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "DELETE FROM project_host WHERE project_id = ?1 AND host_id = ?2",
            [project_id, host_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "host_links_tests.rs"]
mod tests;
