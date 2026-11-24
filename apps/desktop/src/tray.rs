//! devpit in the tray: an icon that says whether something waits on the
//! person, a menu that goes to it, and a window that closes into it.
//!
//! What waits is what devpit already knew: sessions stopped on the person,
//! drafts waiting for Send, reminders gone off. Each calls `refresh_soon`
//! where it changes; the redraw waits a moment so a burst of hooks draws once,
//! and draws nothing when the answer is the same. No timer asks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use devpit_core::locale::Locale;
use devpit_core::store::preference;
use devpit_core::Store;
use devpit_rpc::{Doing, RpcError, Tray};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::desk::{closing, forward, quitting, Close, Quit};
use crate::tray_menu::{said, Act, Entry, Go, Look, Recent, Said, Waiting};

const TRAY: &str = "devpit";
/// How long a change waits for the ones right behind it.
const SETTLE: Duration = Duration::from_millis(400);

static APP: OnceLock<AppHandle> = OnceLock::new();
/// Whether the icon was made. Not whether anybody sees it: see [`shown`].
static BUILT: AtomicBool = AtomicBool::new(false);
/// The preference, held here: the close is decided on the window's thread,
/// which must not wait on the store.
static KEEP_ON: AtomicBool = AtomicBool::new(true);
/// Set by Quit, so the close it asks for is not one more trip to the tray.
static QUITTING: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
/// What the tray shows now.
static LAST: Mutex<Option<Said>> = Mutex::new(None);

/// The mark each look is drawn from. Small: a tray draws it at 16 to 22
/// pixels, and Linux writes it to a file on every change.
fn mark() -> Image<'static> {
    tauri::include_image!("icons/128x128.png")
}

fn icon(look: Look) -> Image<'static> {
    let mark = mark();
    let (width, height) = (mark.width(), mark.height());
    Image::new_owned(
        crate::tray_look::drawn(mark.rgba(), width, height, look),
        width,
        height,
    )
}

/// Whether this desktop can show a tray icon. On Linux the icon goes through
/// the appindicator library, loaded when first used — and its loader panics
/// when the library is not there, which would take the whole app with it.
#[cfg(target_os = "linux")]
fn can_tray() -> bool {
    const NAMES: [&str; 2] = ["libayatana-appindicator3.so.1", "libappindicator3.so.1"];
    let mut dirs: Vec<std::path::PathBuf> = [
        "/usr/lib",
        "/usr/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/lib/x86_64-linux-gnu",
        "/lib/aarch64-linux-gnu",
    ]
    .iter()
    .map(std::path::PathBuf::from)
    .collect();
    // An AppImage carries its own.
    if let Some(appdir) = std::env::var_os("APPDIR") {
        let appdir = std::path::PathBuf::from(appdir);
        dirs.push(appdir.join("usr/lib"));
        dirs.push(appdir.join("usr/lib/x86_64-linux-gnu"));
        dirs.push(appdir.join("usr/lib/aarch64-linux-gnu"));
    }
    dirs.iter()
        .any(|dir| NAMES.iter().any(|name| dir.join(name).exists()))
}

#[cfg(not(target_os = "linux"))]
fn can_tray() -> bool {
    true
}

/// Whether the desktop draws the icon now. On Linux it is a
/// StatusNotifierItem, drawn only while a host holds the watcher's name:
/// GNOME without its AppIndicator extension holds none, and the icon made
/// goes nowhere.
#[cfg(all(unix, not(target_os = "macos")))]
fn hosted() -> bool {
    use gtk::gio;
    use gtk::glib::ToVariant as _;
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return false;
    };
    bus.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some(&("org.kde.StatusNotifierWatcher",).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        500,
        gio::Cancellable::NONE,
    )
    .ok()
    .and_then(|reply| reply.get::<(bool,)>())
    .is_some_and(|(owned,)| owned)
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn hosted() -> bool {
    true
}

/// Whether devpit's icon is somewhere the person can see it now.
pub(crate) fn shown() -> bool {
    BUILT.load(Ordering::Relaxed) && hosted()
}

/// The tray icon and its menu. A desktop without a tray just has none.
pub(crate) fn start(app: &AppHandle) {
    if !can_tray() {
        return;
    }
    // A last guard: the library found, and its loader panicking all the same.
    let handle = app.clone();
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || build(&handle)))
        .unwrap_or(false);
    BUILT.store(built, Ordering::Relaxed);
    if built {
        let _ = APP.set(app.clone());
        refresh_soon();
    }
}

fn build(app: &AppHandle) -> bool {
    // Plain at first; the refresh right after fills in what waits.
    let first = said(&[], &[], crate::pausing::paused());
    let Ok(menu) = menu(app, &first.entries) else {
        return false;
    };
    let built = TrayIconBuilder::with_id(TRAY)
        .tooltip(&first.tooltip)
        .icon(icon(first.look))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| chosen(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                forward(tray.app_handle());
            }
        })
        .build(app)
        .is_ok();
    if built {
        if let Ok(mut last) = LAST.lock() {
            *last = Some(first);
        }
    }
    built
}

/// Redraws the tray a moment from now, once for however many changes arrive
/// meanwhile.
pub(crate) fn refresh_soon() {
    let Some(app) = APP.get() else {
        return;
    };
    if PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SETTLE);
        PENDING.store(false, Ordering::Release);
        refresh(&app);
    });
}

/// The person changed the language: forgets what the tray last drew so the
/// next refresh redraws every word of it, not only what waits.
pub(crate) fn relanguage() {
    if let Ok(mut last) = LAST.lock() {
        *last = None;
    }
    refresh_soon();
}

fn refresh(app: &AppHandle) {
    let (waiting, recent) = gathered();
    let now = said(&waiting, &recent, crate::pausing::paused());
    let Ok(mut last) = LAST.lock() else {
        return;
    };
    let Some(tray) = app.tray_by_id(TRAY) else {
        return;
    };
    let was = last.take();
    let was = was.as_ref();
    if was.map(|was| was.look) != Some(now.look) {
        let _ = tray.set_icon(Some(icon(now.look)));
    }
    if was.map(|was| &was.title) != Some(&now.title) {
        let _ = tray.set_title(now.title.as_deref());
    }
    if was.map(|was| &was.tooltip) != Some(&now.tooltip) {
        let _ = tray.set_tooltip(Some(&now.tooltip));
    }
    if was.map(|was| &was.entries) != Some(&now.entries) {
        if let Ok(menu) = menu(app, &now.entries) {
            let _ = tray.set_menu(Some(menu));
        }
    }
    *last = Some(now);
}

/// What waits on the person, the newest session first, and the projects by
/// when they were last opened.
fn gathered() -> (Vec<Waiting>, Vec<Recent>) {
    let mut waiting: Vec<Waiting> = crate::island_feed::now()
        .into_iter()
        .filter(|one| one.state == Doing::Waiting)
        .map(|one| Waiting::Session {
            who: one
                .card
                .or(one.project)
                .unwrap_or_else(|| devpit_core::i18n::t("tray.aSession", &[])),
            session_id: one.session_id,
            project_id: one.project_id,
            pane_id: one.pane_id,
        })
        .collect();
    waiting.extend(
        crate::reply_drafts::all()
            .into_iter()
            .map(|(profile, session, _)| Waiting::Draft { profile, session }),
    );
    let Ok(store) = Store::open_default() else {
        return (waiting, Vec::new());
    };
    waiting.extend(
        store
            .reminders_pending()
            .unwrap_or_default()
            .into_iter()
            .map(|row| Waiting::Due {
                project_id: row.project_id,
                card_id: row.card_id,
                title: row.title,
            }),
    );
    let recent = store
        .projects()
        .unwrap_or_default()
        .into_iter()
        .map(|row| Recent {
            id: row.id,
            name: row.name,
        })
        .collect();
    (waiting, recent)
}

fn menu(app: &AppHandle, entries: &[Entry]) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    for entry in entries {
        match entry {
            Entry::Heading(text) => menu.append(&MenuItem::with_id(
                app,
                "heading",
                text,
                false,
                None::<&str>,
            )?)?,
            Entry::Go { label, go } => {
                menu.append(&MenuItem::with_id(app, go.id(), label, true, None::<&str>)?)?
            }
            Entry::Act { label, act } => menu.append(&MenuItem::with_id(
                app,
                act.id(),
                label,
                true,
                None::<&str>,
            )?)?,
            Entry::Projects(projects) => {
                let sub = Submenu::with_id(
                    app,
                    "projects",
                    devpit_core::i18n::t("tray.recentProjects", &[]),
                    true,
                )?;
                for (name, go) in projects {
                    sub.append(&MenuItem::with_id(app, go.id(), name, true, None::<&str>)?)?;
                }
                menu.append(&sub)?;
            }
            Entry::Line => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(menu)
}

fn chosen(app: &AppHandle, id: &str) {
    if let Some(go) = Go::from_id(id) {
        return go_to(app, go);
    }
    match Act::from_id(id) {
        Some(Act::Open) => forward(app),
        Some(Act::Terminal) => told(app, "tray:new-terminal", ()),
        Some(Act::Settings) => told(app, "tray:settings", ()),
        // A store write, never on the window's thread: a slow disk froze it.
        Some(Act::Pause) => off_the_window(app, crate::pausing::for_an_hour),
        Some(Act::Resume) => off_the_window(app, crate::pausing::resume),
        Some(Act::Browser) => crate::local_server::open_in_browser(),
        Some(Act::Quit) => quit(app),
        None => {}
    }
}

fn off_the_window(app: &AppHandle, act: fn(&AppHandle)) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || act(&app));
}

/// devpit forward, and its window told what to open.
fn told<P: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if crate::island::raised(app).is_ok() {
        // To `main` by name: a page in a browser pane must not hear it.
        let _ = app.emit_to(tauri::EventTarget::webview("main"), event, payload);
    }
}

/// Opens devpit where a choice in the menu said. One whose place is gone
/// still brings devpit forward, rather than doing nothing.
fn go_to(app: &AppHandle, go: Go) {
    match go {
        Go::Pane {
            project_id,
            pane_id,
        } => {
            let _ = crate::island::open_pane(app, project_id, pane_id);
        }
        Go::Card {
            project_id,
            card_id,
        } => told(app, "tray:open-card", (project_id, card_id)),
        Go::Project { project_id } => told(app, "tray:open-project", project_id),
        // Both read files to find where they go: off the thread menus run on.
        Go::Chat { session_id } => {
            let app = app.clone();
            std::thread::spawn(move || match crate::island::chat_of(&session_id) {
                Ok((project, conversation)) => {
                    let _ = crate::island::open_chat(&app, project, conversation);
                }
                Err(_) => forward(&app),
            });
        }
        Go::Drafts { profile } => {
            let app = app.clone();
            std::thread::spawn(move || {
                let orchestrator = crate::live_sessions::recent_projects()
                    .ok()
                    .and_then(|all| {
                        all.into_iter()
                            .find(|one| one.orchestrator.as_deref() == Some(profile.as_str()))
                    });
                match orchestrator {
                    Some(project) => {
                        let _ = crate::island::open_chat(&app, project.id, None);
                    }
                    None => forward(&app),
                }
            });
        }
    }
}

fn quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::Relaxed);
    let main = app.get_webview_window("main");
    match (quitting(main.is_some()), main) {
        (Quit::CloseTheWindow, Some(main)) => {
            // A window that will not close must not leave Quit doing nothing.
            if main.close().is_err() {
                app.exit(0);
            }
        }
        _ => app.exit(0),
    }
}

/// Whether closing the window puts it in the tray rather than ending devpit.
pub(crate) fn hides_on_close() -> bool {
    closing(
        KEEP_ON.load(Ordering::Relaxed),
        shown(),
        QUITTING.load(Ordering::Relaxed),
    ) == Close::Hide
}

/// The window into the tray. devpit goes on: hooks, notices, the agent door
/// and the Remote are all on this side of it.
pub(crate) fn tuck_away(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    // WebView2 is not told its window hid, and its page would go on drawing
    // and polling; WebKit is. Shown again in `island::raised`.
    #[cfg(windows)]
    let _ = AsRef::<tauri::Webview>::as_ref(&main).hide();
    let _ = main.hide();
    refresh_soon();
    let app = app.clone();
    std::thread::spawn(move || told_once(&app));
}

/// Said the first time only: a window that closes over a process that stays
/// is otherwise a devpit that seems to have quit and has not.
fn told_once(app: &AppHandle) {
    if crate::pausing::paused() {
        return;
    }
    let Ok(store) = Store::open_default() else {
        return;
    };
    if store.preference_flag(preference::TRAY_TOLD).ok().flatten() == Some(true) {
        return;
    }
    let (title, body) = told_words(Locale::current());
    crate::island_notify::show(app, &title, &body, false);
    let _ = store.set_preference_flag(preference::TRAY_TOLD, true);
}

/// The title and body of the one notice closing the window gives.
fn told_words(locale: Locale) -> (String, String) {
    let say = |key: &str| devpit_core::i18n::t_for(locale, key, &[]);
    (say("tray.toldTitle"), say("tray.toldBody"))
}

fn kept(store: &Store) -> bool {
    store
        .preference_flag(preference::KEEP_IN_TRAY)
        .ok()
        .flatten()
        .unwrap_or(true)
}

/// Reads, as the app starts, whether closing keeps devpit in the tray.
pub(crate) fn restore(store: &Store) {
    KEEP_ON.store(kept(store), Ordering::Relaxed);
}

fn now() -> Tray {
    Tray {
        shown: shown(),
        keep: KEEP_ON.load(Ordering::Relaxed),
    }
}

/// `tray.read` — whether devpit's icon is in the tray, and whether closing
/// the window leaves devpit there.
#[tauri::command]
#[specta::specta]
pub async fn tray_read() -> Result<Tray, RpcError> {
    crate::off_main::blocking(|| Ok(now())).await
}

/// `tray.keep_set` — closing the window leaves devpit in the tray, or quits.
#[tauri::command]
#[specta::specta]
pub async fn tray_keep_set(on: bool) -> Result<Tray, RpcError> {
    crate::off_main::blocking(move || {
        Store::open_default()?.set_preference_flag(preference::KEEP_IN_TRAY, on)?;
        KEEP_ON.store(on, Ordering::Relaxed);
        Ok(now())
    })
    .await
}

#[cfg(test)]
#[path = "tray_tests.rs"]
mod tests;
