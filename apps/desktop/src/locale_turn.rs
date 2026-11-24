//! `Locale::current()` is one global, and the desktop's tests move it: the
//! settings write it, and the modules whose words are the person's read it.
//! The tests that touch it take turns here, and each settles the language it
//! expects while it holds the turn — the same pattern the core's own catalog
//! tests use.

use devpit_core::locale::Locale;

/// The turn itself: one test at a time moves or reads the process's language.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Settles the process on `locale` and holds the turn until the test ends.
pub(crate) fn settled(locale: Locale) -> std::sync::MutexGuard<'static, ()> {
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    locale.set();
    turn
}
