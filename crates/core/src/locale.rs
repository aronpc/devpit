//! The one rule for which language devpit's words are in.
//!
//! The choice the person made and the machine's own answer meet here, once,
//! so that the tray, the island and the error paths all read the same
//! [`Locale::current`] rather than each walking back to the settings store.

use std::sync::atomic::{AtomicU8, Ordering};

/// A language devpit can be in. English is the base and the default;
/// Brazilian Portuguese is the first translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    PtBr,
}

/// What the machine says about language, as the person left it: the three
/// variables POSIX looks at, in the order it looks at them.
#[derive(Debug)]
pub struct SystemEnv {
    pub lc_all: Option<String>,
    pub lc_messages: Option<String>,
    pub lang: Option<String>,
}

/// The locale this process settled on, set whenever the settings are read or
/// written. The parts of devpit that are not in a position to ask the store
/// ask this instead.
static CURRENT: AtomicU8 = AtomicU8::new(0);

impl Locale {
    /// The one resolution rule: a choice made outright wins, and only a
    /// `System` choice — or none made — falls through to the machine.
    pub fn resolve(choice: Option<Self>, system: &SystemEnv) -> Self {
        choice.unwrap_or_else(|| Self::from_system(system))
    }

    /// LC_ALL, then LC_MESSAGES, then LANG: the first with something to say
    /// decides, and Portuguese is what starts with `pt`. Anything else, or
    /// silence, is English.
    fn from_system(system: &SystemEnv) -> Self {
        let says = [&system.lc_all, &system.lc_messages, &system.lang]
            .into_iter()
            .flatten()
            .find(|value| !value.is_empty());
        match says {
            Some(value) if value.starts_with("pt") => Self::PtBr,
            _ => Self::En,
        }
    }

    /// The machine's own answer, as the three variables leave it.
    pub fn system() -> SystemEnv {
        SystemEnv {
            lc_all: std::env::var("LC_ALL").ok(),
            lc_messages: std::env::var("LC_MESSAGES").ok(),
            lang: std::env::var("LANG").ok(),
        }
    }

    /// What the interface and the formatters take: a BCP 47 tag for
    /// Portuguese, the bare word for English.
    pub fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }

    /// The locale this process settled on. English until the settings say
    /// otherwise.
    pub fn current() -> Self {
        match CURRENT.load(Ordering::Relaxed) {
            1 => Self::PtBr,
            _ => Self::En,
        }
    }

    /// What the settings reader and writer call: the process's answer, from
    /// here to the next change.
    pub fn set(self) {
        CURRENT.store(self as u8, Ordering::Relaxed);
    }
}
