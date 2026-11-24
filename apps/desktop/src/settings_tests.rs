//! The language choice, through the commands that carry it.
//!
//! One test, in sequence: the commands share the store under the test home,
//! and two tests writing the same preference at once would each read the
//! other's answer.

use devpit_core::locale::Locale;
use devpit_rpc::Language;

/// Nothing but the language, so the answer's other fields are whatever the
/// store already held.
fn write(language: Option<Language>) -> devpit_rpc::Settings {
    crate::settings::settings_write_now(None, language, None, None, None, None, None, None)
        .expect("write")
}

#[test]
fn a_language_choice_answers_resolves_and_stays() {
    let answer = write(Some(Language::English));
    assert_eq!(answer.language, Language::English);
    assert_eq!(answer.locale, "en");
    assert_eq!(
        Locale::current(),
        Locale::En,
        "the process's answer moved with the write"
    );

    let answer = write(Some(Language::PortugueseBrazil));
    assert_eq!(answer.language, Language::PortugueseBrazil);
    assert_eq!(answer.locale, "pt-BR");
    assert_eq!(
        Locale::current(),
        Locale::PtBr,
        "the process's answer moved with the write"
    );

    // The choice is the store's to keep: a later read says the same, and a
    // write about something else does not un-choose it.
    let read = crate::settings::settings_read_now().expect("read");
    assert_eq!(read.language, Language::PortugueseBrazil);
    assert_eq!(read.locale, "pt-BR");

    let answer = write(None);
    assert_eq!(answer.language, Language::PortugueseBrazil);
    assert_eq!(answer.locale, "pt-BR");
}
