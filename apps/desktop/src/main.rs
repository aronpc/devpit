// The window is the product on Windows and macOS; a console behind it leaks an
// implementation detail.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod account_quota;
mod adopted_history;
mod adopting;
mod advancing;
mod agent_aliases;
mod agent_api;
mod agent_door;
mod agent_reach;
mod agent_reminders;
mod arranging;
mod artifacts;
mod asking;
mod asking_kept;
mod asking_picks;
mod asking_plan;
mod asking_waiting;
mod assistant;
mod assistant_watch;
mod attaching;
mod attaching_chat;
mod blocks;
mod board;
mod board_page;
mod branches;
mod briefing;
mod browser;
mod browser_cookies;
mod browser_driving;
mod browser_menu;
// Where tauri draws with GTK, which is every unix that is not macOS. A child
// webview is placed by hand there; the module says why.
#[cfg(all(unix, not(target_os = "macos")))]
mod browser_gtk;
mod claims;
mod cloning;
mod columns;
mod commands;
mod delegations;
mod desk;
#[cfg(all(unix, not(target_os = "macos")))]
mod frame_gtk;
mod handing;
#[cfg(target_os = "linux")]
mod island_wayland;
mod live_answer;
mod live_prompt;
mod live_sessions;
mod mcp_apps;
#[cfg(all(unix, not(target_os = "macos")))]
mod microphone_gtk;
mod opening;
mod orchestrator;
mod orchestrator_brief;
mod orchestrator_handoff;
mod orchestrator_links;
mod orchestrator_notes;
mod orchestrator_view;
mod rewinding;
mod runs_list;
// Only ever compiled where it is used. The contract exists to generate the
// frontend's types — in a release build nothing calls it, and a module dead in
// release should say so rather than warn about it on every build.
mod agent_choice;
mod agent_decide;
mod agent_profiles;
mod calendar;
mod card_activity;
mod card_chat;
mod card_elsewhere;
mod card_follows;
mod card_numbers;
mod card_reconcile;
mod card_relay;
mod card_route;
mod card_sessions;
mod card_ties;
mod card_work;
mod cards;
mod chaining;
mod channel_actions;
mod channel_escalation;
mod channel_rules;
mod channel_talk;
mod channels;
mod chat;
mod chat_aside;
mod chat_listening;
mod chat_pace;
mod chat_queue;
mod chat_relay;
mod chat_remote;
mod chat_resident;
mod chat_running;
mod chat_turn;
mod chat_window;
mod checkout;
mod checkpoint;
mod checkpoint_findings;
mod checkpoint_preview;
mod checkpoint_tested;
mod checkup;
mod claude_accounts;
mod claude_plugin;
#[cfg(any(debug_assertions, test))]
mod contract;
mod credentials;
mod cycles;
mod deciding;
mod decisions;
mod devflow;
mod devflow_adhoc;
mod devflow_asks;
mod devflow_binary;
mod devflow_card;
mod devflow_codex;
mod devflow_dialect;
mod devflow_evidence;
mod devflow_gate;
mod devflow_host;
mod devflow_jev;
mod devflow_launch;
mod devflow_mcp;
mod devflow_plan;
mod devflow_plan_edit;
mod devflow_plugin;
mod devflow_profile;
mod devflow_publish;
mod devflow_review;
mod devflow_screen;
mod devflow_skills;
mod devflow_stage;
mod devflow_team;
mod devflow_work;
mod diagnostics;
mod diffs;
mod display_backend;
mod draft_sending;
mod ended_sessions;
mod error_reports;
mod error_sender;
#[cfg(test)]
mod fake_host;
mod files;
mod filetree;
mod folder_trust;
mod forwarded;
mod front;
mod guardrails;
mod handler;
mod handoff;
mod happening;
mod heads_down;
mod history;
mod host_check;
mod host_links;
mod host_panes;
mod host_sessions;
mod host_ssh;
mod hosts;
mod hub;
mod idle_compact;
mod in_flight;
mod index;
#[cfg(target_os = "linux")]
mod input_method;
mod installations;
mod island;
mod island_answer;
mod island_checks;
mod island_feed;
mod island_notify;
mod kept_out;
mod kinds;
mod leftovers;
mod listener;
mod live;
mod local_events;
mod local_files;
mod local_rpc;
mod local_server;
mod maintenance;
mod mcp;
mod mcp_cache;
mod mcp_health;
mod mcp_reading;
mod media;
mod memory_proposals;
mod moving;
mod notices;
mod off_main;
mod open_pane_arg;
mod openers;
mod outside_sessions;
mod pane_asking;
mod pane_io;
mod pane_said;
mod pane_screen;
mod pane_sessions;
mod pane_snapshot;
mod pane_socket;
mod pane_tmux;
mod panels;
mod panes;
mod pasting;
mod paths;
mod pausing;
mod plan_limits;
mod plugin_data;
mod plugins;
mod plus;
mod post;
mod prime;
mod prime_command;
mod prime_paths;
mod profile_models;
mod profile_waiting;
mod project_agent;
mod project_agent_repo;
mod project_lessons;
mod project_memory;
mod project_memory_view;
mod project_naming;
mod project_onboarding;
mod project_proposals;
mod project_settings;
mod projects;
mod provider_scan;
#[cfg(unix)]
mod ptyd_attach;
mod ptyd_link;
#[cfg(unix)]
mod ptyd_screens;
#[cfg(unix)]
mod ptyd_tap;
#[cfg(unix)]
mod ptyd_watch;
mod question;
mod reading_path;
mod rebound;
mod receipts;
mod recipes;
mod reconcile;
mod refusing;
mod regrouping;
mod reminder_time;
mod reminders;
mod remote;
mod remote_approval;
mod remote_conn;
mod remote_devices;
mod remote_host;
mod remote_http;
mod remote_hub;
mod remote_mcp;
mod remote_mcp_client;
mod remote_pairing;
mod remote_panes;
mod remote_relay;
mod remote_secret;
mod remote_start;
mod remote_tailscale;
mod remote_talk;
mod reply_drafts;
mod restoring;
mod reveal;
mod roots;
mod round_state;
mod rubric;
mod run_from;
mod runs;
mod saves;
mod schedules;
mod search;
mod secret_scan;
mod session_bus;
mod session_cost;
mod session_notice;
mod session_search;
mod session_told;
mod session_window;
mod sessions;
mod settings;
mod shell_launch;
mod shell_probe;
mod sign_ins;
mod skills;
mod slash;
mod sources;
mod spend_history;
mod staging;
mod stall_watch;
mod starting;
mod steering;
mod steps;
mod still_holds;
mod stop_asked;
mod stopping;
mod stopping_a_run;
mod sync;
mod sync_pairing;
mod tab_opening;
mod tap;
mod transcribe;
// Only Windows copies a pane into a file; tested everywhere.
#[cfg(any(windows, test))]
mod tap_file;
mod telegram;
mod telegram_devpit;
mod terminal;
mod terminal_run;
#[cfg(test)]
mod test_home;
mod threads;
mod tray;
mod tray_look;
mod tray_menu;
mod trust_answer;
mod turn_changes;
mod update;
mod update_deb;
mod vault;
mod vault_rotation;
mod watching;
mod whose_checkout;
mod windows_bin;
mod working;
mod workspace;
mod worktree_base;
mod worktree_branch;
mod worktree_cleanup;
mod worktree_setup;
mod worktree_sweep;
mod worktrees;
mod wsfiles;

fn main() {
    // Before any thread, and before anything asks for tmux.
    windows_bin::use_the_bundled_tmux();

    // `devpit agent …` and `devpit mcp` are an agent reaching the running
    // app, not a second window: answered here, before anything starts GTK.
    // Before the engine reads any project: devpit's folder, devpit's spawner.
    devflow_dialect::pinned();

    // The whole of the process's argv, argv[0] included, as the
    // single-instance handover also hears it.
    let argv: Vec<String> = std::env::args().collect();
    if let Some(code) = agent_door(&argv) {
        std::process::exit(code);
    }
    // `devpit` on a terminal: the menu, or a subcommand's plain lines.
    if let Some(code) = terminal_run::from_args(&argv) {
        std::process::exit(code);
    }

    // Before anything starts GTK, and before any thread exists.
    #[cfg(target_os = "linux")]
    input_method::use_the_desktops();
    #[cfg(target_os = "linux")]
    display_backend::choose();

    // Kept only once the switch is on; installed first so none is missed.
    devpit_core::reports::keep_panics();

    // Regenerated on every dev run so `make dev` keeps the frontend types in
    // step while screens are being written. The test does the same thing and
    // fails when the committed file is stale, which is what covers a build
    // nobody ran the window for.
    #[cfg(debug_assertions)]
    contract::contract()
        .export(specta_typescript::Typescript::default(), contract::BINDINGS)
        .expect("export the contract");

    // The runtime handler carries everything, contract or not.
    //
    // `session_attach` streams over `Channel<InvokeResponseBody>` so frames
    // reach the webview as raw bytes rather than JSON. specta cannot describe
    // that enum, so its wrapper is hand-written. Everything else about a
    // session — ensure, split, write, resize — is in the generated contract.
    let mut builder = tauri::Builder::default();
    // First of all, so a second launch hands over before it starts anything:
    // its own listener and door would write over this one's endpoint and
    // secret, and every session's hooks would post to a dead port.
    if desk::one_copy() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            #[cfg(target_os = "linux")]
            display_backend::handed_over();
            // A handover that names one of the panes is brought onto it; one
            // that names none, and one whose leaf the store would not answer
            // for, comes forward as every handover did. But while the
            // webview has yet to ask for a kept pane, no document exists to
            // hear the island's event: the ask is kept for the webview, and
            // only the window comes forward.
            if tauri::Manager::state::<open_pane_arg::PendingPane>(app).kept(&argv) {
                desk::forward(app);
                return;
            }
            match open_pane_arg::opening(&argv, |leaf| {
                sessions::store()
                    .map_err(|err| err.to_string())
                    .and_then(|store| sessions::project_holding(&store, leaf))
            }) {
                Ok(Some((project_id, pane_id))) => {
                    let _ = island::open_pane(app, Some(project_id), pane_id);
                }
                Ok(None) => desk::forward(app),
                Err(why) => {
                    devpit_core::reports::background("open pane", &why);
                    desk::forward(app);
                }
            }
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // The terminal's copy and paste. The webview's own clipboard is not
        // reachable from a terminal on WebKitGTK, and it cannot read a picture.
        .plugin(tauri_plugin_clipboard_manager::init())
        // Checking and downloading are the app's; installing is not — see
        // `update` for why only an AppImage is ever installed from here.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Telling the person an agent needs them while devpit is behind.
        .plugin(tauri_plugin_notification::init())
        .manage(chat::Talking::default())
        .manage(chat_relay::Relay::default())
        .manage(chat_resident::Residents::default())
        .manage(project_agent::Agents::default())
        .manage(chat_remote::Remotes::default())
        .manage(mcp_apps::McpApps::default())
        // A page that came with an MCP tool, on an origin of its own.
        .register_uri_scheme_protocol("mcpapp", |ctx, request| {
            mcp_apps::serve(ctx.app_handle(), request.uri().path())
        })
        // Video, audio and big pictures, in the ranges a player asks for; off
        // the main thread, which a file read must never hold.
        .register_asynchronous_uri_scheme_protocol(media::SCHEME, |_ctx, request, responder| {
            let path = request.uri().path().to_owned();
            let range = request
                .headers()
                .get("range")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            std::thread::spawn(move || responder.respond(media::serve(&path, range.as_deref())));
        })
        .manage(steering::Steering::default())
        .manage(asking::Asking::default())
        .manage(asking_waiting::Waiting::default())
        .manage(asking_kept::Kept::default())
        .manage(asking_picks::Picks::default())
        .manage(blocks::Blocks::default())
        .manage(update::Updating::default())
        /* Empty, and filled only by somebody granting a pane. */
        .manage(browser_driving::Granted::default())
        .manage(browser::Sessions::default())
        /* What the browser menu was last opened for, so its window can ask. */
        .manage(browser_menu::Opening::default())
        /* The pane this launch asked to open, taken by the webview once it
        is there to ask; see `open_pane_arg`. */
        .manage(open_pane_arg::PendingPane::from(&argv))
        .manage(error_sender::Presence::new(devpit_core::reports::now()))
        // Whether somebody is at the window, for reports that only go when
        // nobody is.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(focused) = event {
                if window.label() == "main" && *focused {
                    pane_asking::release_all();
                }
                if window.label() == "main" {
                    if let Some(presence) =
                        tauri::Manager::try_state::<error_sender::Presence>(window)
                    {
                        presence.focus(*focused, devpit_core::reports::now());
                    }
                }
            }
            // Into the tray, when there is one to come back from; otherwise
            // the close goes on and ends devpit, below.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if desk::ends_devpit(window.label()) && tray::hides_on_close() {
                    api.prevent_close();
                    tray::tuck_away(tauri::Manager::app_handle(window));
                }
            }
            // The hidden island would otherwise keep the process alive with
            // no window, on every platform alike.
            if matches!(event, tauri::WindowEvent::Destroyed) && desk::ends_devpit(window.label()) {
                tauri::Manager::app_handle(window).exit(0);
            }
        })
        // A reloaded page never detached its terminals: end them here.
        .on_page_load(|webview, payload| {
            #[cfg(target_os = "linux")]
            if webview.label() == "main"
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                display_backend::page_loaded(island_wayland::wayland());
            }
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
                if let Some(state) = tauri::Manager::try_state::<sessions::SessionState>(webview) {
                    for live in state.claims.take_all_from(webview.label()) {
                        let _ = live.stop();
                    }
                }
            }
        })
        .setup(|app| {
            // Managed here and not in the builder because it holds the handle
            // it relays through, and the handle does not exist until now.
            tauri::Manager::manage(app, sessions::SessionState::new(app.handle().clone()));
            // The person's Telegram bot, listened to from here when one is set up.
            telegram::listen(app.handle());
            // Buttons pressed on devpit.app's channels, done here.
            hub::listen();
            // Card dates on devpit.app's calendar, when it is on.
            calendar::keep_sent();
            // The same app in a browser, on loopback, behind a cookie.
            local_events::listen(app.handle());
            if let Ok(root) = devpit_core::Store::root() {
                if let Err(err) = local_server::start(app.handle().clone(), &root) {
                    eprintln!("devpit could not serve itself on loopback: {err}");
                }
            }
            // The account's other machines, sealed through devpit.app, when on.
            sync::keep_synced(app.handle());
            // The window rebuilt around a container that can hold a page
            // *beside* the app rather than under it. Done now, while the
            // window has exactly one webview in it, so the swap has nothing to
            // disturb; see `browser_gtk` for what GTK does otherwise.
            #[cfg(all(unix, not(target_os = "macos")))]
            if let Some(main) = tauri::Manager::get_webview_window(app, "main") {
                browser_gtk::settle(&main);
                frame_gtk::repaint_on_state_change(&main);
                microphone_gtk::allow(&main);
            }
            // Windows opens a window where it last put one, which for a new
            // install is low enough to run under the taskbar.
            #[cfg(windows)]
            if let Some(main) = tauri::Manager::get_webview_window(app, "main") {
                let _ = main.center();
            }
            // A development build says so where the window is listed —
            // the taskbar, alt-tab — because the window draws its own frame
            // and the title is otherwise seen nowhere. The top bar's badge is
            // the other half, for when both devpits are on screen.
            #[cfg(debug_assertions)]
            if let Some(main) = tauri::Manager::get_webview_window(app, "main") {
                let _ = main.set_title("devpit (dev)");
            }
            // The catalogue is compiled in and pinned by a test, so a manifest
            // breaking the contract is a build defect: loud where it is being
            // written; logged in release, where `plugin_data` refuses it on use
            // and panicking would only take the whole app away.
            if let Err(err) = devpit_rpc::validate_catalogue(&devpit_rpc::catalogue()) {
                if cfg!(debug_assertions) {
                    panic!("the plugin catalogue breaks the contract: {err}");
                }
                eprintln!("the plugin catalogue breaks the contract: {err}");
            }
            // The home made private before anything opens or writes in it. A
            // first start creates it owner-only; an install from before this
            // existed is world-readable until someone tightens it, and this
            // start is the only moment that knows about every file in there.
            // What it could not do is said, not fatal.
            if let Ok(root) = devpit_core::Store::root() {
                if let Err(err) = devpit_core::home::make_private_root(&root) {
                    eprintln!("{} could not be created private: {err}", root.display());
                }
                for (path, err) in devpit_core::home::harden(&root) {
                    eprintln!("{} was left as it was: {err}", path.display());
                }
                error_reports::resume(&root);
            }
            // Project folders named and moved before anything reads one. Here
            // and not in `Store::open`, so a test opening a store moves
            // nothing; a failure leaves each project where it was.
            if let Ok(root) = devpit_core::Store::root() {
                if let Err(err) = devpit_core::Store::open_default()
                    .and_then(|store| devpit_core::home::settle(&store, &root))
                {
                    eprintln!("project folders were not settled: {err}");
                }
            }
            // A card an older devpit made on this file gets its number.
            if let Err(err) =
                devpit_core::Store::open_default().and_then(|store| store.number_unnumbered())
            {
                eprintln!("cards were left without a number: {err}");
            }
            // A pause given before this start still holds, until its time.
            if let Ok(store) = devpit_core::Store::open_default() {
                pausing::restore(app.handle(), &store);
                tray::restore(&store);
                maintenance::restore(app.handle(), &store);
                project_onboarding::on_start(app.handle());
            }
            island::note_drawn();
            // In the tray, and the shortcut that brings it forward, if one was chosen.
            tray::start(app.handle());
            // Reachable from the person's other devices, when it was left on.
            remote::restore(app.handle());
            desk::shortcut_restore(app.handle());
            // A card's date goes off at its time, whatever project it is in.
            reminders::watch(app.handle().clone());
            assistant_watch::watch(app.handle().clone());
            // Messages between orchestrators and sessions of any CLI.
            session_bus::start(app.handle().clone());
            // devpit's half of each orchestrator's brief, as this build has it.
            orchestrator::seed_all_on_start();
            // Hooks are how the board hears about work as it happens rather
            // than a poll later. Started here so the endpoint is on disk before
            // the first turn goes out.
            if let Ok(root) = devpit_core::Store::root() {
                listener::start(app.handle().clone(), &root);
            }
            // Knocks on that listener's `/agent` and opens it again when it stops answering.
            agent_door::watch(app.handle().clone());
            // The island, when the person wants it and the screen can hold it.
            island::apply(app.handle());
            // Which agent CLIs this machine has, asked of the login shell —
            // which costs an interactive shell startup. Started now so the
            // first time somebody opens the menu the answer is already there.
            shell_launch::warm_installed();
            // What the cards were doing before this process started: asked
            // once, on its own thread, for the project the window opens on.
            card_reconcile::rebuild_on_start(app.handle().clone());
            pane_snapshot::watch(app.handle().clone());
            #[cfg(unix)]
            ptyd_tap::watch(app.handle().clone());
            // Looks for a newer devpit while the switch says to. Started
            // last: it is the one thing here that can wait.
            update::watch(app.handle().clone());
            // Sends the error reports the person switched on, when idle.
            error_sender::watch(app.handle().clone());
            // Says on stderr and in the error log when the window stops being answered.
            stall_watch::watch(app.handle().clone());
            // Merged cards' worktrees and build caches, put away on their own.
            worktree_cleanup::watch();
            // Old logs and run artifacts, put away when days are set.
            maintenance::watch();
            // A run still marked `running` after a restart is a run whose
            // thread died with the last process. Closed here, before anything
            // draws, or the card says it is working forever.
            reconcile::close_abandoned(app.handle());
            Ok(())
        })
        .manage(std::sync::Arc::new(in_flight::InFlight::new()))
        .invoke_handler(handler::handler())
        .build(tauri::generate_context!())
        .expect("the window did not open")
        .run(|app, event| {
            desk::reopened(app, &event);
            if let tauri::RunEvent::Exit = event {
                chat_resident::quit_all(app);
            }
        });
}

/// The exit code of an agent's command, when the arguments are one. `argv`
/// is the process's whole argv, argv[0] and all.
fn agent_door(argv: &[String]) -> Option<i32> {
    let args = argv.get(1..).unwrap_or_default();
    let first = args.first()?.as_str();
    // `devpit hook` is how an agent's hooks reach the app on Windows, where
    // the `curl` line the settings carry elsewhere has nothing to run on.
    let hook = cfg!(windows) && first == "hook";
    // The terminal daemon, run from the copy devpit installs of itself.
    if first == "ptyd" {
        return Some(devpit_ptyd::run(std::env::args_os().nth(2)));
    }
    // A ptyd pane on this terminal, which is how tmux shows one.
    if first == "ptyd-attach" {
        return Some(pane_tmux::attach_here(args.get(1).map(String::as_str)));
    }
    // The orchestrator's hook, on every message: its preferences as they are now.
    if first == orchestrator_view::SUBCOMMAND {
        if let (Some(folder), Ok(root)) = (args.get(1), devpit_core::Store::root()) {
            let under = devpit_core::home::orchestrators(&root);
            print!(
                "{}",
                orchestrator_view::shown_now(std::path::Path::new(folder), &under)
            );
        }
        return Some(0);
    }
    if first != "agent" && first != "mcp" && !hook {
        return None;
    }
    let root = match devpit_core::Store::root() {
        Ok(root) => root,
        // A hook that fails must not fail the turn it reports on.
        Err(_) if hook => return Some(0),
        Err(err) => {
            eprintln!("devpit: cannot find where devpit keeps its state: {err}");
            return Some(1);
        }
    };
    Some(match first {
        // A client on this machine for devpit's MCP on another, through the relay.
        "mcp" if args.get(1).is_some_and(|arg| arg == "--remote") => {
            remote_mcp_client::run(&root, &args[2..])
        }
        "mcp" => devpit_agentapi::mcp::serve(&root),
        "hook" => devpit_agentapi::hook::run(&root, &args[1..]),
        _ => devpit_agentapi::cli::run(&root, &args[1..]),
    })
}
