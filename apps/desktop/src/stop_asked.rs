//! A session stopped at the orchestrator's call: on the person's own word,
//! read in the orchestrator's log as `draft_sending` reads it, when another
//! session already works the same card and relieves it, or on the person's
//! click. A stop their words did not cover waits above the orchestrator's
//! composer and on devpit's Remote with every other one it asked for, so one
//! click stops them all, on what each loses at that click.
//!
//! Held in memory, per orchestrator account: a stop asked is for now.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use devpit_agentcli::person::{From, Said};
use devpit_rpc::{
    Guardrail, LiveSession, RemoteStop, RpcError, StopAsked, StopsAsked, StopsConfirmed,
};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter as _};

use crate::stopping::Clicked;

/// Said to the window, and to the Remote's viewers, when the stops waiting
/// for a click change.
pub(crate) const STOP_ASKED: &str = "orchestrator:stop-asked";

/// An orchestrator's stop of `name` (only `pid`), as devpit found things
/// when it was asked.
pub(crate) struct Request<'a> {
    /// The person's last message to the orchestrator.
    pub said: Option<&'a Said>,
    pub name: &'a str,
    pub pid: Option<i32>,
    /// The sessions running, whose full names a message may list.
    pub running: &'a [&'a str],
    /// Another session runs on its card and relieves it.
    pub swapped: bool,
    /// What stopping it loses now, if anything.
    pub losing: Option<String>,
    pub force: bool,
    /// A draft waits for it, which a ✅ beside its name answers.
    pub drafted: bool,
}

/// Whether the stop goes ahead now: on the person's word — the whole
/// message the request, beside other names of `running` to stop several — or
/// a swap, and what it would lose only with `force`. Otherwise why: it waits
/// for the person's click.
pub(crate) fn allowed(asked: &Request<'_>) -> Result<(), String> {
    let name = asked.name;
    let refused = match asked.said {
        _ if asked.swapped => None,
        Some(said) if matches!(said.from, From::Channel(_)) => Some(format!(
            "the person last wrote from {}, where a stop goes only by its button",
            said.from.label()
        )),
        Some(said) if asked.drafted && devpit_agentcli::person::by_mark(&said.text) => Some(
            format!("the person's ✅ beside {name} answers the draft waiting for it, not a stop"),
        ),
        Some(said) if devpit_agentcli::person::asks_to_stop(&said.text, name, asked.running) => {
            None
        }
        Some(said) => Some(format!(
            "the person's last message («{}») is not only a request to stop {name}{}",
            crate::draft_sending::short(&said.text),
            if said.text.to_lowercase().contains(&name.to_lowercase()) {
                ""
            } else {
                ": it does not name it"
            }
        )),
        None => Some(format!(
            "devpit found nothing the person said asking for {name} to stop"
        )),
    };
    if let Some(why) = refused {
        return Err(why);
    }
    match &asked.losing {
        Some(losing) if !asked.force => Err(format!("{name}: {losing}")),
        _ => Ok(()),
    }
}

/// The guardrail a stop answers to: a forced one, or one that loses work,
/// has its own.
pub(crate) fn guardrail(force: bool, losing: Option<&str>) -> Guardrail {
    if force || losing.is_some() {
        Guardrail::ForceStop
    } else {
        Guardrail::Stop
    }
}

/// The orchestrator's stop: `Ok` when it goes ahead now, else held for the
/// click of `profile`'s person, with what the orchestrator is answered.
pub(crate) fn requested(profile: &str, asked: &Request<'_>) -> Result<(), Value> {
    let why = match allowed(asked) {
        Ok(()) => return Ok(()),
        Err(why) => why,
    };
    let held = StopAsked {
        name: asked.name.to_owned(),
        pid: asked.pid,
        losing: asked.losing.clone(),
    };
    Err(ask(profile, held, &why, asked.said.map(|said| said.from)))
}

/// Names stopped on the person's word, by orchestrator account and the id of
/// the message: one message stops several sessions through one call each, and
/// the ones stopped before are no longer running to be read in it.
type Stopped = HashMap<(String, String), Vec<String>>;

fn stopped() -> &'static Mutex<Stopped> {
    static STOPPED: OnceLock<Mutex<Stopped>> = OnceLock::new();
    STOPPED.get_or_init(Mutex::default)
}

/// Remembers `name` as stopped on the message `said`. Only the last message
/// is kept per account.
pub(crate) fn stopped_now(profile: &str, said: &str, name: &str) {
    if let Ok(mut held) = stopped().lock() {
        held.retain(|(one, id), _| one != profile || id == said);
        held.entry((profile.to_owned(), said.to_owned()))
            .or_default()
            .push(name.to_owned());
    }
}

/// The names already stopped on the message `said`, to read it with.
pub(crate) fn stopped_on(profile: &str, said: &str) -> Vec<String> {
    stopped()
        .lock()
        .ok()
        .and_then(|held| held.get(&(profile.to_owned(), said.to_owned())).cloned())
        .unwrap_or_default()
}

type Asked = HashMap<String, Vec<StopAsked>>;

fn asked() -> &'static Mutex<Asked> {
    static ASKED: OnceLock<Mutex<Asked>> = OnceLock::new();
    ASKED.get_or_init(Mutex::default)
}

/// Keeps `stop` waiting for the click of `profile`'s person, in place of one
/// asked for the same name before.
fn hold(profile: &str, stop: StopAsked) {
    if let Ok(mut held) = asked().lock() {
        let list = held.entry(profile.to_owned()).or_default();
        list.retain(|one| one.name != stop.name);
        list.push(stop);
    }
}

/// Holds `stop` and answers the orchestrator `why` nothing stopped yet. The
/// Claude app shows no devpit button, so a person last heard `from` there is
/// told the one message that stops every stop waiting.
pub(crate) fn ask(profile: &str, stop: StopAsked, why: &str, from: Option<From>) -> Value {
    let name = stop.name.clone();
    hold(profile, stop);
    let waits = "It waits for the person's click above this chat's composer and on devpit's Remote, listed with every other stop you asked for and what each would lose; one click stops them all.";
    let tell = if from == Some(From::RemoteControl) {
        let names: Vec<String> = waiting(profile).into_iter().map(|one| one.name).collect();
        format!("The person wrote from the Claude app, where that button does not show: tell them they can send `stop {}` alone instead, then call this again for each — with force for one that would lose work, once they have heard what.", names.join(" "))
    } else {
        "Tell them so; do not ask them to type it again.".to_owned()
    };
    json!({
        "done": false,
        "asked": format!("stop {name}"),
        "note": format!("Not stopped yet: {why}. {waits} {tell}"),
    })
}

/// The stops waiting for `profile`'s person, oldest first.
pub(crate) fn waiting(profile: &str) -> Vec<StopAsked> {
    asked()
        .lock()
        .ok()
        .and_then(|held| held.get(profile).cloned())
        .unwrap_or_default()
}

/// Every stop waiting, by the orchestrator account it waits for: what the
/// Remote shows.
pub(crate) fn all() -> Vec<RemoteStop> {
    let Ok(held) = asked().lock() else {
        return Vec::new();
    };
    let mut every: Vec<RemoteStop> = held
        .iter()
        .flat_map(|(profile, stops)| {
            stops.iter().map(|stop| RemoteStop {
                profile: profile.clone(),
                stop: stop.clone(),
            })
        })
        .collect();
    every.sort_by(|a, b| a.profile.cmp(&b.profile));
    every
}

/// The stops among `names` that wait, taken: what the person's click
/// confirms. A name the orchestrator did not ask for is not among them.
pub(crate) fn take(profile: &str, names: &[String]) -> Vec<StopAsked> {
    let Ok(mut held) = asked().lock() else {
        return Vec::new();
    };
    let Some(list) = held.get_mut(profile) else {
        return Vec::new();
    };
    let (taken, kept) = std::mem::take(list)
        .into_iter()
        .partition(|one| names.contains(&one.name));
    *list = kept;
    taken
}

/// A session stopped some other way waits no more.
pub(crate) fn forget(profile: &str, name: &str) {
    if let Ok(mut held) = asked().lock() {
        if let Some(list) = held.get_mut(profile) {
            list.retain(|one| one.name != name);
        }
    }
}

/// The person's click on `shown`, the stops as they were shown: each still
/// waiting is stopped by `stop` on the loss the click showed for it. One
/// whose loss changed since stops nothing and waits again, showing it.
pub(crate) fn confirm(
    profile: &str,
    shown: &[StopAsked],
    mut stop: impl FnMut(&StopAsked, Option<&str>) -> Result<Clicked, String>,
) -> StopsConfirmed {
    let names: Vec<String> = shown.iter().map(|one| one.name.clone()).collect();
    let mut done = StopsConfirmed {
        stopped: Vec::new(),
        failed: Vec::new(),
        changed: Vec::new(),
    };
    for held in take(profile, &names) {
        let seen = shown
            .iter()
            .find(|one| one.name == held.name)
            .and_then(|one| one.losing.as_deref());
        match stop(&held, seen) {
            Ok(Clicked::Stopped) => done.stopped.push(held.name),
            Ok(Clicked::Changed(now)) => {
                done.changed.push(format!("{}: {now}", held.name));
                hold(
                    profile,
                    StopAsked {
                        losing: Some(now),
                        ..held
                    },
                );
            }
            Err(why) => done.failed.push(format!("{}: {why}", held.name)),
        }
    }
    done
}

/// Stops `shown` as the person of `profile`, from the window or the Remote.
pub(crate) fn confirm_now(app: &AppHandle, profile: &str, shown: &[StopAsked]) -> StopsConfirmed {
    let done = confirm(profile, shown, |one, seen| {
        crate::stopping::stop_as_seen(app, profile, &one.name, one.pid, seen)
            .map_err(|err| err.message)
    });
    let _ = app.emit(STOP_ASKED, profile);
    done
}

/// The person keeps `names` of `profile` running.
pub(crate) fn drop_now(app: &AppHandle, profile: &str, names: &[String]) {
    take(profile, names);
    let _ = app.emit(STOP_ASKED, profile);
}

/// `orchestrator.stops_asked` — the stops waiting for the person's click.
#[tauri::command]
#[specta::specta]
pub fn orchestrator_stops_asked(profile_id: String) -> StopsAsked {
    StopsAsked {
        stops: waiting(&profile_id),
    }
}

/// `orchestrator.stops_confirm` — the person's click on the stops as it
/// showed them: each stops as theirs on the loss it showed, read again now.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_stops_confirm(
    app: AppHandle,
    profile_id: String,
    stops: Vec<StopAsked>,
) -> Result<StopsConfirmed, RpcError> {
    crate::off_main::blocking(move || Ok(confirm_now(&app, &profile_id, &stops))).await
}

/// `orchestrator.stops_drop` — the person keeps `names` running.
#[tauri::command]
#[specta::specta]
pub fn orchestrator_stops_drop(app: AppHandle, profile_id: String, names: Vec<String>) {
    drop_now(&app, &profile_id, &names);
}

/// Whether every session stopped as `name` (only `pid`, when given) has
/// another running session on its card: a swap on the card.
pub(crate) fn swapped(live: &[LiveSession], name: &str, pid: Option<i32>) -> bool {
    let stopped = |one: &LiveSession| one.name == name && pid.is_none_or(|pid| one.pid == pid);
    let mut stopping = live.iter().filter(|one| stopped(one)).peekable();
    stopping.peek().is_some()
        && stopping.all(|one| {
            one.card_id.as_deref().is_some_and(|card| {
                live.iter()
                    .any(|other| !stopped(other) && other.card_id.as_deref() == Some(card))
            })
        })
}

#[cfg(test)]
#[path = "stop_asked_tests.rs"]
mod tests;
