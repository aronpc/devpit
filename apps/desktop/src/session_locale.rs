//! The language of one session, held from the moment the session is born.
//!
//! The settings can change under a running session — the person switches the
//! language while an agent works — and the session must not notice: its
//! words, and its MCP server's, were chosen where it started. [`SessionLocale::capture`]
//! is that choice, made once, and everything a session is started with takes
//! it from there rather than asking the settings again.

use std::collections::HashMap;

use devpit_core::locale::Locale;

/// The variable a session's own process carries its language in. Set on the
/// line that starts the session — typed into a terminal or given to a spawned
/// CLI — and inherited by the stdio MCP server that process starts, which
/// reads it as its own answer's language.
pub(crate) const ENV: &str = devpit_agentcli::running::LOCALE_ENV;

/// One session's language, read once when the session is born and never
/// asked for again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionLocale(Locale);

impl SessionLocale {
    /// What a session is born in: the process's locale as it stands at the
    /// birth, held for the session's life. Everything the session is started
    /// with takes it from here, so a later change reaches the sessions born
    /// after it, and never this one.
    pub(crate) fn capture() -> Self {
        Self(Locale::current())
    }

    /// A locale held outright: a session already born in it, as the tests
    /// start theirs.
    #[cfg(test)]
    pub(crate) const fn of(locale: Locale) -> Self {
        Self(locale)
    }

    /// The held locale, for the catalog's `t_for`: a session's words are said
    /// in it, never in the process's own.
    pub(crate) fn locale(self) -> Locale {
        self.0
    }

    /// The locale's BCP 47 tag, as the variable carries it.
    pub(crate) fn tag(self) -> &'static str {
        self.0.tag()
    }

    /// This session's language as the variable its process is started with.
    pub(crate) fn var(self) -> (String, String) {
        (ENV.to_owned(), self.tag().to_owned())
    }

    /// The `DEVPIT_LOCALE='pt-BR' ` a typed line starts with — an assignment
    /// like a profile's own, and the only way a line of text can put a value
    /// in a process's environment.
    pub(crate) fn assignment(self) -> String {
        format!("{ENV}='{}' ", self.tag())
    }
}

/// The language of conversation `id` at this turn. `newborn` is its first
/// turn, and only then is the settings' language read; every later turn, a
/// resident's restart and a compaction reuse it. Held in memory: a
/// conversation first heard after the app started is born again there.
pub(crate) fn of_conversation(id: &str, newborn: bool) -> SessionLocale {
    static BORN: std::sync::OnceLock<std::sync::Mutex<HashMap<String, SessionLocale>>> =
        std::sync::OnceLock::new();
    let Ok(mut born) = BORN.get_or_init(Default::default).lock() else {
        return SessionLocale::capture();
    };
    if newborn {
        born.remove(id);
    }
    *born
        .entry(id.to_owned())
        .or_insert_with(SessionLocale::capture)
}

#[cfg(test)]
#[path = "session_locale_tests.rs"]
mod tests;
