//! One error shape for the whole contract.
//!
//! `code` is enumerated and stable — the UI branches on it. `message` is a
//! sentence a person can read and may change freely. The screen shows
//! `message` verbatim, so write it for whoever reads it.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthenticated,
    /// Used when a path falls outside the registered project root. Distinct
    /// from a generic error on purpose: the daemon runs terminals and writes
    /// files, so reaching it is reaching the machine.
    Forbidden,
    NotFound,
    Conflict,
    RateLimited,
    Invalid,
    Unsupported,
    Busy,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RpcError {
    pub code: ErrorCode,
    pub message: String,
    pub retry_after_ms: Option<u32>,
    pub details: Option<String>,
}

impl RpcError {
    /// An `Internal` is a bug by definition, so it is kept for the error
    /// report here, where every one of them is made, with the place that made
    /// it: the response itself cannot be reached on its way to the window.
    #[track_caller]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let error = Self {
            code,
            message: message.into(),
            retry_after_ms: None,
            details: None,
        };
        if code == ErrorCode::Internal {
            let at = std::panic::Location::caller().to_string();
            devpit_core::reports::record(devpit_core::reports::Report {
                kind: "internal",
                location: Some(&at),
                message: &error.message,
                stack: None,
            });
        }
        error
    }

    #[track_caller]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

/// Store errors cross into the contract without leaking SQLite detail to the
/// screen: "database is locked" helps nobody.
impl From<devpit_core::StoreError> for RpcError {
    #[track_caller]
    fn from(err: devpit_core::StoreError) -> Self {
        match err {
            // Busy, not Conflict: a conflict is the board's "move it anyway?",
            // and no answer to it lets a second run into the same checkout.
            devpit_core::StoreError::AlreadyRunning => Self::new(
                ErrorCode::Busy,
                devpit_core::i18n::t("error.alreadyRunning", &[]),
            ),
            // `Internal` goes to the report, which is never translated: the
            // `StoreError` text is English by construction.
            err => Self::internal(err.to_string()),
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_serialize_to_stable_strings() {
        // The UI compares against these. Changing them breaks the screen
        // silently, so they are pinned by a test.
        assert_eq!(
            serde_json::to_string(&ErrorCode::NotFound).expect("serialize"),
            "\"not_found\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::RateLimited).expect("serialize"),
            "\"rate_limited\""
        );
    }

    #[test]
    fn the_envelope_carries_four_camel_case_fields() {
        let json = serde_json::to_value(RpcError::forbidden("outside the project root"))
            .expect("serialize");
        for field in ["code", "message", "retryAfterMs", "details"] {
            assert!(json.get(field).is_some(), "missing {field}");
        }
    }
}
