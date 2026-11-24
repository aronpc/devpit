//! The core catalog: one function that takes the language, and one that
//! reads it from the process.

use super::{t, t_for};
use crate::locale::Locale;

#[test]
fn a_key_is_said_in_the_language_asked() {
    assert_eq!(t_for(Locale::En, "tray.quit", &[]), "Quit devpit");
    assert_eq!(t_for(Locale::PtBr, "tray.quit", &[]), "Sair do devpit");
}

#[test]
fn placeholders_are_filled_and_plurals_follow_the_language() {
    assert_eq!(
        t_for(Locale::PtBr, "tray.greet", &[("name", "Ana")]),
        "Olá, Ana"
    );
    assert_eq!(
        t_for(Locale::En, "tray.sessions", &[("count", "1")]),
        "1 session"
    );
    assert_eq!(
        t_for(Locale::En, "tray.sessions", &[("count", "0")]),
        "0 sessions"
    );
    /* Portuguese counts zero as singular; English does not. */
    assert_eq!(
        t_for(Locale::PtBr, "tray.sessions", &[("count", "0")]),
        "0 sessão"
    );
    assert_eq!(
        t_for(Locale::PtBr, "tray.sessions", &[("count", "3")]),
        "3 sessões"
    );
}

#[test]
fn a_key_in_no_catalog_reads_as_its_last_word_never_as_a_dotted_key() {
    assert_eq!(t_for(Locale::PtBr, "tray.noSuchKey", &[]), "no such key");
}

#[test]
fn a_key_only_english_has_falls_back_to_english() {
    let english = super::parse(r#"{"only.english": "English alone"}"#);
    let portuguese = super::parse("{}");
    assert_eq!(
        super::say(Locale::PtBr, &portuguese, &english, "only.english", &[]),
        "English alone"
    );
}

/// `Locale::current()` is one global; the tests that move it take turns.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn t_is_t_for_the_locale_the_process_settled_on() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    Locale::PtBr.set();
    let said = t("tray.quit", &[]);
    Locale::En.set();
    assert_eq!(said, "Sair do devpit");
    assert_eq!(t("tray.quit", &[]), "Quit devpit");
}

#[test]
fn two_threads_each_get_their_own_language_whatever_the_process_is_in() {
    /* The process is Portuguese; a session in English must still read
    English, because t_for never asks the process. Locale::current() is
    global, so this is the only test that holds it Portuguese for long —
    and it restores English before asserting anything about t(). */
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    Locale::PtBr.set();
    let english = std::thread::spawn(|| {
        (0..2000)
            .map(|_| t_for(Locale::En, "tray.quit", &[]))
            .all(|said| said == "Quit devpit")
    });
    let portuguese = std::thread::spawn(|| {
        (0..2000)
            .map(|_| t_for(Locale::PtBr, "tray.quit", &[]))
            .all(|said| said == "Sair do devpit")
    });
    let (english, portuguese) = (english.join().unwrap(), portuguese.join().unwrap());
    Locale::En.set();
    assert!(english, "the English thread was handed another language");
    assert!(
        portuguese,
        "the Portuguese thread was handed another language"
    );
}

/// Every sentence a catalog holds, its plural forms included.
fn sentences(entry: &serde_json::Value) -> Vec<&str> {
    match entry {
        serde_json::Value::String(one) => vec![one.as_str()],
        serde_json::Value::Object(forms) => forms.values().filter_map(|f| f.as_str()).collect(),
        _ => Vec::new(),
    }
}

/// A bracketed prefix such as `[devpit, not the person]` is read back by
/// devpit (`devpit_agentcli::person`), not by a person: a translation that
/// changed it would make the person's own words stop counting. So every
/// language keeps an English sentence's leading `[...]` exactly as it is.
#[test]
fn a_prefix_devpit_reads_back_is_never_translated() {
    let english = super::parse(super::ENGLISH);
    let portuguese = super::parse(super::PORTUGUESE);
    let mut checked = 0;
    for (key, entry) in &english {
        let Some(prefix) = sentences(entry).into_iter().find_map(|one| {
            one.starts_with('[')
                .then(|| one.split_inclusive(']').next())
                .flatten()
        }) else {
            continue;
        };
        let Some(translated) = portuguese.get(key) else {
            continue;
        };
        for sentence in sentences(translated) {
            assert!(
                sentence.starts_with(prefix),
                "{key} lost its prefix {prefix:?} in pt-BR: {sentence:?}"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "no English sentence carries a prefix any more");
}
