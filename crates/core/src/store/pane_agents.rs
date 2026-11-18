//! The agent devpit started in a pane, and the session it is in.
//!
//! Written when the agent is launched and updated from its own hooks: where
//! a session runs, and the one pane that holds each conversation.

use rusqlite::OptionalExtension;

use crate::store::{Store, StoreError};

#[derive(Debug, Clone, PartialEq)]
pub struct PaneAgent {
    /// The profile or agent id it was started with.
    pub launch: String,
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

impl Store {
    /// A new launch replaces whatever the pane ran before, session and all.
    pub fn remember_pane_launch(
        &self,
        project_id: &str,
        leaf_id: &str,
        launch: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO pane_agent (leaf_id, project_id, launch, updated_at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(leaf_id) DO UPDATE SET project_id = ?2, launch = ?3, \
             session_id = NULL, transcript_path = NULL, updated_at = ?4",
            rusqlite::params![leaf_id, project_id, launch, now()],
        )?;
        Ok(())
    }

    /// Only for a pane devpit launched an agent in: a hook from an agent
    /// somebody typed by hand creates nothing, since there is no launch to repeat.
    ///
    /// A conversation runs in one pane: a pane devpit launched that takes it
    /// over makes any other forget it, or coming back that one would start
    /// the same conversation a second time.
    pub fn remember_pane_session(
        &self,
        leaf_id: &str,
        session_id: &str,
        transcript_path: Option<&str>,
    ) -> Result<(), StoreError> {
        let tx = self.writing()?;
        let taken = tx.execute(
            "UPDATE pane_agent SET session_id = ?2, transcript_path = ?3, updated_at = ?4 \
             WHERE leaf_id = ?1",
            rusqlite::params![leaf_id, session_id, transcript_path, now()],
        )?;
        if taken > 0 {
            tx.execute(
                "DELETE FROM pane_agent WHERE session_id = ?1 AND leaf_id != ?2",
                [session_id, leaf_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn forget_pane_agent(&self, leaf_id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM pane_agent WHERE leaf_id = ?1", [leaf_id])?;
        Ok(())
    }

    /// The pane a session was last seen in, if one was.
    pub fn pane_of_session(&self, session_id: &str) -> Result<Option<String>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row(
                "SELECT leaf_id FROM pane_agent WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The project and pane a session was last seen in, if one was: how a
    /// session in a pane with no tmux to say so is found.
    pub fn place_of_session(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, String)>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT project_id, leaf_id FROM pane_agent WHERE session_id = ?1",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    pub fn pane_agent(&self, leaf_id: &str) -> Result<Option<PaneAgent>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT launch, session_id, transcript_path FROM pane_agent WHERE leaf_id = ?1",
                [leaf_id],
                |row| {
                    Ok(PaneAgent {
                        launch: row.get(0)?,
                        session_id: row.get(1)?,
                        transcript_path: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }
}

#[cfg(test)]
#[path = "pane_agents_tests.rs"]
mod tests;
