//! Telegram, through the person's own bot: devpit's notices sent from this
//! machine, and their buttons answered here. No devpit.app in between.
//!
//! The token is kept in `telegram.json` under devpit's root, readable by the
//! person alone, and never shown, logged or put in an error — an HTTP error
//! names its URL, and a Bot API URL holds the token, so none is passed on.
//! Linking: devpit shows a code, the person sends `/start <code>` to their
//! bot, and that chat is the one devpit talks to and listens to; any other
//! chat is counted and ignored, its words never read. What the paired chat
//! says goes to an orchestrator through `channel_talk`.
//!
//! Without a bot of their own, the person can link a chat to devpit's bot on
//! devpit.app instead (`telegram_devpit`): one Telegram channel either way.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use devpit_rpc::{ChannelInfo, Privacy, RpcError, TelegramStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::channel_talk::Heard;

pub(crate) const ID: &str = "telegram";
const API: &str = "https://api.telegram.org";
/// How long a linking code holds.
const CODE_SECS: i64 = 10 * 60;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Kept {
    token: String,
    bot: String,
    chat_id: Option<i64>,
    code: Option<String>,
    code_until: i64,
    /// What a file written before `privacy` said: names, or nothing named.
    titles: bool,
    privacy: Option<Privacy>,
    /// The orchestrator the chat talks to, by project.
    orchestrator: Option<String>,
}

impl Kept {
    fn privacy(&self) -> Privacy {
        self.privacy.unwrap_or(if self.titles {
            Privacy::Titles
        } else {
            Privacy::Minimal
        })
    }
}

fn file() -> Option<std::path::PathBuf> {
    devpit_core::Store::root()
        .ok()
        .map(|root| root.join("telegram.json"))
}

fn read() -> Option<Kept> {
    serde_json::from_str(&std::fs::read_to_string(file()?).ok()?).ok()
}

fn write(kept: &Kept) -> Result<(), RpcError> {
    let path = file().ok_or_else(|| RpcError::internal("no home directory to write to"))?;
    let text = serde_json::to_vec(kept).map_err(|err| RpcError::internal(err.to_string()))?;
    devpit_core::home::write_private(&path, &text)
        .map_err(|err| RpcError::internal(err.to_string()))
}

pub(crate) fn app() -> &'static OnceLock<AppHandle> {
    static APP: OnceLock<AppHandle> = OnceLock::new();
    &APP
}

/// The person's own bot, with a chat linked.
fn own() -> Option<Kept> {
    read().filter(|kept| kept.chat_id.is_some())
}

/// The channel, when a chat is linked: through the person's own bot, or else
/// through devpit's.
pub(crate) fn connected() -> Option<ChannelInfo> {
    let Some(kept) = own() else {
        return crate::telegram_devpit::connected();
    };
    Some(ChannelInfo {
        id: ID.to_owned(),
        label: format!("Telegram, @{}", kept.bot),
    })
}

/// Whether messages say what they are about, or only that something happened.
pub(crate) fn titles() -> bool {
    match read() {
        Some(kept) => kept.privacy() != Privacy::Minimal,
        None => crate::telegram_devpit::titles(),
    }
}

/// A call to the Bot API. Any failure is one sentence with no URL in it.
async fn call(token: &str, method: &str, body: Value) -> Result<Value, String> {
    let answer = crate::account::client()
        .map_err(|err| err.message)?
        .post(format!("{API}/bot{token}/{method}"))
        .json(&body)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|_| "Telegram did not answer".to_owned())?;
    let said: Value = answer
        .json()
        .await
        .map_err(|_| "Telegram's answer did not read".to_owned())?;
    if said["ok"] != true {
        return Err(said["description"]
            .as_str()
            .unwrap_or("Telegram refused it")
            .to_owned());
    }
    Ok(said["result"].clone())
}

/// Sends `text` to the linked chat, with a button per action.
pub(crate) fn send(text: &str, offered: &[crate::channels::Action]) {
    if own().is_none() {
        return crate::telegram_devpit::send(text, offered);
    }
    let (text, offered) = (text.to_owned(), offered.to_vec());
    tauri::async_runtime::spawn(async move { said(&text, &offered).await });
}

/// [`send`], done before it answers.
async fn said(text: &str, offered: &[crate::channels::Action]) {
    let Some(kept) = read() else { return };
    let Some(chat) = kept.chat_id else { return };
    let row: Vec<Value> = offered
        .iter()
        .map(|action| {
            let key = crate::channel_actions::remember(action);
            json!({ "text": crate::channel_actions::label(action), "callback_data": format!("a:{key}") })
        })
        .collect();
    let mut body = json!({ "chat_id": chat, "text": text });
    if !row.is_empty() {
        body["reply_markup"] = json!({ "inline_keyboard": [row] });
    }
    if let Err(why) = call(&kept.token, "sendMessage", body).await {
        devpit_core::reports::background("telegram", &why);
    }
}

/// The bot, as the paired chat's side of a talk with an orchestrator.
struct Bot;

impl crate::channel_talk::Line for Bot {
    fn id(&self) -> &'static str {
        ID
    }

    fn privacy(&self) -> Privacy {
        read().map(|kept| kept.privacy()).unwrap_or_default()
    }

    fn orchestrator(&self) -> Option<String> {
        read()?.orchestrator
    }

    async fn reply(&self, text: &str, actions: &[crate::channels::Action]) {
        said(text, actions).await;
    }

    async fn fetch(&self, file: &str, most: usize) -> Result<Vec<u8>, String> {
        let kept = read().ok_or("no bot is set up")?;
        let found = call(&kept.token, "getFile", json!({ "file_id": file })).await?;
        if found["file_size"]
            .as_u64()
            .is_some_and(|size| size > most as u64)
        {
            return Err(crate::channel_talk::past_most(most));
        }
        let path = found["file_path"]
            .as_str()
            .ok_or("Telegram did not say where the voice note is")?;
        // A download URL holds the token too: its errors are not passed on.
        let mut answer = crate::account::client()
            .map_err(|err| err.message)?
            .get(format!("{API}/file/bot{}/{path}", kept.token))
            .timeout(Duration::from_secs(60))
            .send()
            .await
            .map_err(|_| "Telegram did not send the voice note".to_owned())?;
        let mut held = Vec::new();
        while let Some(piece) = answer
            .chunk()
            .await
            .map_err(|_| "the voice note stopped arriving".to_owned())?
        {
            crate::channel_talk::gather(&mut held, &piece, most)?;
        }
        Ok(held)
    }
}

/// What a code the person sent links, if it is the one devpit showed.
pub(crate) fn links(kept: &Kept, said: &str, now: i64) -> bool {
    let Some(code) = kept.code.as_deref() else {
        return false;
    };
    let said = said.trim();
    let said = said.strip_prefix("/start").map(str::trim).unwrap_or(said);
    now < kept.code_until && said.eq_ignore_ascii_case(code)
}

/// One update from the bot, read against what devpit keeps.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Update {
    /// The linking code devpit showed, sent from this private chat.
    Links(i64),
    /// The paired chat said something to its orchestrator.
    Said(Heard),
    /// A button pressed: its key, when it was pressed in the paired chat.
    Pressed { query: String, key: Option<String> },
    /// A message from a chat that is not the paired one: counted, not read.
    Unpaired,
    /// Nothing devpit acts on: `/start` again, a sticker, a photo.
    Nothing,
}

/// What `update` is: only the paired chat is the person.
pub(crate) fn read_update(kept: &Kept, update: &Value, now: i64) -> Update {
    if let Some(pressed) = update.get("callback_query") {
        let paired =
            kept.chat_id.is_some() && pressed["message"]["chat"]["id"].as_i64() == kept.chat_id;
        return Update::Pressed {
            query: pressed["id"].as_str().unwrap_or_default().to_owned(),
            key: pressed["data"]
                .as_str()
                .and_then(|data| data.strip_prefix("a:"))
                .filter(|_| paired)
                .map(str::to_owned),
        };
    }
    let Some(message) = update.get("message") else {
        return Update::Nothing;
    };
    let chat = message["chat"]["id"].as_i64();
    let text = message["text"].as_str();
    if message["chat"]["type"] == "private" && links(kept, text.unwrap_or_default(), now) {
        return chat.map_or(Update::Nothing, Update::Links);
    }
    if kept.chat_id.is_none() || chat != kept.chat_id {
        return Update::Unpaired;
    }
    if let Some(text) = text {
        if text.trim_start().starts_with("/start") {
            return Update::Nothing;
        }
        return Update::Said(Heard::Text(text.to_owned()));
    }
    let voice = &message["voice"];
    match voice["file_id"].as_str() {
        Some(file) => Update::Said(Heard::Voice {
            file: file.to_owned(),
            media_type: voice["mime_type"]
                .as_str()
                .unwrap_or("audio/ogg")
                .to_owned(),
            size: voice["file_size"]
                .as_u64()
                .map(|size| usize::try_from(size).unwrap_or(usize::MAX)),
        }),
        None => Update::Nothing,
    }
}

/// Messages from chats other than the paired one, since devpit started.
static IGNORED: AtomicU64 = AtomicU64::new(0);

/// One update from the bot: a linking code, the paired chat talking, or a
/// button pressed there.
async fn heard(update: &Value) {
    let Some(mut kept) = read() else { return };
    let now = devpit_core::reports::now() as i64;
    match read_update(&kept, update, now) {
        Update::Links(chat) => {
            kept.chat_id = Some(chat);
            kept.code = None;
            if write(&kept).is_ok() {
                let _ = call(&kept.token, "sendMessage", json!({ "chat_id": chat, "text": "Linked: devpit tells you here what waits on you, and what you write here goes to your orchestrator." })).await;
            }
        }
        Update::Said(said) => {
            let Some(app) = app().get().cloned() else {
                return;
            };
            // A task of its own: a turn takes a while, and a button pressed
            // meanwhile is still answered.
            tauri::async_runtime::spawn(async move {
                crate::channel_talk::heard(&Bot, &crate::channel_talk::Here(app), said).await;
            });
        }
        Update::Pressed { query, key } => {
            let action = key.and_then(|key| crate::channel_actions::take(&key));
            let said = match action {
                // Off the runtime: a stop waits to see its processes gone.
                Some(action) => tauri::async_runtime::spawn_blocking(move || {
                    crate::channel_actions::act(&action, "Telegram")
                })
                .await
                .unwrap_or_else(|_| "That button could not be acted on.".to_owned()),
                None => "That button has expired.".to_owned(),
            };
            let _ = call(
                &kept.token,
                "answerCallbackQuery",
                json!({ "callback_query_id": query, "text": said }),
            )
            .await;
        }
        // Counted, never read: whoever else writes to the bot is not the person.
        Update::Unpaired => {
            let ignored = IGNORED.fetch_add(1, Ordering::Relaxed) + 1;
            eprintln!("telegram: {ignored} message(s) from chats not paired with devpit ignored");
        }
        Update::Nothing => {}
    }
}

static LISTENING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Listens to the bot while one is set up: long polling, from this machine.
pub(crate) fn listen(app: &AppHandle) {
    let _ = self::app().set(app.clone());
    if read().is_none() || LISTENING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async {
        let mut offset = 0i64;
        while let Some(kept) = read() {
            match call(&kept.token, "getUpdates", json!({ "offset": offset, "timeout": 50, "allowed_updates": ["message", "callback_query"] })).await {
                Ok(Value::Array(updates)) => {
                    for update in &updates {
                        offset = offset.max(update["update_id"].as_i64().unwrap_or(0) + 1);
                        heard(update).await;
                    }
                }
                _ => tokio::time::sleep(Duration::from_secs(30)).await,
            }
        }
        LISTENING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

fn status(kept: Option<&Kept>) -> TelegramStatus {
    TelegramStatus {
        linked: kept.is_some_and(|kept| kept.chat_id.is_some()),
        bot: kept.map(|kept| kept.bot.clone()),
        code: kept.and_then(|kept| kept.code.clone()),
        link: kept.and_then(|kept| {
            kept.code
                .as_ref()
                .map(|code| format!("https://t.me/{}?start={code}", kept.bot))
        }),
        privacy: kept.map(Kept::privacy).unwrap_or_default(),
        orchestrator: kept.and_then(|kept| kept.orchestrator.clone()),
        orchestrators: crate::channel_talk::orchestrators(),
        devpit_bot: false,
        through_devpit: false,
    }
}

/// [`status`] off the main thread: the orchestrators are read from the store.
async fn status_now(kept: Option<Kept>) -> Result<TelegramStatus, RpcError> {
    crate::off_main::blocking(move || Ok(status(kept.as_ref()))).await
}

/// `telegram.status` — the person's own bot, or else devpit's, as devpit.app
/// tells it.
#[tauri::command]
#[specta::specta]
pub async fn telegram_status() -> Result<TelegramStatus, RpcError> {
    match read() {
        Some(kept) => status_now(Some(kept)).await,
        None => crate::telegram_devpit::status().await,
    }
}

/// `telegram.use_devpit` — devpit's own bot instead of one of the person's:
/// a single-use code from devpit.app, and the link that sends it.
#[tauri::command]
#[specta::specta]
pub async fn telegram_use_devpit() -> Result<TelegramStatus, RpcError> {
    crate::telegram_devpit::link().await
}

/// `telegram.link` — takes the person's bot token, checks it, and offers a code
/// to send to the bot from the chat devpit should use.
#[tauri::command]
#[specta::specta]
pub async fn telegram_link(app: AppHandle, token: String) -> Result<TelegramStatus, RpcError> {
    let token = token.trim().to_owned();
    if token.is_empty()
        || token.len() > 100
        || !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-'))
    {
        return Err(RpcError::new(
            devpit_rpc::ErrorCode::Invalid,
            "that is not a bot token from @BotFather",
        ));
    }
    let me = call(&token, "getMe", json!({})).await.map_err(|_| {
        RpcError::new(
            devpit_rpc::ErrorCode::Invalid,
            "Telegram does not know that token",
        )
    })?;
    let code: String = ulid::Ulid::generate()
        .to_string()
        .chars()
        .rev()
        .take(6)
        .collect();
    let was = read().unwrap_or_default();
    let kept = Kept {
        token,
        bot: me["username"].as_str().unwrap_or_default().to_owned(),
        chat_id: None,
        code: Some(code),
        code_until: devpit_core::reports::now() as i64 + CODE_SECS,
        titles: was.titles,
        privacy: was.privacy,
        orchestrator: was.orchestrator,
    };
    write(&kept)?;
    listen(&app);
    status_now(Some(kept)).await
}

fn set_up() -> Result<Kept, RpcError> {
    read().ok_or_else(|| RpcError::new(devpit_rpc::ErrorCode::NotFound, "no bot is set up"))
}

/// `telegram.privacy_set` — how much of what devpit says goes to the chat.
#[tauri::command]
#[specta::specta]
pub async fn telegram_privacy_set(privacy: Privacy) -> Result<TelegramStatus, RpcError> {
    if read().is_none() {
        crate::telegram_devpit::privacy_set(privacy)?;
        return crate::telegram_devpit::status().await;
    }
    let mut kept = set_up()?;
    kept.privacy = Some(privacy);
    write(&kept)?;
    status_now(Some(kept)).await
}

/// `telegram.orchestrator_set` — which orchestrator the chat talks to; none
/// is the only one there is.
#[tauri::command]
#[specta::specta]
pub async fn telegram_orchestrator_set(
    project: Option<String>,
) -> Result<TelegramStatus, RpcError> {
    if let Some(chosen) = &project {
        let known = crate::off_main::blocking(|| Ok(crate::channel_talk::orchestrators())).await?;
        if !known.iter().any(|one| &one.project == chosen) {
            return Err(RpcError::new(
                devpit_rpc::ErrorCode::NotFound,
                "that is not an orchestrator here",
            ));
        }
    }
    if read().is_none() {
        crate::telegram_devpit::orchestrator_set(project)?;
        return crate::telegram_devpit::status().await;
    }
    let mut kept = set_up()?;
    kept.orchestrator = project;
    write(&kept)?;
    status_now(Some(kept)).await
}

/// `telegram.unlink` — forgets the bot and the chat: the person's own, or
/// else the chat linked to devpit's.
#[tauri::command]
#[specta::specta]
pub async fn telegram_unlink() -> Result<TelegramStatus, RpcError> {
    match (read(), file()) {
        (Some(_), Some(path)) => {
            let _ = std::fs::remove_file(path);
        }
        _ => crate::telegram_devpit::unlink().await,
    }
    crate::telegram_devpit::status().await
}

#[cfg(test)]
#[path = "telegram_tests.rs"]
mod tests;
