//! Reading a headless turn's stream, line by line.
//!
//! Apart from `headless.rs` because that file starts the process and this one
//! only reads what it wrote, which is testable against a recorded stream.

use serde::Deserialize;

use crate::Outcome;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Line {
    #[serde(rename = "result")]
    Result {
        #[serde(default)]
        result: String,
        #[serde(default)]
        is_error: bool,
        #[serde(default)]
        total_cost_usd: f64,
        #[serde(default)]
        duration_ms: i64,
        #[serde(default)]
        num_turns: i64,
        #[serde(default)]
        stop_reason: Option<String>,
    },
    #[serde(rename = "system")]
    System {
        #[serde(default)]
        subtype: String,
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        model: Option<String>,
    },
    #[serde(other)]
    Other,
}

/// A turn's stream: each fragment to `on_partial`, the outcome from its
/// `result` line, and the session and model from the `init` line before it.
pub(crate) fn read_stream(
    lines: impl Iterator<Item = String>,
    mut on_partial: impl FnMut(&str),
) -> Option<Outcome> {
    let mut session_id = None;
    let mut model = None;
    let mut outcome = None;
    for line in lines {
        match serde_json::from_str::<Line>(&line) {
            Ok(Line::Result {
                result,
                is_error,
                total_cost_usd,
                duration_ms,
                num_turns,
                stop_reason,
            }) => {
                outcome = Some(Outcome {
                    result,
                    cost_usd: total_cost_usd,
                    duration_ms,
                    turns: num_turns,
                    is_error,
                    stop_reason,
                    session_id: session_id.clone(),
                    model: model.clone(),
                });
            }
            Ok(Line::System {
                subtype,
                session_id: said,
                model: used,
            }) => {
                if subtype == "init" {
                    session_id = said;
                    model = used;
                }
                on_partial(&line);
            }
            Ok(Line::Other) => on_partial(&line),
            // A line we cannot read is not a reason to abandon the run: the
            // one that matters is `result`, and it comes last.
            Err(_) => {}
        }
    }
    outcome
}

#[cfg(test)]
#[path = "headless_stream_tests.rs"]
mod tests;
