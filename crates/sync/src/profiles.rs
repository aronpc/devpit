//! The person's agent profiles on every machine, without their secrets.
//!
//! A profile is a record, `profile:<id>`, by the id minted once when it was
//! made. Its label, agent, program (a bare name), models and the names of
//! its variables travel; its arguments stay where they were typed. A
//! variable's value travels only for a config folder (as `~/…`), a base URL
//! without credentials in it, a model name and the timeout, and never for a
//! name that reads as a secret. Every other value arrives empty and is asked
//! for on each machine; a value filled here is never emptied from elsewhere.
//!
//! What decides what runs, where a token goes or which account is spent —
//! the agent, the program (one word to a shell), a base URL, the config
//! folder — never changes here on another machine's word: it waits in the
//! profile's `shadow` for the person's yes, and a profile that arrived from
//! another machine does not start until they take it. A value this machine
//! keeps its own (a config folder outside home) stays, and the synced one is
//! kept beside it. A profile or a variable removed here is a tombstone, so
//! a round never brings it back. All of this is `agents.profiles.synced`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use devpit_core::{preference as key, Store, PROFILES_WRITE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::space::{Here, Namespace, Seen};

pub const LABEL: &str = "label";
pub const BASE: &str = "base";
pub const COMMAND: &str = "command";
pub const MODELS: &str = "models";
/// `env.<NAME>`: the value when it may travel, else empty; cleared when the
/// variable was removed.
pub const ENV: &str = "env.";
/// Set when the profile was removed: it is not made again anywhere.
pub const REMOVED: &str = "removed";

/// What sync keeps about this machine's profiles.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    #[serde(default)]
    pub profiles: BTreeMap<String, Held>,
    /// Profiles removed here or elsewhere, with the fields their record held,
    /// cleared so no round writes them again.
    #[serde(default)]
    pub removed: BTreeMap<String, BTreeSet<String>>,
}

/// What sync keeps about one profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// A field's synced value that this machine does not use: one waiting
    /// for the person's yes (`ask`), or one beside a value kept here.
    #[serde(default)]
    pub shadow: BTreeMap<String, Shadow>,
    /// Variables that arrived empty, to be filled here.
    #[serde(default)]
    pub needs: BTreeSet<String>,
    /// Made here from another machine and not yet taken by the person.
    #[serde(default)]
    pub arrived: bool,
    /// Made by the round still running: its fields fill it without asking,
    /// since there is nothing here yet to protect. The next look ends it.
    #[serde(default)]
    pub creating: bool,
    /// Its variables' names as last seen, and those removed since.
    #[serde(default)]
    pub env: BTreeSet<String>,
    #[serde(default)]
    pub cleared: BTreeSet<String>,
}

impl Held {
    /// Every field a record of this profile may hold, to clear on removal.
    fn fields(&self) -> BTreeSet<String> {
        let mut fields: BTreeSet<String> = [LABEL, BASE, COMMAND, MODELS]
            .iter()
            .map(|field| (*field).to_owned())
            .collect();
        for name in self.env.iter().chain(&self.cleared) {
            fields.insert(format!("{ENV}{name}"));
        }
        fields.extend(self.shadow.keys().cloned());
        fields
    }
}

/// The person kept this machine's own against a change from elsewhere: a
/// variable this machine never had stays out, the other machine's value
/// beside it so no round asks again; any other field is this machine's to
/// send back, as the person's answer.
pub fn keep_out(held: &mut Held, field: &str, here: bool) {
    match held.shadow.get_mut(field) {
        Some(shadow) if field.starts_with(ENV) && !here => shadow.ask = false,
        Some(_) => {
            held.shadow.remove(field);
        }
        None => {}
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shadow {
    pub value: String,
    #[serde(default)]
    pub ask: bool,
}

/// What sync keeps, read from the store; empty when it cannot be read.
pub fn kept(store: &Store) -> Kept {
    store
        .preference(key::AGENT_PROFILES_SYNCED)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn keep(store: &Store, kept: &Kept) -> Result<(), String> {
    let raw = serde_json::to_string(kept).map_err(|err| err.to_string())?;
    store
        .set_preference(key::AGENT_PROFILES_SYNCED, &raw)
        .map_err(|err| err.to_string())
}

/// The profiles namespace, over this machine's store and home folder.
pub struct Profiles<'a> {
    pub store: &'a Store,
    pub home: &'a Path,
}

/// A profile's record.
pub fn identity(id: &str) -> String {
    format!("profile:{id}")
}

/// A name that reads as a secret, whatever else it says.
fn secret_like(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    ["TOKEN", "KEY", "SECRET", "PASSWORD", "AUTH"]
        .iter()
        .any(|word| name.contains(word))
}

/// Whether a variable's value travels: what names an account's folder, a
/// gateway or a model, never what proves who the person is.
pub fn shared(name: &str) -> bool {
    !secret_like(name)
        && (matches!(name, "CLAUDE_CONFIG_DIR" | "API_TIMEOUT_MS")
            || name.ends_with("_BASE_URL")
            || name.contains("_MODEL"))
}

/// A field whose change decides what runs, where a token is sent, or which
/// account (config folder) a profile spends.
pub fn guarded(field: &str) -> bool {
    matches!(field, BASE | COMMAND)
        || field
            .strip_prefix(ENV)
            .is_some_and(|name| name.ends_with("_BASE_URL") || name == "CLAUDE_CONFIG_DIR")
}

fn plain(id: &str) -> bool {
    devpit_core::home::plain_id(id)
}

fn env_name(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && name
            .bytes()
            .enumerate()
            .all(|(at, b)| b == b'_' || b.is_ascii_alphabetic() || (at > 0 && b.is_ascii_digit()))
}

fn clean(text: &str, longest: usize) -> bool {
    text.chars().count() <= longest && !text.chars().any(char::is_control)
}

/// A URL with a user, a password or a query in it carries what may be a
/// secret, and does not travel.
fn credentialed(value: &str) -> bool {
    value.split_once("://").is_some_and(|(_, rest)| {
        let authority = rest.split('/').next().unwrap_or_default();
        authority.contains('@') || rest.contains('?')
    })
}

/// Whether a shared variable's value is safe to carry to another machine.
pub fn safe_to_share(value: &str) -> bool {
    clean(value, 4096) && !credentialed(value)
}

fn list_of(value: &str) -> Option<Vec<String>> {
    let list: Vec<String> = serde_json::from_str(value).ok()?;
    (list.len() <= 64 && list.iter().all(|one| clean(one, 256))).then_some(list)
}

/// A program named, not pathed, and one word to a shell: the editor's rule,
/// since it is typed into a terminal as it stands.
fn bare(command: &str) -> bool {
    command.is_empty()
        || (devpit_core::words::is_one_word(command)
            && command.len() <= 128
            && !command.contains(['/', '\\']))
}

/// Whether `value` may be written as `field`.
pub fn fits(field: &str, value: Option<&str>) -> bool {
    if let Some(name) = field.strip_prefix(ENV) {
        return env_name(name)
            && match value {
                None => true,
                Some(value) if shared(name) => safe_to_share(value),
                Some(value) => value.is_empty(),
            };
    }
    let Some(value) = value else {
        return false;
    };
    match field {
        LABEL => !value.trim().is_empty() && clean(value, 120),
        BASE => plain(value),
        COMMAND => bare(value),
        MODELS => list_of(value).is_some(),
        REMOVED => value == "1",
        _ => false,
    }
}

fn text(profile: &Value, name: &str) -> String {
    profile[name].as_str().unwrap_or_default().to_owned()
}

fn vars(profile: &Value) -> Vec<(String, String)> {
    profile["env"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|var| Some((var["name"].as_str()?.to_owned(), text(var, "value"))))
                .collect()
        })
        .unwrap_or_default()
}

fn set_var(profile: &mut Value, name: &str, value: Option<&str>) {
    let mut kept: Vec<Value> = profile["env"].as_array().cloned().unwrap_or_default();
    let at = kept.iter().position(|var| var["name"] == name);
    match (at, value) {
        (Some(at), Some(value)) => kept[at]["value"] = json!(value),
        (None, Some(value)) => kept.push(json!({"name": name, "value": value})),
        (Some(at), None) => {
            kept.remove(at);
        }
        (None, None) => {}
    }
    profile["env"] = Value::Array(kept);
}

impl Profiles<'_> {
    fn home(&self) -> String {
        self.home.to_string_lossy().trim_end_matches('/').to_owned()
    }

    /// A value as another machine may read it: under the home folder as `~`;
    /// `None` for a path elsewhere on this machine or a URL with credentials.
    fn portable(&self, value: &str) -> Option<String> {
        if credentialed(value) {
            return None;
        }
        let home = self.home();
        if value == home {
            return Some("~".to_owned());
        }
        if let Some(rest) = value.strip_prefix(&format!("{home}/")) {
            return Some(format!("~/{rest}"));
        }
        (!value.starts_with('/')).then(|| value.to_owned())
    }

    /// A value from another machine, under this one's home.
    pub fn local(&self, value: &str) -> String {
        match value.strip_prefix("~/") {
            Some(rest) => format!("{}/{rest}", self.home()),
            None if value == "~" => self.home(),
            None => value.to_owned(),
        }
    }

    /// The stored list, or `None` when it cannot be read: writing over a
    /// list nobody can parse would drop every profile on it.
    fn list(&self) -> Result<Option<Vec<Value>>, String> {
        let raw = self
            .store
            .preference(key::AGENT_PROFILES)
            .map_err(|err| err.to_string())?
            .unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(Some(Vec::new()));
        }
        Ok(serde_json::from_str::<Vec<Value>>(&raw).ok())
    }

    fn keep_list(&self, list: &[Value]) -> Result<(), String> {
        self.store
            .set_preference(key::AGENT_PROFILES, &Value::from(list.to_vec()).to_string())
            .map_err(|err| err.to_string())
    }

    fn fields(&self, profile: &Value, held: &mut Held) -> BTreeMap<String, Seen> {
        let mut fields = BTreeMap::new();
        let mut put = |field: String, value: Option<String>| {
            if fits(&field, value.as_deref()) {
                fields.insert(field, Seen::of(value, None));
            }
        };
        let shadowed = |field: &str| held.shadow.get(field).map(|one| one.value.clone());
        put(LABEL.to_owned(), Some(text(profile, LABEL)));
        put(
            BASE.to_owned(),
            Some(shadowed(BASE).unwrap_or_else(|| text(profile, BASE))),
        );
        let command = text(profile, COMMAND);
        let command = shadowed(COMMAND).unwrap_or(if bare(&command) {
            command
        } else {
            String::new()
        });
        put(COMMAND.to_owned(), Some(command));
        let models = profile[MODELS].as_array().cloned().unwrap_or_default();
        put(MODELS.to_owned(), Some(Value::from(models).to_string()));

        let env = vars(profile);
        let now: BTreeSet<String> = env.iter().map(|(name, _)| name.clone()).collect();
        for gone in held.env.difference(&now) {
            held.cleared.insert(gone.clone());
        }
        held.cleared.retain(|name| !now.contains(name));
        held.env = now;
        // A value kept here that can travel again is this machine's to send.
        let portable: BTreeSet<String> = env
            .iter()
            .filter(|(_, value)| self.portable(value).is_some())
            .map(|(name, _)| format!("{ENV}{name}"))
            .collect();
        let bare_here = bare(&text(profile, COMMAND));
        held.shadow.retain(|field, one| {
            one.ask || !(portable.contains(field) || (field == COMMAND && bare_here))
        });

        for (name, value) in &env {
            let field = format!("{ENV}{name}");
            let value = match (shared(name), held.shadow.get(&field)) {
                (false, _) => Some(String::new()),
                (true, Some(one)) => Some(one.value.clone()),
                (true, None) => self.portable(value),
            };
            if value.is_some() {
                put(field, value);
            }
        }
        // A value that waits for a variable this machine does not have yet.
        let have: BTreeSet<String> = env.iter().map(|(name, _)| format!("{ENV}{name}")).collect();
        for (field, one) in &held.shadow {
            if field.starts_with(ENV) && !have.contains(field) {
                put(field.clone(), Some(one.value.clone()));
            }
        }
        for name in &held.cleared {
            put(format!("{ENV}{name}"), None);
        }
        fields
    }
}

impl Namespace for Profiles<'_> {
    fn name(&self) -> &'static str {
        "profiles"
    }

    fn here(&self) -> Result<Here, String> {
        let _held = PROFILES_WRITE.lock().map_err(|err| err.to_string())?;
        let Some(list) = self.list()? else {
            return Ok(Here::new());
        };
        let mut kept = kept(self.store);
        let before = kept.clone();
        // A round that made a profile is over, whether or not it can be
        // carried yet: anything after it asks.
        for held in kept.profiles.values_mut() {
            held.creating = false;
        }
        let present: BTreeSet<String> = list.iter().map(|one| text(one, "id")).collect();
        // Known here and gone: the person removed it.
        let gone: Vec<String> = kept
            .profiles
            .keys()
            .filter(|id| !present.contains(*id))
            .cloned()
            .collect();
        for id in gone {
            let held = kept.profiles.remove(&id).unwrap_or_default();
            kept.removed.insert(id, held.fields());
        }
        let mut here = Here::new();
        for profile in &list {
            let id = text(profile, "id");
            // Not a profile anyone could start: nothing to carry.
            if !plain(&id)
                || !fits(LABEL, Some(&text(profile, LABEL)))
                || !fits(BASE, Some(&text(profile, BASE)))
                || kept.removed.contains_key(&id)
            {
                continue;
            }
            let held = kept.profiles.entry(id.clone()).or_default();
            here.insert(identity(&id), self.fields(profile, held));
        }
        for (id, held) in &kept.removed {
            let mut fields: BTreeMap<String, Seen> = held
                .iter()
                .map(|field| (field.clone(), Seen::of(None, None)))
                .collect();
            fields.insert(REMOVED.to_owned(), Seen::of(Some("1".to_owned()), None));
            here.insert(identity(id), fields);
        }
        if kept != before {
            keep(self.store, &kept)?;
        }
        Ok(here)
    }

    fn write(&self, record: &str, field: &str, value: Option<&str>) -> Result<(), String> {
        let _held = PROFILES_WRITE.lock().map_err(|err| err.to_string())?;
        let Some(id) = record.strip_prefix("profile:").filter(|id| plain(id)) else {
            return Ok(());
        };
        if !fits(field, value) {
            return Ok(());
        }
        let Some(mut list) = self.list()? else {
            return Ok(());
        };
        let mut kept = kept(self.store);
        if field == REMOVED {
            let held = kept.profiles.get(id).cloned().unwrap_or_default();
            let mut fields = held.fields();
            if let Some(profile) = list.iter().find(|one| one["id"] == id) {
                fields.extend(
                    vars(profile)
                        .into_iter()
                        .map(|(name, _)| format!("{ENV}{name}")),
                );
            }
            kept.removed.insert(id.to_owned(), fields);
            // A step here still runs under it: it stays on this machine, out
            // of sync, rather than leave the step naming nothing.
            let used = self
                .store
                .steps_using_profile(id)
                .map_err(|err| err.to_string())?;
            // What it waits for here stays with it.
            if used.is_empty() {
                kept.profiles.remove(id);
                list.retain(|one| one["id"] != id);
                self.keep_list(&list)?;
            }
            return keep(self.store, &kept);
        }
        if kept.removed.contains_key(id) {
            return Ok(());
        }
        let at = match list.iter().position(|one| one["id"] == id) {
            Some(at) => at,
            None => {
                list.push(json!({
                    "id": id, "label": "", "base": "", "command": "",
                    "args": [], "env": [], "models": []
                }));
                let held = kept.profiles.entry(id.to_owned()).or_default();
                held.arrived = true;
                held.creating = true;
                list.len() - 1
            }
        };
        let held = kept.profiles.entry(id.to_owned()).or_default();
        let profile = &mut list[at];
        match (field, value) {
            (LABEL, Some(value)) => profile[LABEL] = json!(value),
            (MODELS, Some(value)) => profile[MODELS] = json!(list_of(value).unwrap_or_default()),
            (BASE | COMMAND, Some(value)) => {
                let now = text(profile, field);
                if now == value {
                    held.shadow.remove(field);
                } else if held.creating && now.is_empty() {
                    profile[field] = json!(value);
                } else {
                    // A program named by its path here stays; any other
                    // change to what runs waits for the person.
                    let ask = field != COMMAND || bare(&now);
                    held.shadow.insert(
                        field.to_owned(),
                        Shadow {
                            value: value.to_owned(),
                            ask,
                        },
                    );
                }
            }
            (field, value) => {
                let Some(name) = field.strip_prefix(ENV) else {
                    return Ok(());
                };
                let now = vars(profile)
                    .into_iter()
                    .find(|(one, _)| one == name)
                    .map(|(_, value)| value);
                match value {
                    None => {
                        set_var(profile, name, None);
                        held.env.remove(name);
                        held.needs.remove(name);
                        held.shadow.remove(field);
                        held.cleared.insert(name.to_owned());
                    }
                    Some(_) if !shared(name) => {
                        held.cleared.remove(name);
                        // A secret: named so it can be asked for, never
                        // emptied once this machine has it.
                        if now.is_none() {
                            set_var(profile, name, Some(""));
                            held.env.insert(name.to_owned());
                            held.needs.insert(name.to_owned());
                        }
                    }
                    Some(value) => {
                        held.cleared.remove(name);
                        let local = self.local(value);
                        let mine = now.as_deref().filter(|now| !now.is_empty());
                        if mine.is_some_and(|now| self.portable(now).is_none()) {
                            held.shadow.insert(
                                field.to_owned(),
                                Shadow {
                                    value: value.to_owned(),
                                    ask: false,
                                },
                            );
                        } else if guarded(field)
                            && now.as_deref() != Some(local.as_str())
                            // Made by this round: there is nothing to protect yet.
                            && !(held.creating && now.as_deref().is_none_or(str::is_empty))
                        {
                            held.shadow.insert(
                                field.to_owned(),
                                Shadow {
                                    value: value.to_owned(),
                                    ask: true,
                                },
                            );
                        } else {
                            set_var(profile, name, Some(&local));
                            held.env.insert(name.to_owned());
                            held.shadow.remove(field);
                        }
                    }
                }
            }
        }
        self.keep_list(&list)?;
        keep(self.store, &kept)
    }

    fn welcomes(&self, record: &str) -> bool {
        record.strip_prefix("profile:").is_some_and(plain)
    }
}

#[cfg(test)]
#[path = "profiles_tests.rs"]
mod tests;
