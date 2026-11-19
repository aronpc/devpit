//! devpit.app as a channel: notices go to the account's hub, which e-mails
//! them and wakes its push devices; the buttons pressed there come back here
//! to be done. What goes is what the person chose to say — by default only
//! that something waits — and the buttons as keys only this machine reads.

use std::sync::Mutex;
use std::time::Duration;

use devpit_core::preference;
use devpit_rpc::{ChannelInfo, HubStatus, RpcError};
use serde::Deserialize;
use serde_json::json;

use crate::channels::Action;

pub(crate) const ID: &str = "devpit_app";
/// How often pressed buttons are asked for while the hub is on.
const EVERY: Duration = Duration::from_secs(20);

fn flag(key: &str) -> bool {
    crate::projects::store()
        .ok()
        .and_then(|store| store.preference_flag(key).ok().flatten())
        .unwrap_or(false)
}

fn status() -> HubStatus {
    HubStatus {
        on: flag(preference::CHANNEL_HUB),
        titles: flag(preference::CHANNEL_HUB_TITLES),
        signed_in: crate::account::token::read().is_some(),
    }
}

/// The channel, when switched on and signed in.
pub(crate) fn connected() -> Option<ChannelInfo> {
    let now = status();
    (now.on && now.signed_in).then(|| ChannelInfo {
        id: ID.to_owned(),
        label: "devpit.app (e-mail, push)".to_owned(),
    })
}

pub(crate) fn titles() -> bool {
    flag(preference::CHANNEL_HUB_TITLES)
}

/// Posts a notice to the hub, on the channels in `via` — `push`, `email` —
/// or on every one when none is named.
pub(crate) fn send(text: &str, offered: &[Action], via: &[&str]) {
    let Some(token) = crate::account::token::read() else {
        return;
    };
    let buttons: Vec<_> = offered
        .iter()
        .map(|action| json!({ "action": crate::channel_actions::remember(action), "label": crate::channel_actions::label(action) }))
        .collect();
    let body = json!({ "text": text, "buttons": buttons, "via": via });
    tauri::async_runtime::spawn(async move {
        let sent = match crate::account::client() {
            Ok(client) => client
                .post(format!("{}/v1/hub/events", crate::account::origin()))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .map(|answer| answer.status()),
            Err(_) => return,
        };
        match sent {
            Ok(status) if status.is_success() => {}
            Ok(status) => {
                devpit_core::reports::background("hub", &format!("devpit.app answered {status}"))
            }
            Err(_) => devpit_core::reports::background("hub", &"devpit.app did not answer"),
        }
    });
}

#[derive(Deserialize)]
struct Pending {
    pressed: Vec<Pressed>,
    /// Absent from a devpit.app that predates it.
    #[serde(default)]
    reach: Option<Reach>,
    /// What the chat linked to devpit's bot wrote, when asked for.
    #[serde(default)]
    said: Vec<Said>,
}

#[derive(Deserialize)]
struct Said {
    text: String,
}

#[derive(Deserialize)]
struct Pressed {
    action: String,
}

/// Which of the account's channels a notice would reach, as devpit.app said.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Reach {
    pub push: bool,
    pub email: bool,
    /// A chat linked to devpit's bot.
    #[serde(default)]
    pub telegram: bool,
}

static REACH: Mutex<Option<Reach>> = Mutex::new(None);

/// What devpit.app last said a notice would reach; none until it has said.
pub(crate) fn reach() -> Option<Reach> {
    REACH.lock().ok().and_then(|held| *held)
}

/// Picks up the buttons pressed on the person's channels and does them here,
/// and what the chat linked to devpit's bot wrote, for as long as the app runs.
pub(crate) fn listen() {
    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(crate::maintenance::stretch(EVERY)).await;
            // Only a machine that linked devpit's bot takes what its chat wrote.
            let bot = crate::telegram_devpit::linked();
            if connected().is_none() && !bot {
                continue;
            }
            let (Some(token), Ok(client)) =
                (crate::account::token::read(), crate::account::client())
            else {
                continue;
            };
            let Ok(answer) = client
                .get(format!(
                    "{}/v1/hub/actions?said={bot}",
                    crate::account::origin()
                ))
                .bearer_auth(token)
                .send()
                .await
            else {
                continue;
            };
            let Ok(pending) = answer.json::<Pending>().await else {
                continue;
            };
            if let Some(reach) = pending.reach {
                crate::telegram_devpit::seen(reach.telegram);
            }
            if let (Some(said), Ok(mut held)) = (pending.reach, REACH.lock()) {
                *held = Some(said);
            }
            crate::telegram_devpit::heard(pending.said.into_iter().map(|one| one.text).collect());
            for pressed in pending.pressed {
                // A key from before a restart names nothing: that button has expired.
                if let Some(action) = crate::channel_actions::take(&pressed.action) {
                    // Off the runtime: a stop waits to see its processes gone.
                    let said = tauri::async_runtime::spawn_blocking(move || {
                        crate::channel_actions::act(&action, "devpit.app")
                    })
                    .await
                    .unwrap_or_else(|_| "that button could not be acted on".to_owned());
                    devpit_core::reports::background(
                        "hub",
                        &format!("a button pressed on devpit.app: {said}"),
                    );
                }
            }
        }
    });
}

/// `hub.status`
#[tauri::command]
#[specta::specta]
pub async fn hub_status() -> Result<HubStatus, RpcError> {
    crate::off_main::blocking(|| Ok(status())).await
}

/// `hub.set` — whether notices go through devpit.app, and whether they name things.
#[tauri::command]
#[specta::specta]
pub async fn hub_set(on: bool, titles: bool) -> Result<HubStatus, RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        store.set_preference_flag(preference::CHANNEL_HUB, on)?;
        store.set_preference_flag(preference::CHANNEL_HUB_TITLES, titles)?;
        Ok(status())
    })
    .await
}
