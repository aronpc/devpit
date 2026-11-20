//! How much of each registered account's plan is left, read live.
//!
//! The same undocumented endpoint `plan_limits` reads, once per account. The
//! access token is read from `<dir>/.credentials.json`, sent, and dropped: it
//! is never logged, never put in an error and never in what the window gets.
//! It is never renewed either — that is the CLI's, and a renewal here would
//! race the session using it. A lapsed one says to open a session there.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use devpit_rpc::{AccountQuota, AccountQuotas, ClaudeAccount, PlanWindow, QuotaState, RpcError};
use serde_json::Value;

use crate::plan_limits::{OAUTH_BETA, USAGE_URL, USER_AGENT};

/// A read is kept this long; the screen asks more often than that.
pub(crate) const FRESH_FOR: Duration = Duration::from_secs(60);
/// After a 429 the endpoint is left alone this long.
pub(crate) const COOL_DOWN: Duration = Duration::from_secs(5 * 60);
/// Past this share of the 5-hour window, a new session belongs elsewhere.
pub(crate) const NEARLY_OUT: f64 = 90.0;
/// A `.credentials.json` also keeps MCP tokens; never read past this.
const CREDENTIALS_CEILING: u64 = 1024 * 1024;

/// A saved access token. No `Debug`, no `Display`, no `Serialize`: the one
/// way out of it is the request header.
pub(crate) struct Token(String);

impl Token {
    fn bearer(&self) -> &str {
        &self.0
    }
}

/// What a `.credentials.json` holds of a sign-in, as of `now_ms`.
pub(crate) enum Saved {
    Usable(Token),
    Expired,
    SignedOut,
}

pub(crate) fn saved_in(credentials: &str, now_ms: f64) -> Saved {
    let Ok(value) = serde_json::from_str::<Value>(credentials.trim()) else {
        return Saved::SignedOut;
    };
    let oauth = value.get("claudeAiOauth");
    let Some(token) = oauth
        .and_then(|one| one.get("accessToken"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    else {
        return Saved::SignedOut;
    };
    // The access token's own expiry, not the refresh token's: an expired one
    // is refused, and renewing it is the CLI's job.
    match oauth
        .and_then(|one| one.get("expiresAt"))
        .and_then(Value::as_f64)
    {
        Some(at) if at <= now_ms => Saved::Expired,
        _ => Saved::Usable(Token(token.to_owned())),
    }
}

/// What one call to the endpoint came back with, the token already gone.
pub(crate) enum Answer {
    Windows(Vec<PlanWindow>),
    Refused,
    TooMany,
    Failed,
}

fn five_hour(windows: &[PlanWindow]) -> Option<f64> {
    windows
        .iter()
        .find(|one| one.label == "5-hour")
        .map(|one| one.percent)
}

/// Whether a new session should open in another account.
pub(crate) fn nearly_out(windows: &[PlanWindow]) -> bool {
    five_hour(windows).is_some_and(|percent| percent > NEARLY_OUT)
}

/// Whether the 5-hour window is used up: a step stops only on this, not on
/// `nearly_out`, because what is merely low can still run.
pub(crate) fn exhausted(windows: &[PlanWindow]) -> bool {
    five_hour(windows).is_some_and(|percent| percent >= 100.0)
}

/// One account's quota from what was read, as the window gets it.
pub(crate) fn quota_of(account: &ClaudeAccount, answer: Answer, read_at: f64) -> AccountQuota {
    let (state, windows) = match answer {
        Answer::Windows(windows) => (QuotaState::Read, windows),
        Answer::Refused => (QuotaState::Expired, Vec::new()),
        Answer::TooMany | Answer::Failed => (QuotaState::Unavailable, Vec::new()),
    };
    AccountQuota {
        name: account.name.clone(),
        config_dir: account.config_dir.clone(),
        nearly_out: nearly_out(&windows),
        state,
        windows,
        read_at,
    }
}

fn said(account: &ClaudeAccount, state: QuotaState) -> AccountQuota {
    AccountQuota {
        name: account.name.clone(),
        config_dir: account.config_dir.clone(),
        state,
        windows: Vec::new(),
        nearly_out: false,
        read_at: now(),
    }
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs_f64())
        .unwrap_or_default()
}

/// What is remembered per directory: the last read, and a 429's cool-down.
#[derive(Default)]
pub(crate) struct Memory {
    read: HashMap<String, (Instant, AccountQuota)>,
    cooling: HashMap<String, Instant>,
}

impl Memory {
    /// A read still fresh, or one to stand in while the endpoint cools down.
    pub(crate) fn kept(&self, dir: &str, now: Instant) -> Option<AccountQuota> {
        if let Some(since) = self.cooling.get(dir) {
            if now.duration_since(*since) < COOL_DOWN {
                return Some(
                    self.read
                        .get(dir)
                        .map(|(_, quota)| AccountQuota {
                            state: QuotaState::Unavailable,
                            ..quota.clone()
                        })
                        .unwrap_or_else(|| AccountQuota {
                            name: String::new(),
                            config_dir: dir.to_owned(),
                            state: QuotaState::Unavailable,
                            windows: Vec::new(),
                            nearly_out: false,
                            read_at: 0.0,
                        }),
                );
            }
        }
        self.read
            .get(dir)
            .filter(|(at, _)| now.duration_since(*at) < FRESH_FOR)
            .map(|(_, quota)| quota.clone())
    }

    pub(crate) fn heard(&mut self, dir: &str, quota: &AccountQuota, too_many: bool, now: Instant) {
        if too_many {
            self.cooling.insert(dir.to_owned(), now);
        } else {
            self.cooling.remove(dir);
            self.read.insert(dir.to_owned(), (now, quota.clone()));
        }
    }
}

fn memory() -> &'static Mutex<Memory> {
    static MEMORY: OnceLock<Mutex<Memory>> = OnceLock::new();
    MEMORY.get_or_init(Mutex::default)
}

fn read_credentials(dir: &str) -> Option<String> {
    let file = std::fs::File::open(std::path::Path::new(dir).join(".credentials.json")).ok()?;
    let mut text = String::new();
    std::io::Read::read_to_string(
        &mut std::io::Read::take(file, CREDENTIALS_CEILING),
        &mut text,
    )
    .ok()?;
    Some(text)
}

/// One call. Every error is collapsed to a variant before it leaves, so no
/// error string that might carry the request reaches anything.
async fn ask(client: &reqwest::Client, url: &str, token: Token) -> Answer {
    let sent = client
        .get(url)
        .bearer_auth(token.bearer())
        .header("anthropic-beta", OAUTH_BETA)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await;
    drop(token);
    let Ok(response) = sent else {
        return Answer::Failed;
    };
    match response.status() {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => Answer::Refused,
        reqwest::StatusCode::TOO_MANY_REQUESTS => Answer::TooMany,
        status if status.is_success() => match response.json::<Value>().await {
            Ok(body) => Answer::Windows(crate::plan_limits::windows_of(&body)),
            Err(_) => Answer::Failed,
        },
        _ => Answer::Failed,
    }
}

/// [`quota_at`] against Anthropic's endpoint.
async fn quota(client: &reqwest::Client, account: &ClaudeAccount) -> AccountQuota {
    quota_at(client, USAGE_URL, account, memory()).await
}

/// One account's quota from `url`, through `memory`'s cache and cool-down.
pub(crate) async fn quota_at(
    client: &reqwest::Client,
    url: &str,
    account: &ClaudeAccount,
    memory: &Mutex<Memory>,
) -> AccountQuota {
    let dir = account.config_dir.as_str();
    if let Some(kept) = memory
        .lock()
        .ok()
        .and_then(|mem| mem.kept(dir, Instant::now()))
    {
        return AccountQuota {
            name: account.name.clone(),
            ..kept
        };
    }
    let now_ms = now() * 1000.0;
    let token = match read_credentials(dir).map(|text| saved_in(&text, now_ms)) {
        Some(Saved::Usable(token)) => token,
        Some(Saved::Expired) => return said(account, QuotaState::Expired),
        Some(Saved::SignedOut) | None => return said(account, QuotaState::SignedOut),
    };
    let answer = ask(client, url, token).await;
    let too_many = matches!(answer, Answer::TooMany);
    let read = quota_of(account, answer, now());
    if let Ok(mut mem) = memory.lock() {
        mem.heard(dir, &read, too_many, Instant::now());
    }
    read
}

/// `accounts.quota` — every registered account's quota windows.
#[tauri::command]
#[specta::specta]
pub async fn accounts_quota() -> Result<AccountQuotas, RpcError> {
    let accounts = crate::off_main::blocking(|| Ok(crate::claude_accounts::read())).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| RpcError::internal("the quota could not be asked"))?;
    let mut all = Vec::new();
    for account in &accounts.accounts {
        all.push(quota(&client, account).await);
    }
    Ok(AccountQuotas { accounts: all })
}

#[cfg(test)]
#[path = "account_quota_tests.rs"]
mod tests;
