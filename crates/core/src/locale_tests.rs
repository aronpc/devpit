//! The resolution rule, apart from the machine it reads.
//!
//! Every test says the three variables outright, because a test that read
//! the real environment would pass or fail with the machine it happened to
//! run on.

use crate::locale::{Locale, SystemEnv};

/// The system as the test wants it, not as the machine happens to be left.
fn system(lc_all: Option<&str>, lc_messages: Option<&str>, lang: Option<&str>) -> SystemEnv {
    SystemEnv {
        lc_all: lc_all.map(str::to_string),
        lc_messages: lc_messages.map(str::to_string),
        lang: lang.map(str::to_string),
    }
}

#[test]
fn a_choice_made_outright_wins_over_the_machine() {
    let portuguese_machine = system(Some("pt_BR.UTF-8"), None, None);
    assert_eq!(
        Locale::resolve(Some(Locale::En), &portuguese_machine),
        Locale::En
    );

    let english_machine = system(Some("en_US.UTF-8"), None, None);
    assert_eq!(
        Locale::resolve(Some(Locale::PtBr), &english_machine),
        Locale::PtBr
    );
}

#[test]
fn a_system_choice_reads_lc_all_first() {
    let machine = system(
        Some("pt_BR.UTF-8"),
        Some("en_US.UTF-8"),
        Some("en_US.UTF-8"),
    );
    assert_eq!(Locale::resolve(None, &machine), Locale::PtBr);
}

#[test]
fn an_empty_lc_all_does_not_decide_l_c_messages_does() {
    // Set but saying nothing is the same as unset: the next one decides.
    let machine = system(Some(""), Some("pt_BR.UTF-8"), Some("en_US.UTF-8"));
    assert_eq!(Locale::resolve(None, &machine), Locale::PtBr);
}

#[test]
fn lang_is_the_last_place_a_system_choice_looks() {
    let machine = system(None, None, Some("pt_BR.UTF-8"));
    assert_eq!(Locale::resolve(None, &machine), Locale::PtBr);
}

#[test]
fn a_machine_that_says_nothing_portuguese_is_english() {
    // English is not one locale among many here: it is the answer when the
    // machine said nothing, and the answer when it said something else.
    assert_eq!(
        Locale::resolve(None, &system(Some("en_US.UTF-8"), None, None)),
        Locale::En
    );
    assert_eq!(
        Locale::resolve(None, &system(Some("fr_FR.UTF-8"), None, None)),
        Locale::En
    );
    assert_eq!(Locale::resolve(None, &system(None, None, None)), Locale::En);
}
