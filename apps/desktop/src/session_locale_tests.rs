//! The language of one session, held from its birth: what it says, where it
//! rides, and the two sessions that must not hear each other's.

use std::sync::{Arc, Barrier};

use devpit_core::locale::Locale;

use super::SessionLocale;

/// The assignment is the whole mechanism for a typed line: one variable, one
/// value, in front of the command, as a profile's own variables already are.
#[test]
fn a_session_carries_its_language_as_an_assignment_in_front() {
    assert_eq!(
        SessionLocale::of(Locale::PtBr).assignment(),
        "DEVPIT_LOCALE='pt-BR' "
    );
    assert_eq!(
        SessionLocale::of(Locale::En).assignment(),
        "DEVPIT_LOCALE='en' "
    );
}

/// The variable of the assignment is the variable, name for name: two
/// spellings would leave every session answering English.
#[test]
fn the_assignment_names_the_variable_the_server_reads() {
    let (name, value) = SessionLocale::of(Locale::PtBr).var();
    assert_eq!(name, "DEVPIT_LOCALE");
    assert_eq!(value, "pt-BR");
}

/// A session already running when the person switches the language in
/// Settings keeps the one it was born in: its MCP server keeps answering
/// `tools/list` as it began, and only the sessions born after switch.
#[test]
fn a_live_session_keeps_the_language_it_was_born_in() {
    Locale::PtBr.set();
    let born = SessionLocale::capture();
    let said = born.assignment();
    Locale::En.set();
    assert_eq!(born.assignment(), said);
    Locale::En.set();
}

/// Two sessions born one after another under different settings — the person
/// switched between the births — each carry their own: the barrier holds the
/// first's birth before the second's, as a click that starts one and then
/// the other would space them.
#[test]
fn two_sessions_born_apart_each_carry_their_own_language() {
    let apart = Arc::new(Barrier::new(2));
    let together = Arc::new(Barrier::new(2));
    let first = {
        let (apart, together) = (apart.clone(), together.clone());
        std::thread::spawn(move || {
            Locale::PtBr.set();
            let born = SessionLocale::capture();
            let said = born.assignment();
            apart.wait();
            together.wait();
            (born, said)
        })
    };
    let second = {
        let (apart, together) = (apart.clone(), together.clone());
        std::thread::spawn(move || {
            apart.wait();
            Locale::En.set();
            let born = SessionLocale::capture();
            let said = born.assignment();
            together.wait();
            (born, said)
        })
    };
    let ((first, first_said), (second, second_said)) =
        (first.join().expect("first"), second.join().expect("second"));
    assert_eq!(first.assignment(), first_said);
    assert_eq!(second.assignment(), second_said);
    assert_ne!(first_said, second_said);
    Locale::En.set();
}
