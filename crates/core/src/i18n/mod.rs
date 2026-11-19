//! The words the core itself says: the tray, and errors that belong to no
//! session.
//!
//! The catalog lives here, in the core, because the RPC server and the
//! desktop shell both depend on it — one catalog, so an error and the tray
//! never disagree about a word.
//!
//! Two functions, and the difference matters:
//!
//! - [`t_for`] takes the language. Anything said *to a session* — a brief,
//!   a note, a project agent's text — uses it with that session's
//!   `SessionLocale`, because a session born in English must stay in
//!   English while the person's settings say Portuguese.
//! - [`t`] reads the process's [`Locale::current`]. It is for the tray and
//!   the errors the core raises for the person looking at the screen, and
//!   nowhere else.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::locale::Locale;

#[cfg(test)]
mod tests;

type Catalog = HashMap<String, Value>;

const ENGLISH: &str = include_str!("en.json");
const PORTUGUESE: &str = include_str!("pt-BR.json");

/// What `key` says in `locale`, with `{name}` filled from `args`. A plural
/// takes its form from `count` (an arg of that name) in the language the
/// sentence is in. A key the language lacks is English; a key no catalog has
/// reads as its last word, never as a dotted key and never as nothing.
pub fn t_for(locale: Locale, key: &str, args: &[(&str, &str)]) -> String {
    static ENGLISH_CATALOG: OnceLock<Catalog> = OnceLock::new();
    static PORTUGUESE_CATALOG: OnceLock<Catalog> = OnceLock::new();
    let english = ENGLISH_CATALOG.get_or_init(|| parse(ENGLISH));
    let own = match locale {
        Locale::En => english,
        Locale::PtBr => PORTUGUESE_CATALOG.get_or_init(|| parse(PORTUGUESE)),
    };
    say(locale, own, english, key, args)
}

/// [`t_for`] in the process's own language: for the tray and global errors.
pub fn t(key: &str, args: &[(&str, &str)]) -> String {
    t_for(Locale::current(), key, args)
}

fn parse(text: &str) -> Catalog {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => map.into_iter().collect(),
        /* The catalogs are compiled in and the guard parses them: this is
        unreachable in a build that passed `cargo xtask check`. */
        _ => Catalog::new(),
    }
}

fn say(
    locale: Locale,
    own: &Catalog,
    english: &Catalog,
    key: &str,
    args: &[(&str, &str)],
) -> String {
    let (entry, from) = match own.get(key) {
        Some(entry) => (entry, locale),
        None => match english.get(key) {
            Some(entry) => (entry, Locale::En),
            None => return fill(&missing(key), args),
        },
    };
    let sentence = match entry {
        Value::String(sentence) => sentence.as_str(),
        Value::Object(forms) => {
            let form = count_of(args).map_or("other", |count| plural_form(from, count));
            forms
                .get(form)
                .or_else(|| forms.get("other"))
                .and_then(Value::as_str)
                .unwrap_or_default()
        }
        _ => "",
    };
    fill(sentence, args)
}

fn count_of(args: &[(&str, &str)]) -> Option<u64> {
    args.iter()
        .find(|(name, _)| *name == "count")
        .and_then(|(_, value)| value.parse().ok())
}

/// CLDR's cardinal rule for the two languages devpit has: English's `one` is
/// exactly 1; Portuguese's is 0 and 1.
fn plural_form(locale: Locale, count: u64) -> &'static str {
    let one = match locale {
        Locale::En => count == 1,
        Locale::PtBr => count <= 1,
    };
    if one {
        "one"
    } else {
        "other"
    }
}

/// `{name}` becomes the arg of that name; one the sentence lacks is simply
/// not there, and one the args lack is nothing.
fn fill(sentence: &str, args: &[(&str, &str)]) -> String {
    let mut said = String::with_capacity(sentence.len());
    let mut rest = sentence;
    while let Some(open) = rest.find('{') {
        said.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                if let Some((_, value)) = args.iter().find(|(arg, _)| *arg == name) {
                    said.push_str(value);
                }
                rest = &after[close + 1..];
            }
            None => {
                said.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    said.push_str(rest);
    said
}

/// `emptyLane` reads as `empty lane`: the last segment, spaced.
fn missing(key: &str) -> String {
    let last = key.rsplit('.').find(|part| !part.is_empty()).unwrap_or(key);
    let mut said = String::new();
    let mut before: Option<char> = None;
    for c in last.chars() {
        if c.is_uppercase() && before.is_some_and(|b| b.is_lowercase() || b.is_ascii_digit()) {
            said.push(' ');
            said.extend(c.to_lowercase());
        } else {
            said.push(c);
        }
        before = Some(c);
    }
    said
}
