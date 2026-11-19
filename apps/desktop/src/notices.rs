//! The bell, and what rings it.
//!
//! Nothing here invents an event. Every notice is something the app already
//! knew and had nowhere to say: a run ended, an agent stopped and is waiting
//! for a person, a card went past its date. The bell is the place they were
//! missing, not a new source of them.
//!
//! Written where the thing happens and read here, rather than polled: a timer
//! that asks "did anything happen" finds out late and finds out repeatedly.
//! The one exception is deadlines, which are not an event at all — nothing
//! *happens* when a date passes, so something has to look.

use devpit_core::i18n::t;
use devpit_core::Store;
use devpit_rpc::{Notice, Notices, RpcError};
use tauri::Emitter;

use crate::projects::store;

/// How many the panel shows. The store keeps more; a list nobody scrolls past
/// the first screen of does not need to be longer than this.
const SHOWN: i64 = 100;

/// The kinds this build writes. A word rather than an enum in the contract,
/// because the set grows with whatever learns to notice something — but the
/// writers here share one list so a typo is a compile error.
pub mod kind {
    pub const RUN: &str = "run";
    pub const AGENT: &str = "agent";
    pub const DUE: &str = "due";
    pub const IRREVERSIBLE: &str = "irreversible";
    /// A terminal command that took a while finished while nobody looked.
    pub const COMMAND: &str = "command";
    /// devpit installed or updated the devflow program Devflow runs on.
    pub const DEVFLOW: &str = "devflow";
}

/// The event the window listens on so the bell updates without asking.
pub const RANG: &str = "notice:rang";

pub(crate) fn drawn(row: devpit_core::NoticeRow) -> Notice {
    Notice {
        id: row.id,
        project_id: row.project_id,
        kind: row.kind,
        title: row.title,
        detail: row.detail,
        card_id: row.card_id,
        created_at: row.created_at as f64,
        read_at: row.read_at.map(|at| at as f64),
        pane_id: row.pane_id,
    }
}

fn answer(store: &Store) -> Result<Notices, RpcError> {
    Ok(Notices {
        notices: store.notices(SHOWN)?.into_iter().map(drawn).collect(),
        unread: store.unread_notices()?,
    })
}

/// Records one and tells the window.
///
/// Called from wherever the thing happened. Failure is swallowed on purpose:
/// a notice that could not be written must not fail the run it was about.
pub fn ring(
    app: &tauri::AppHandle,
    project_id: Option<&str>,
    kind: &str,
    title: &str,
    detail: Option<&str>,
    card_id: Option<&str>,
) {
    let Ok(store) = Store::open_default() else {
        return;
    };
    if store
        .add_notice(project_id, kind, title, detail, card_id)
        .is_ok()
    {
        let _ = app.emit(RANG, ());
    }
}

/// `ring`, about something that happened in terminal `pane`.
pub fn ring_at(
    app: &tauri::AppHandle,
    project_id: Option<&str>,
    kind: &str,
    title: &str,
    detail: Option<&str>,
    pane: &str,
) {
    let Ok(store) = Store::open_default() else {
        return;
    };
    if store
        .add_pane_notice(project_id, kind, title, detail, None, pane)
        .is_ok()
    {
        let _ = app.emit(RANG, ());
    }
}

/// `ring`, with a Store the caller already opened for the same event, about
/// something that happened in terminal `pane`.
pub(crate) fn ring_in(
    store: &Store,
    app: &tauri::AppHandle,
    project_id: Option<&str>,
    kind: &str,
    title: &str,
    card_id: Option<&str>,
    pane: &str,
) {
    if store
        .add_pane_notice(project_id, kind, title, None, card_id, pane)
        .is_err()
    {
        return;
    }
    let _ = app.emit(RANG, ());
}

/// A run has ended, and the card it was on should say so.
///
/// Here rather than at the call site so `runs.rs` keeps one line of it: what
/// the notice says is a question about notices, not about running things.
pub fn run_ended(app: &tauri::AppHandle, store: &Store, card_id: &str, step: &str, ok: bool) {
    let title = store
        .card(card_id)
        .ok()
        .flatten()
        .map(|row| row.title)
        .unwrap_or_else(|| t("notice.aCard", &[]));
    let project = store.project_id_of_card(card_id).ok().flatten();
    let said = t(
        if ok {
            "notice.stepFinished"
        } else {
            "notice.stepFailed"
        },
        &[("step", step), ("title", &title)],
    );
    if !ok {
        crate::channels::emit(crate::channels::Told {
            event: devpit_rpc::ChannelEvent::StepFailed,
            line: said.clone(),
            bare: t("notice.stepFailedBare", &[]),
            actions: Vec::new(),
            asking: None,
            about: None,
        });
    }
    ring(
        app,
        project.as_deref(),
        kind::RUN,
        &said,
        None,
        Some(card_id),
    );
}

/// `notices.read` — the list, and how many are unread.
#[tauri::command]
#[specta::specta]
pub async fn notices_read() -> Result<Notices, RpcError> {
    crate::off_main::blocking(notices_read_now).await
}

/// [`notices_read`], on the calling thread.
pub(crate) fn notices_read_now() -> Result<Notices, RpcError> {
    answer(&store()?)
}

/// `notices.mark` — marks one read.
#[tauri::command]
#[specta::specta]
pub async fn notices_mark(notice_id: String) -> Result<Notices, RpcError> {
    crate::off_main::blocking(move || notices_mark_now(notice_id)).await
}

/// [`notices_mark`], on the calling thread.
pub(crate) fn notices_mark_now(notice_id: String) -> Result<Notices, RpcError> {
    let store = store()?;
    store.read_notice(&notice_id)?;
    answer(&store)
}

/// `notices.mark_all` — marks every one read.
///
/// Read, never deleted: a list that empties itself is a list where something
/// you meant to come back to is gone.
#[tauri::command]
#[specta::specta]
pub async fn notices_mark_all() -> Result<Notices, RpcError> {
    crate::off_main::blocking(notices_mark_all_now).await
}

/// [`notices_mark_all`], on the calling thread.
pub(crate) fn notices_mark_all_now() -> Result<Notices, RpcError> {
    let store = store()?;
    store.read_all_notices()?;
    answer(&store)
}

/// `notices.sweep_due` — notices the deadlines that have passed.
///
/// A sweep and not an event, because a date passing is not something that
/// happens — nothing calls anybody when a clock ticks over. Idempotent: a card
/// already noticed is not noticed again, which is what makes it safe to call
/// on every launch and every board read.
#[tauri::command]
#[specta::specta]
pub async fn notices_sweep_due(app: tauri::AppHandle) -> Result<Notices, RpcError> {
    crate::off_main::blocking(move || notices_sweep_due_now(app.clone())).await
}

/// [`notices_sweep_due`], on the calling thread.
pub(crate) fn notices_sweep_due_now(app: tauri::AppHandle) -> Result<Notices, RpcError> {
    // One sweep at a time: it reads which cards were already told about and
    // then writes the rest, and two sweeps at once — both windows answering
    // the same timer — would each write the same notice.
    static SWEEPING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = SWEEPING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let store = store()?;
    // With reminders on, a date going off is theirs to tell, at its time; this
    // would tell it a second time, as a date gone by.
    if crate::reminders::enabled(&store) {
        return answer(&store);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default();

    let already: std::collections::HashSet<String> =
        store.cards_noticed(kind::DUE)?.into_iter().collect();

    let mut rang = false;
    for (card_id, project_id, title) in store.overdue_cards(now)? {
        if already.contains(&card_id) {
            continue;
        }
        store.add_notice(
            Some(&project_id),
            kind::DUE,
            &t("notice.pastDate", &[("title", &title)]),
            None,
            Some(&card_id),
        )?;
        rang = true;
    }
    if rang {
        let _ = app.emit(RANG, ());
    }

    answer(&store)
}
