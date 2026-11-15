//! The preference keys, in one place.
//!
//! Apart from `settings.rs` for the same reason `migrations_list.rs` is
//! apart from `migrations.rs`: that file holds how a preference is read and
//! written, and this one holds what they are called. A key spelled
//! differently in two files is a setting that silently forgets itself, and
//! nothing about the symptom points at the typo.

pub const ONBOARDED_AT: &str = "onboarding.completed_at";
pub const THEME: &str = "appearance.theme";
pub const AUTO_UPDATE: &str = "general.automatic_updates";
/// Whether closing a terminal with something running stops to ask.
pub const CONFIRM_STOP: &str = "terminal.confirm_stop";
/// The minimum contrast the terminal lifts colours to, as a number from 1 to 21.
pub const TERMINAL_CONTRAST: &str = "terminal.minimum_contrast";
/// What holds new terminals: `tmux` for tmux, anything else devpit-ptyd.
pub const TERMINAL_BACKEND: &str = "terminal.backend";
/// Where new worktrees are created. Empty is the devpit workspace.
pub const WORKTREE_BASE: &str = "worktrees.base";
/// What a new card's branch is called. Empty is `{type}/{number}-{slug}`.
pub const WORKTREE_BRANCH: &str = "worktrees.branch_pattern";
/// Whether a finished card opens the next card of its epic in a session of
/// its own: only when `on`.
pub const SESSION_NEXT_CARD: &str = "sessions.next_card";
/// The share of the context window, in percent, past which a card's session
/// hands off to a fresh one: never unless set to a share from 1 to 100.
pub const SESSION_HANDOFF_PERCENT: &str = "sessions.handoff_at_percent";
/// The apps offered in "Open in", as JSON.
pub const OPEN_IN_APPS: &str = "apps.open_in";
/// Which worktree origins are hidden from the lists, comma-separated.
pub const WORKTREES_HIDDEN: &str = "worktrees.hidden";
/// Whether a card's worktree, build cache and branch go once its work is in
/// the default branch. On unless turned off.
pub const WORKTREES_DELETE_MERGED: &str = "worktrees.delete_merged";
/// What the last worktree cleanup did, as JSON.
pub const WORKTREES_CLEANUP_LAST: &str = "worktrees.cleanup_last";
/// Whether background probes slow down while idle or on battery. Off unless turned on.
pub const MAINTENANCE_POWER_SAVER: &str = "maintenance.power_saver";
/// Days logs and run artifacts are kept. Absent or 0 is until deleted.
pub const MAINTENANCE_KEEP_DAYS: &str = "maintenance.keep_days";
/// What the last cleaning of logs and run artifacts did, as JSON.
pub const MAINTENANCE_CLEAN_LAST: &str = "maintenance.clean_last";
/// Labs: whether the project agent answers. On unless turned off.
pub const LABS_PROJECT_AGENT: &str = "labs.project_agent";
/// Labs, before each project was turned on: the projects left out of the
/// project agent, as a JSON array of ids. Read once, to carry them over.
pub const LABS_PROJECT_AGENT_OFF: &str = "labs.project_agent_off";
/// Labs: the projects the person turned the project agent on for, as a JSON
/// array of ids. Absent until carried over from before; off unless listed.
pub const LABS_PROJECT_AGENT_ON: &str = "labs.project_agent_on";
/// Labs: the projects linked while off, each waiting for the person to answer
/// whether to turn on its learning, as a JSON array of ids.
pub const LABS_PROJECT_AGENT_ASKING: &str = "labs.project_agent_asking";
/// Labs: the projects the person answered Not now for, or turned off, as a
/// JSON array of ids. Linking one again asks nothing.
pub const LABS_PROJECT_AGENT_DECLINED: &str = "labs.project_agent_declined";
/// Labs: the projects whose facts resting only on files are accepted without
/// the person, as a JSON array of ids. Off unless a project is listed.
pub const LABS_ACCEPT_FILE_FACTS: &str = "labs.accept_file_facts";
/// Labs: whether the Canvas plugin is offered. On unless turned off.
pub const LABS_CANVAS: &str = "labs.canvas";
/// When each profile's idle conversations are compacted, by profile id, as
/// JSON. Absent is never.
pub const AGENTS_IDLE_COMPACT: &str = "agents.idle_compact";
/// The agent profiles the person declared, as JSON.
pub const AGENT_PROFILES: &str = "agents.profiles";
/// What sync keeps about each profile on this machine: values another
/// machine gave it that wait for the person's yes or stay apart from this
/// machine's own, profiles and variables removed here, and which arrived.
pub const AGENT_PROFILES_SYNCED: &str = "agents.profiles.synced";
/// What a new terminal opens, by agent or profile id. Empty is a shell.
pub const AGENT_DEFAULT: &str = "agents.default";
/// The ids kept out of the menus, as a JSON array.
pub const AGENTS_DISABLED: &str = "agents.disabled";
/// Whether devpit tells the agents it starts to report what they are doing.
pub const AGENT_HOOKS: &str = "agents.hooks";
/// Whether devpit tells Claude Code that the folders of the projects it adds,
/// and their worktrees, are trusted. On unless turned off.
pub const AGENT_TRUST: &str = "agents.trust_folders";
/// How wide the two side panels were left, in pixels.
pub const SIDEBAR_WIDTH: &str = "layout.sidebar_width";
pub const FILES_WIDTH: &str = "layout.files_width";
/// The focus that is on, as `<project id>:<seconds since the epoch>`, or
/// absent. Two fields in one row because a focus is one thing: half of it
/// written and half not is a focus with no beginning.
pub const HEADS_DOWN: &str = "focus.heads_down";
/// Whether the focus mode is offered. Off unless it is turned on.
pub const FOCUS_MODE: &str = "focus.enabled";
/// The keys that bring devpit forward from anywhere, as the shortcut
/// plugin spells them (`CommandOrControl+Shift+D`). Absent is none.
pub const SHORTCUT: &str = "general.shortcut";
/// The keys recorded in place of the window's shortcuts' defaults, as JSON.
/// Absent is every default.
pub const SHORTCUT_KEYS: &str = "keyboard.rebound";
/// The moment a pause ends, in seconds since the epoch; `0` is none.
pub const PAUSED_UNTIL: &str = "focus.paused_until";
/// Whether a card's date goes off as a reminder at its time. On unless
/// turned off.
pub const REMINDERS: &str = "general.reminders";
/// Whether closing the window leaves devpit in the tray. On unless turned off.
pub const KEEP_IN_TRAY: &str = "general.keep_in_tray";
/// Whether the person was told, once, that a closed window left devpit running.
pub const TRAY_TOLD: &str = "general.tray_told";
/// Whether the island opens above the other windows. On unless turned off.
pub const ISLAND: &str = "island.enabled";
/// The screen the island was last dragged to, by the name the system gives it.
pub const ISLAND_SCREEN: &str = "island.screen";
/// Whether the island opens where it is an ordinary window that can take
/// focus (Wayland with no layers: GNOME). Off unless turned on; this
/// machine's alone, like the display it is about.
pub const ISLAND_ANYWAY: &str = "island.anyway";
/// The display GTK opens on Linux: `wayland` or `x11`. Absent is automatic.
/// This machine's alone: it is not among the synced keys.
pub const DISPLAY_BACKEND: &str = "display.backend";
/// Whether devpit keeps its own errors for a report. Off unless turned on.
pub const ERROR_REPORTS: &str = "telemetry.error_reports";
/// How a voice message becomes words: `local` (a whisper on this machine),
/// `api` (a transcription service the person chose) or `off`. Local unless set.
pub const TRANSCRIBE: &str = "chat.transcribe";
/// The model: a whisper.cpp `.bin` file, a whisper model's name, or the
/// service's model. Empty is the engine's default.
pub const TRANSCRIBE_MODEL: &str = "chat.transcribe_model";
/// The language a voice message is heard in, as a code; empty is the system's.
pub const TRANSCRIBE_LANGUAGE: &str = "chat.transcribe_language";
/// The transcription service's address, OpenAI's form of it. Empty is OpenAI.
pub const TRANSCRIBE_URL: &str = "chat.transcribe_url";
/// Whether this machine can be reached from the person's other devices,
/// over their tailnet. Off unless turned on.
pub const REMOTE: &str = "remote.enabled";
/// The loopback port the remote viewer is served from, kept so the tailnet
/// address stays the same across restarts.
pub const REMOTE_PORT: &str = "remote.port";
/// How paired devices reach this machine: `tailscale` (its tailnet) or
/// `relay` (devpit.app's relay, encrypted end to end). Tailscale unless set.
pub const REMOTE_VIA: &str = "remote.via";
/// Whether a paired MCP client on another machine may read this devpit
/// through the relay. Off unless turned on.
pub const REMOTE_MCP: &str = "remote.mcp";
/// Who answers Decisions: `openrouter` or `vercel`. OpenRouter unless set.
pub const DECISIONS_PROVIDER: &str = "decisions.provider";
/// The Decisions model; empty is the provider's default.
pub const DECISIONS_MODEL: &str = "decisions.model";
/// The provider's address; empty is its default.
pub const DECISIONS_URL: &str = "decisions.url";
/// The most Decisions may spend in a day, in US dollars.
pub const DECISIONS_DAILY_CAP: &str = "decisions.daily_cap_usd";
/// The projects whose state is never sent to Decisions, as a JSON array.
pub const DECISIONS_OPTED_OUT: &str = "decisions.opted_out";
/// The gates that enforce, as a JSON object of gate to mode. Shadow unless named.
pub const DECISIONS_MODES: &str = "decisions.modes";
/// Whether Devflow is offered to any project at all. On unless turned off.
pub const DEVFLOW: &str = "devflow.enabled";
/// The setup a project without Devflow starts from: `recommended` or
/// `custom`. Recommended unless set.
pub const DEVFLOW_PRESET: &str = "devflow.preset";
/// The gate commands of team layers the person confirmed on this machine, by
/// fingerprint of the command, its layer and its commit, as a JSON array.
pub const DEVFLOW_COMMANDS: &str = "devflow.commands";
/// The projects whose Devflow the person switched on on this machine, as a
/// JSON array. A devflow.yaml saying `enabled: true` is not a click.
pub const DEVFLOW_ON: &str = "devflow.switched_on";
/// The projects whose Devflow Decisions the person moved to enforce, as a
/// JSON array.
pub const DEVFLOW_ENFORCED: &str = "devflow.enforced";
/// The profile each project's Devflow reviewers run as, as a JSON object of
/// project id to profile id. A project not in it follows the asking session.
pub const DEVFLOW_PROFILES: &str = "devflow.profiles";
/// Which events reach which channels, quiet hours and grouping, as JSON.
pub const CHANNEL_RULES: &str = "channels.rules";
/// Whether notices also go through devpit.app's hub (e-mail, push). Off unless turned on.
pub const CHANNEL_HUB: &str = "channels.hub";
/// Whether those notices name what they are about. Off unless turned on.
pub const CHANNEL_HUB_TITLES: &str = "channels.hub_titles";
/// The calendar's address on devpit.app, with its secret. Absent until turned on.
pub const CALENDAR_LINK: &str = "calendar.link";
/// Whether the calendar's events name the card and project. Off unless turned on.
pub const CALENDAR_TITLES: &str = "calendar.titles";
/// Whether this machine syncs with the account's others. Off unless turned on.
pub const SYNC: &str = "sync.enabled";
/// Each profile's models switched off and its advisor model, by profile id, as JSON.
pub const AGENTS_PROFILE_MODELS: &str = "agents.profile_models";
/// Minutes between checks of sign-ins and MCP servers. Absent is five.
pub const PROVIDERS_CHECK_MINUTES: &str = "providers.check_minutes";
/// The model a new chat starts with on every project. Absent is the CLI's own.
pub const CHAT_MODEL: &str = "chat.default_model";
/// The permission mode a new chat starts with on every project. Absent is the first.
pub const CHAT_PERMISSION: &str = "chat.default_permission";
/// What each project sets for itself over the machine and its repo, by project id, as JSON.
pub const PROJECT_SETTINGS: &str = "projects.settings";
