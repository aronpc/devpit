//! Telling the person, through the system, when an agent needs them and
//! devpit is not in front.
//!
//! Only on a change, and only for what is worth leaving another window for:
//! a session that stopped on you, failed, or finished a turn it was working
//! on. Working itself never notifies — that is the island's to show.

use devpit_core::i18n::t;
use devpit_rpc::{Doing, HeadsDown, IslandSession};
use tauri::Manager as _;
use tauri_plugin_notification::NotificationExt as _;

/// What a change is worth telling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tell {
    Waiting,
    Failed,
    Done,
}

/// Whether a session going from `was` to `now` is worth a notification.
pub(crate) fn worth_telling(was: Option<Doing>, now: Doing) -> Option<Tell> {
    if was == Some(now) {
        return None;
    }
    match now {
        Doing::Waiting => Some(Tell::Waiting),
        Doing::Failed => Some(Tell::Failed),
        // Only a turn it was seen working on: a session that opens and is
        // already done has nothing anybody was waiting for.
        Doing::Done if was == Some(Doing::Working) => Some(Tell::Done),
        _ => None,
    }
}

/// Whether a focus holds this project's news back, the way it holds the bell:
/// somebody heads down in one project hears from that one alone.
pub(crate) fn held_by(focus: Option<&HeadsDown>, project_id: Option<&str>) -> bool {
    focus.is_some_and(|focus| project_id != Some(focus.project_id.as_str()))
}

/// The words of a notification: who, and what happened.
pub(crate) fn words(session: &IslandSession, tell: Tell) -> (String, String) {
    let who = session
        .card
        .clone()
        .or_else(|| session.project.clone())
        .unwrap_or_else(|| t("notify.anAgent", &[]));
    let body = match tell {
        Tell::Waiting => t("notify.waiting", &[]),
        Tell::Failed => session.said.as_deref().map_or_else(
            || t("notify.stoppedOnError", &[]),
            |said| t("notify.stopped", &[("said", said)]),
        ),
        Tell::Done => session
            .said
            .clone()
            .unwrap_or_else(|| t("notify.finishedTurn", &[])),
    };
    (who, body)
}

/// Whether devpit's own window is the one somebody is looking at, in which
/// case the bell is enough.
pub(crate) fn in_front(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|main| main.is_focused().ok())
        .unwrap_or(false)
}

pub(crate) fn show(app: &tauri::AppHandle, title: &str, body: &str, urgent: bool) {
    let _ = app.notification().builder().title(title).body(body).show();
    if urgent {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
    }
}

/// A session changed: tells the person when it is worth it.
///
/// Off the hook's thread: asking the window whether it is in front waits on
/// the event loop, and a hook's thread waits on nothing.
pub(crate) fn changed(app: &tauri::AppHandle, session: &IslandSession, was: Option<Doing>) {
    let Some(tell) = worth_telling(was, session.state) else {
        return;
    };
    if crate::pausing::paused() {
        return;
    }
    let (title, body) = words(session, tell);
    crate::channels::emit(crate::channels::Told {
        event: match tell {
            Tell::Waiting => devpit_rpc::ChannelEvent::SessionWaiting,
            Tell::Failed => devpit_rpc::ChannelEvent::SessionFailed,
            Tell::Done => devpit_rpc::ChannelEvent::SessionDone,
        },
        line: format!("{title}: {body}"),
        bare: t(
            match tell {
                Tell::Waiting => "notify.bareWaiting",
                Tell::Failed => "notify.bareFailed",
                Tell::Done => "notify.bareDone",
            },
            &[],
        ),
        actions: Vec::new(),
        asking: match (tell, &session.project_id, &session.pane_id) {
            (Tell::Waiting, Some(project), Some(pane)) => {
                crate::pane_io::target_of(project, pane).ok()
            }
            _ => None,
        },
        about: Some(crate::channel_escalation::About::Session {
            id: session.session_id.clone(),
            state: session.state,
        }),
    });
    let project = session.project_id.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let focus = crate::heads_down::focus_read_now().ok().flatten();
        if !in_front(&app) && !held_by(focus.as_ref(), project.as_deref()) {
            show(&app, &title, &body, tell == Tell::Waiting);
        }
    });
}

/// An agent asked to be allowed something: always worth telling, unless
/// devpit is in front, where the question is already on screen.
pub(crate) fn asked(app: &tauri::AppHandle, tool: &str, input: &str) {
    if crate::pausing::paused() {
        return;
    }
    let input = serde_json::from_str(input).unwrap_or_default();
    let what = devpit_agentcli::target_of(&input)
        .map_or_else(|| tool.to_owned(), |target| format!("{tool} {target}"));
    crate::channels::emit(crate::channels::Told {
        event: devpit_rpc::ChannelEvent::SessionWaiting,
        line: t("notify.asks", &[("what", &what)]),
        bare: t("notify.asksBare", &[]),
        actions: Vec::new(),
        asking: None,
        // Held two minutes at most; the session's own waiting notice moves on.
        about: None,
    });
    let app = app.clone();
    std::thread::spawn(move || {
        if !in_front(&app) {
            show(&app, &t("notify.asksTitle", &[]), &what, true);
        }
    });
}

#[cfg(test)]
#[path = "island_notify_tests.rs"]
mod tests;
