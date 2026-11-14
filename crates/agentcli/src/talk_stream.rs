//! Reading a chat turn's stream, line by line.
//!
//! Apart from `talk.rs` because that file starts the process and owns its
//! stdin; this one only reads what the CLI wrote, and says what the stream
//! said about itself.

use devpit_rpc::{Part, SessionInit};

use crate::control::{turn_over, Control, Running};
use crate::driver::{Driver, Read};

/// What a turn's stream said, apart from its parts.
#[derive(Default)]
pub(crate) struct Heard {
    /// The last id seen, not the first: `/clear` starts a new session inside
    /// the turn, and resuming the one it left would undo it.
    pub session_id: Option<String>,
    pub init: Option<SessionInit>,
    pub anchor: Option<String>,
    /// The CLI's own reason, cost and error flag. `None` when the stream ended
    /// without an end frame, which means the turn was stopped.
    pub ended: Option<Ending>,
    /// The provider refused the turn: a rate limit or an outage.
    pub limit: Option<devpit_rpc::Limit>,
}

/// The CLI's own end frame, as it said it.
pub(crate) struct Ending {
    pub stop_reason: Option<String>,
    pub cost_usd: Option<f64>,
    pub is_error: bool,
    pub context: Option<devpit_rpc::Context>,
}

/// Follows the stream, handing each part to `on_part` as it arrives, and
/// closing the turn's stdin as soon as the CLI can no longer be asked for
/// anything.
pub(crate) fn follow(
    driver: &dyn Driver,
    lines: impl Iterator<Item = String>,
    control: &Control,
    on_session: Option<&(dyn Fn(&str) + Sync)>,
    on_part: impl FnMut(Part),
) -> Heard {
    let (heard, _) = one_turn(driver, lines, control, on_session, on_part, false);
    heard
}

/// One turn of a process that stays: stops reading once the turn is over —
/// its result in and nothing of it still running — and leaves stdin open for
/// the next. `false` beside it when the stream ended first.
pub(crate) fn next_turn(
    driver: &dyn Driver,
    lines: impl Iterator<Item = String>,
    control: &Control,
    on_session: Option<&(dyn Fn(&str) + Sync)>,
    on_part: impl FnMut(Part),
) -> (Heard, bool) {
    one_turn(driver, lines, control, on_session, on_part, true)
}

fn one_turn(
    driver: &dyn Driver,
    lines: impl Iterator<Item = String>,
    control: &Control,
    on_session: Option<&(dyn Fn(&str) + Sync)>,
    mut on_part: impl FnMut(Part),
    stays: bool,
) -> (Heard, bool) {
    let mut heard = Heard::default();
    let mut running = Running::default();
    let mut result_seen = false;

    for line in lines {
        control.reply(&line);
        if control.prompted(&line, heard.session_id.as_deref()) {
            continue;
        }
        if let Some(seen) = driver.session(&line) {
            if heard.session_id.as_deref() != Some(seen.as_str()) {
                on_session.inspect(|tell| tell(&seen));
            }
            heard.session_id = Some(seen);
        }
        if let Some(uuid) = driver.echoed(&line) {
            control.heard(&uuid);
        }
        heard.anchor = driver.anchor(&line).or(heard.anchor);
        heard.limit = crate::claude_lines::limit(&line, heard.limit);
        match driver.read(&line) {
            Read::Parts(parts) => parts.into_iter().for_each(|part| {
                running.saw(&part);
                on_part(part);
            }),
            Read::Ended {
                stop_reason,
                cost_usd,
                is_error,
                context,
            } => {
                heard.ended = Some(Ending {
                    stop_reason,
                    cost_usd,
                    is_error,
                    context,
                });
                result_seen = true;
            }
            Read::Init(said) => heard.init = Some(said),
            Read::Nothing => {}
        }
        if turn_over(stays, result_seen, &running) {
            if stays {
                return (heard, true);
            }
            control.close();
        }
    }

    (heard, false)
}

#[cfg(test)]
#[path = "talk_stream_tests.rs"]
mod tests;
