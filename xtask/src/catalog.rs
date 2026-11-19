//! The guard over the translation catalogs, English beside pt-BR.
//!
//! English is where a key is born (AGENTS.md); pt-BR is its translation, not
//! a second catalog someone remembers to keep in step. A key English says
//! and pt-BR never heard of renders as English on a screen that promised
//! Portuguese; an empty value renders as nothing at all; a placeholder one
//! language fills and the other drops renders half a sentence. Each drift is
//! invisible in the diff that makes it — which is what this is for.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::Finding;

/// How much English may go untranslated in the open. A list that grows
/// without bound is a translation that quietly stopped happening.
const UNTRANSLATED_CEILING: usize = 20;

/// Where catalogs live. The web's are one pair per area
/// (`en/<area>.json` beside `pt-BR/<area>.json`); the core's is one flat
/// pair (`en.json` beside `pt-BR.json`). An absent folder is nothing to
/// compare.
const CATALOG_ROOTS: [&str; 2] = ["web/src/i18n", "crates/core/src/i18n"];

pub fn every_catalog_keeps_up_with_english(root: &Path) -> Vec<Finding> {
    let mut said = Vec::new();
    for base in CATALOG_ROOTS {
        let dir = root.join(base);
        if dir.is_dir() {
            said.extend(against_the_tree(&dir, base));
        }
    }
    said
}

/// One catalog folder: its untranslated list, and every area pair in it.
fn against_the_tree(dir: &Path, base: &str) -> Vec<Finding> {
    let allowed = std::fs::read_to_string(dir.join("untranslated.txt"))
        .map(|text| keys_of(&text))
        .unwrap_or_default();
    let mut said = Vec::new();
    if allowed.len() > UNTRANSLATED_CEILING {
        said.push(Finding {
            file: PathBuf::from(format!("{base}/untranslated.txt")),
            line: 1,
            what: format!(
                "lists {} keys as untranslated; the ceiling is {UNTRANSLATED_CEILING} — \
                 the backlog has outgrown the list",
                allowed.len()
            ),
        });
    }
    let (areas, unread) = areas_in(dir, base);
    said.extend(unread);
    for area in &areas {
        said.extend(disagreements(area, &allowed, base));
    }
    said
}

/// One area's pair of catalogs, `en/<area>.json` beside `pt-BR/<area>.json`.
/// The core's single pair has the empty name: `en.json` beside `pt-BR.json`.
pub struct Area {
    pub name: String,
    pub english: BTreeMap<String, serde_json::Value>,
    pub portuguese: BTreeMap<String, serde_json::Value>,
}

/// Where one side of an area lives, as it reads in a finding.
fn shown_path(base: &str, side: &str, name: &str) -> String {
    if name.is_empty() {
        format!("{base}/{side}.json")
    } else {
        format!("{base}/{side}/{name}.json")
    }
}

/// Every area either side names, read as a pair. A side with no file is an
/// empty map, so a missing pt-BR area reads as every key untranslated and a
/// missing English one as every key an orphan — which is what they are.
fn areas_in(dir: &Path, base: &str) -> (Vec<Area>, Vec<Finding>) {
    let mut names = BTreeSet::new();
    for side in ["en", "pt-BR"] {
        if dir.join(format!("{side}.json")).is_file() {
            names.insert(String::new());
        }
        let Ok(entries) = std::fs::read_dir(dir.join(side)) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Some(stem) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
            {
                names.insert(stem.to_owned());
            }
        }
    }
    let mut areas = Vec::new();
    let mut unread = Vec::new();
    for name in names {
        let (english, broken) = read_catalog(dir, base, "en", &name);
        unread.extend(broken);
        let (portuguese, broken) = read_catalog(dir, base, "pt-BR", &name);
        unread.extend(broken);
        areas.push(Area {
            name,
            english,
            portuguese,
        });
    }
    (areas, unread)
}

/// One catalog file as a map. Absent is an empty map (the pair's rules say
/// what that means); present but unreadable is refused — a catalog nobody
/// can read is a broken build, not an empty translation.
fn read_catalog(
    dir: &Path,
    base: &str,
    side: &str,
    name: &str,
) -> (BTreeMap<String, serde_json::Value>, Option<Finding>) {
    let shown = shown_path(base, side, name);
    let path = if name.is_empty() {
        dir.join(format!("{side}.json"))
    } else {
        dir.join(side).join(format!("{name}.json"))
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return (BTreeMap::new(), None);
    };
    let broken = |what: String| {
        Some(Finding {
            file: PathBuf::from(shown),
            line: 1,
            what,
        })
    };
    match serde_json::from_str(&text) {
        Ok(serde_json::Value::Object(map)) => (map.into_iter().collect::<BTreeMap<_, _>>(), None),
        Ok(_) => (
            BTreeMap::new(),
            broken("is not a flat catalog of keys".to_owned()),
        ),
        Err(why) => (BTreeMap::new(), broken(format!("does not parse: {why}"))),
    }
}

/// What one area's pair disagrees about.
fn disagreements(area: &Area, allowed: &[String], base: &str) -> Vec<Finding> {
    let english_file = shown_path(base, "en", &area.name);
    let portuguese_file = shown_path(base, "pt-BR", &area.name);
    let mut said = Vec::new();

    for (key, english) in &area.english {
        let Some(portuguese) = area.portuguese.get(key) else {
            if !allowed.contains(key) {
                said.push(Finding {
                    file: PathBuf::from(&portuguese_file),
                    line: 1,
                    what: format!(
                        "`{key}` is in {english_file} but not here — translate it, \
                         or say why in untranslated.txt"
                    ),
                });
            }
            continue;
        };
        for (value, file) in [(english, &english_file), (portuguese, &portuguese_file)] {
            said.extend(value_disagreements(key, value, file));
        }
        let carried = placeholders_of(english);
        let translated = placeholders_of(portuguese);
        if carried != translated {
            said.push(Finding {
                file: PathBuf::from(&portuguese_file),
                line: 1,
                what: format!(
                    "`{key}` fills {} in {english_file} but {} in {portuguese_file}",
                    said_as(&carried),
                    said_as(&translated)
                ),
            });
        }
    }

    for key in area.portuguese.keys() {
        if !area.english.contains_key(key) {
            said.push(Finding {
                file: PathBuf::from(&portuguese_file),
                line: 1,
                what: format!(
                    "`{key}` is only here — English is where every key is born (AGENTS.md)"
                ),
            });
        }
    }
    said
}

/// What one value says that it should not: nothing, or half a plural.
fn value_disagreements(key: &str, value: &serde_json::Value, file: &str) -> Vec<Finding> {
    let mut said = Vec::new();
    if sentences_of(value).iter().any(|s| s.trim().is_empty()) {
        said.push(Finding {
            file: file.into(),
            line: 1,
            what: format!("`{key}` says nothing — an empty value is a blank on the screen"),
        });
    }
    if let serde_json::Value::Object(forms) = value {
        for form in ["one", "other"] {
            if !forms.contains_key(form) {
                said.push(Finding {
                    file: file.into(),
                    line: 1,
                    what: format!(
                        "`{key}` is a plural without `.{form}` — .one and .other are \
                         the two forms the screen fills"
                    ),
                });
            }
        }
    }
    said
}

/// The sentences one value says: the value itself, or its plural forms.
fn sentences_of(value: &serde_json::Value) -> Vec<&str> {
    match value {
        serde_json::Value::String(sentence) => vec![sentence.as_str()],
        serde_json::Value::Object(forms) => {
            forms.iter().filter_map(|(_, form)| form.as_str()).collect()
        }
        _ => Vec::new(),
    }
}

/// Every {placeholder} one value carries; both sides of the pair must agree.
fn placeholders_of(value: &serde_json::Value) -> BTreeSet<String> {
    sentences_of(value)
        .iter()
        .flat_map(|sentence| placeholders_in(sentence).into_iter())
        .collect()
}

fn placeholders_in(sentence: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = sentence;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                found.insert(after[..close].to_owned());
                rest = &after[close + 1..];
            }
            /* An unclosed `{` is text, not a placeholder. */
            None => break,
        }
    }
    found
}

/// How a placeholder set reads in a finding: `{name} {count}`, or `none`.
fn said_as(placeholders: &BTreeSet<String>) -> String {
    if placeholders.is_empty() {
        return "no placeholders".to_owned();
    }
    placeholders
        .iter()
        .map(|name| format!("{{{name}}}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The untranslated list: one key per line, its reason after a `#`.
fn keys_of(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim().to_owned())
        .filter(|key| !key.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
