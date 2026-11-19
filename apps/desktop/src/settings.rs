//! Contract commands for what the person chose.

use devpit_core::locale::Locale;
use devpit_core::{preference, Store};
use devpit_rpc::{ErrorCode, Language, RpcError, Settings, Theme};

/// What xterm accepts: 1 is no lift, 21 is black on white.
const CONTRAST: std::ops::RangeInclusive<f64> = 1.0..=21.0;

fn store() -> Result<Store, RpcError> {
    Ok(Store::open_default()?)
}

fn read(store: &Store) -> Result<Settings, RpcError> {
    let language = store
        .preference(preference::LANGUAGE)?
        .map_or(Language::System, |stored| Language::parse(&stored));
    let locale = Locale::resolve(language.locale(), &Locale::system());
    // Now, so that the tray, the island and the error paths read one answer
    // rather than each walking back to the store.
    locale.set();
    Ok(Settings {
        theme: store
            .preference(preference::THEME)?
            .map_or(Theme::Dark, |stored| Theme::parse(&stored)),
        language,
        locale: locale.tag().to_string(),
        onboarded_at: store
            .preference(preference::ONBOARDED_AT)?
            .and_then(|value| value.parse::<i64>().ok())
            .map(|seconds| seconds as f64),
        automatic_updates: store.preference_flag(preference::AUTO_UPDATE)?,
        confirm_stop: store.preference_flag(preference::CONFIRM_STOP)?,
        terminal_contrast: store
            .preference(preference::TERMINAL_CONTRAST)?
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| CONTRAST.contains(value)),
        focus_mode: store.preference_flag(preference::FOCUS_MODE)?,
        error_reports: store.preference_flag(preference::ERROR_REPORTS)?,
        island: store.preference_flag(preference::ISLAND)?,
        reminders: store.preference_flag(preference::REMINDERS)?,
    })
}

/// `settings.read` — everything the first run and the settings screen need.
#[tauri::command]
#[specta::specta]
pub async fn settings_read() -> Result<Settings, RpcError> {
    crate::off_main::blocking(settings_read_now).await
}

/// [`settings_read`], on the calling thread.
pub(crate) fn settings_read_now() -> Result<Settings, RpcError> {
    read(&store()?)
}

/// `settings.write` — records a choice and answers with the whole object.
///
/// Answering with the whole object rather than nothing means the screen never
/// has to predict what a write did to the rest of it.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn settings_write(
    theme: Option<Theme>,
    language: Option<Language>,
    automatic_updates: Option<bool>,
    confirm_stop: Option<bool>,
    terminal_contrast: Option<f64>,
    focus_mode: Option<bool>,
    error_reports: Option<bool>,
    island: Option<bool>,
) -> Result<Settings, RpcError> {
    crate::off_main::blocking(move || {
        settings_write_now(
            theme,
            language,
            automatic_updates,
            confirm_stop,
            terminal_contrast,
            focus_mode,
            error_reports,
            island,
        )
    })
    .await
}

/// [`settings_write`], on the calling thread.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settings_write_now(
    theme: Option<Theme>,
    language: Option<Language>,
    automatic_updates: Option<bool>,
    confirm_stop: Option<bool>,
    terminal_contrast: Option<f64>,
    focus_mode: Option<bool>,
    error_reports: Option<bool>,
    island: Option<bool>,
) -> Result<Settings, RpcError> {
    let store = store()?;
    if let Some(chosen) = theme {
        store.set_preference(preference::THEME, chosen.stored())?;
    }
    let relanguaged = language.is_some();
    if let Some(chosen) = language {
        store.set_preference(preference::LANGUAGE, chosen.stored())?;
    }
    if let Some(automatic) = automatic_updates {
        store.set_preference_flag(preference::AUTO_UPDATE, automatic)?;
    }
    if let Some(ask) = confirm_stop {
        store.set_preference_flag(preference::CONFIRM_STOP, ask)?;
    }
    if let Some(offered) = focus_mode {
        store.set_preference_flag(preference::FOCUS_MODE, offered)?;
    }
    if let Some(shown) = island {
        store.set_preference_flag(preference::ISLAND, shown)?;
    }
    if let Some(kept) = error_reports {
        // Now, not at the next start: off means nothing kept from this moment.
        crate::error_reports::choose(&store, &devpit_core::Store::root()?, kept)?;
    }
    if let Some(contrast) = terminal_contrast {
        if !CONTRAST.contains(&contrast) {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "terminal contrast goes from 1 to 21",
            ));
        }
        store.set_preference(preference::TERMINAL_CONTRAST, &contrast.to_string())?;
    }
    crate::sync::nudge();
    let settings = read(&store);
    if relanguaged {
        // `read` has settled the process on the new language; the tray's
        // menu and tooltip are words, so they are drawn again in it.
        crate::tray::relanguage();
    }
    settings
}

/// `settings.finish_onboarding` — the first run is done.
///
/// Recorded so the flow does not reappear for someone who finished it and then
/// removed their only project — those are different situations and the second
/// one is not a first run.
#[tauri::command]
#[specta::specta]
pub async fn settings_finish_onboarding() -> Result<Settings, RpcError> {
    crate::off_main::blocking(settings_finish_onboarding_now).await
}

/// [`settings_finish_onboarding`], on the calling thread.
pub(crate) fn settings_finish_onboarding_now() -> Result<Settings, RpcError> {
    let store = store()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    store.set_preference(preference::ONBOARDED_AT, &now.to_string())?;
    read(&store)
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
