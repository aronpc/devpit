//! The migrations themselves, which are mostly SQL.
//!
//! Apart from the runner because they are data and it is code: the list only
//! ever grows, and a file that grows without bound should not be the one
//! holding the logic that applies it.

use super::{Before, Migration};

/// The Rust run before a version's SQL, by version.
pub(super) const BEFORE: &[(i64, Before)] = &[
    (13, crate::store::plugins::export_drawings),
    (32, crate::store::origins::rehash_origins),
];

/// Migration 001 — the core tables.
pub(super) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: r#"
CREATE TABLE trust_workspace (
    id          TEXT PRIMARY KEY,
    slug        TEXT NOT NULL UNIQUE,
    label       TEXT NOT NULL,
    -- Required, not decorative: the persistent visual mark in the bar. Without
    -- it, client context eventually lands in the personal remote.
    color       TEXT NOT NULL,
    memory_remote    TEXT,
    vault_namespace  TEXT NOT NULL,
    is_default  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    revision    INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE project (
    id          TEXT PRIMARY KEY,
    trust_workspace_id TEXT NOT NULL REFERENCES trust_workspace(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    -- Local. Never syncs: an absolute path means nothing on another machine.
    root_path   TEXT NOT NULL,
    origin_url  TEXT,
    -- Normalized, hashed git origin. Routing key across machines: two clones
    -- of the same repo converge on the same remote project.
    origin_hash TEXT,
    group_name  TEXT,
    last_opened_at INTEGER,
    archived_at INTEGER,
    created_at  INTEGER NOT NULL,
    revision    INTEGER NOT NULL DEFAULT 1
);

CREATE UNIQUE INDEX project_root_path ON project(root_path);
CREATE INDEX project_origin_hash ON project(origin_hash) WHERE origin_hash IS NOT NULL;

CREATE TABLE scratch (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES project(id) ON DELETE CASCADE,
    body        TEXT NOT NULL,
    -- Lets the system age a note: "open 40 days, the file it cites changed 6
    -- times, still valid?". The antidote to orphan notes is the system asking,
    -- not discipline.
    anchor_path   TEXT,
    anchor_commit TEXT,
    archived_at INTEGER,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    revision    INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX scratch_project ON scratch(project_id, archived_at);

CREATE TABLE drawing (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    -- Opaque on purpose: the core never reads it. Storing the editor's format
    -- as structured data would sign us up to maintain it every release.
    scene       TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    revision    INTEGER NOT NULL DEFAULT 1
);

CREATE UNIQUE INDEX drawing_name ON drawing(project_id, name);

-- Append-only. Nothing here is updated, which is what makes it auditable.
CREATE TABLE event (
    id          TEXT PRIMARY KEY,
    topic       TEXT NOT NULL,
    scope_id    TEXT,
    -- neutral | attention | failure | verified. A field of the event, not a UI
    -- decision: colouring normal activity turns the screen into an alarm panel.
    severity    TEXT NOT NULL,
    payload     TEXT NOT NULL,
    acknowledged_at INTEGER,
    created_at  INTEGER NOT NULL
);

CREATE INDEX event_feed ON event(created_at DESC);
"#,
    },
    // Migration 002 — the answers the first run asks for.
    Migration {
        version: 2,
        sql: r#"
-- Key/value and not a column per setting: every new preference would otherwise
-- be a migration, and a migration over real data is the part that goes wrong.
-- Values are text; the reader parses what it asked for.
CREATE TABLE preference (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  INTEGER NOT NULL,
    revision    INTEGER NOT NULL DEFAULT 1
);
"#,
    },
    Migration {
        version: 3,
        sql: r#"
CREATE TABLE pane_layout (
    project_id  TEXT PRIMARY KEY REFERENCES project(id) ON DELETE CASCADE,
    tree        TEXT NOT NULL,
    focused_id  TEXT NOT NULL,
    updated_at  INTEGER NOT NULL
);
"#,
    },
    Migration {
        version: 4,
        sql: r#"
-- A column is data, not code. It is created, renamed, reordered and deleted
-- by the person using it, and nothing in the code may assume the names the
-- default board happens to seed.
CREATE TABLE board_column (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    position    INTEGER NOT NULL,
    -- NULL means the column runs nothing, and that is a column doing its job:
    -- a holding lane is not half-configured.
    step_id     TEXT REFERENCES step(id) ON DELETE SET NULL,
    created_at  INTEGER NOT NULL
);
CREATE UNIQUE INDEX board_column_position ON board_column(project_id, position);

-- A step belongs to the project, not to one column: "run the tests" serves
-- more than one lane, and editing the command in one place holds everywhere.
--
-- `config` is JSON, shaped by `kind`:
--   agent   -> { agent, schema, budgetUsd, model }
--   session -> { agent?, worktree, model }
--   command -> { command, timeoutSeconds }
CREATE TABLE step (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('agent','session','command')),
    name        TEXT NOT NULL,
    config      TEXT NOT NULL,
    -- A deploy has no undo. An irreversible step is confirmed before it runs,
    -- never fired by a drag alone.
    irreversible INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);

-- RESTRICT, not CASCADE: deleting a column that still holds cards has to fail
-- so the interface can ask where they go. CASCADE would delete described work
-- on one click.
CREATE TABLE card (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    column_id   TEXT NOT NULL REFERENCES board_column(id) ON DELETE RESTRICT,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    position    INTEGER NOT NULL,
    -- The worktree of this line of work, and the ref it started from. The
    -- diff is against `base_ref`, never against HEAD.
    worktree_path TEXT,
    base_ref    TEXT,
    archived_at INTEGER,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX card_column ON card(column_id, position);

CREATE TABLE run (
    id          TEXT PRIMARY KEY,
    card_id     TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    step_id     TEXT NOT NULL REFERENCES step(id) ON DELETE RESTRICT,
    state       TEXT NOT NULL
                CHECK (state IN ('running','ok','failed','cancelled')),
    -- Validated JSON for an agent step, streamed output for a command, or the
    -- reason it failed.
    output      TEXT,
    exit_code   INTEGER,
    cost_usd    REAL,
    duration_ms INTEGER,
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER
);
CREATE INDEX run_card ON run(card_id, started_at DESC);

-- Only the link. Whether a session is idle or busy, and what it cost, are
-- answered by the agent CLI and its transcript; a copy here would be a second
-- truth that drifts from the first.
CREATE TABLE session_link (
    card_id     TEXT PRIMARY KEY REFERENCES card(id) ON DELETE CASCADE,
    short_id    TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    transcript_path TEXT,
    created_at  INTEGER NOT NULL
);
"#,
    },
    Migration {
        version: 5,
        sql: r#"
-- A layout belongs to a tab, not to a project.
--
-- It was keyed by project alone, which meant one tree for the whole project —
-- and opening a second terminal tab had to split that tree to get a leaf. So
-- three tabs were three leaves under two split nodes, drawn as three tabs: a
-- list wearing a tree's clothes, whose shape meant nothing.
--
-- Keyed by tab, a split is what its name says: dividing what one tab shows.
--
-- Rebuilt rather than altered because the primary key changes, which SQLite
-- cannot do in place. Existing rows keep their tree under a tab named after
-- the project, so a session open across the upgrade is not lost.
CREATE TABLE pane_layout_v5 (
    project_id  TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    tab_id      TEXT NOT NULL,
    tree        TEXT NOT NULL,
    focused_id  TEXT NOT NULL,
    updated_at  INTEGER NOT NULL,
    PRIMARY KEY (project_id, tab_id)
);

INSERT INTO pane_layout_v5 (project_id, tab_id, tree, focused_id, updated_at)
SELECT project_id, 'tab_' || project_id, tree, focused_id, updated_at FROM pane_layout;

DROP TABLE pane_layout;
ALTER TABLE pane_layout_v5 RENAME TO pane_layout;
"#,
    },
    // Migration 006 — what a card is besides a title.
    //
    // One migration for three things because a migration over real data is
    // the part that goes wrong, and doing it once is better than three times.
    Migration {
        version: 6,
        sql: r#"
-- Seconds since the epoch, like every other time here. Nullable, because
-- most cards never have one and a default would be a date nobody chose.
ALTER TABLE card ADD COLUMN due_at INTEGER;

CREATE TABLE card_comment (
    id          TEXT PRIMARY KEY,
    card_id     TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    -- Who said it. `you` or an agent id; never a display name, which
    -- changes, and never free text, which a screen would have to trust.
    author      TEXT NOT NULL,
    body        TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    edited_at   INTEGER
);
CREATE INDEX card_comment_card ON card_comment(card_id, created_at);

-- The path, not the bytes. The file is already on this person's disk and
-- copying it into the database duplicates what git already versions — and
-- makes the store grow without bound for nothing.
CREATE TABLE card_attachment (
    id          TEXT PRIMARY KEY,
    card_id     TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    -- Absolute. Local only, like project.root_path, and for the same reason.
    path        TEXT NOT NULL,
    label       TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);
CREATE INDEX card_attachment_card ON card_attachment(card_id, created_at);

-- What the bell has to show. Written by whatever noticed; read by one pane.
CREATE TABLE notice (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES project(id) ON DELETE CASCADE,
    -- `run`, `agent`, `due`, `irreversible` — what kind of thing happened.
    kind        TEXT NOT NULL,
    title       TEXT NOT NULL,
    detail      TEXT,
    -- What it is about, so a click can go there. Null for a notice with
    -- nowhere to go.
    card_id     TEXT REFERENCES card(id) ON DELETE CASCADE,
    created_at  INTEGER NOT NULL,
    read_at     INTEGER
);
CREATE INDEX notice_unread ON notice(read_at, created_at DESC);
"#,
    },
    // Migration 007 — a run remembers where its card stood.
    Migration {
        version: 7,
        sql: r#"
-- Where the card was when this run started.
--
-- It was read before the move and passed by value into the thread that runs
-- the step, which is enough while the process lives and nothing at all when
-- it does not: a run in flight when the app quits took the only record of
-- where its card came from with it. A verdict that sends the card back then
-- has nowhere to send it.
--
-- Nullable, because every run already on disk was started before this column
-- existed and inventing a column for them would be inventing a fact.
ALTER TABLE run ADD COLUMN from_column TEXT REFERENCES board_column(id) ON DELETE SET NULL;

-- Runs that are still `running` after a restart are runs whose process is
-- gone. Indexed because the sweep at launch asks exactly this.
CREATE INDEX run_unfinished ON run(state) WHERE ended_at IS NULL;
"#,
    },
    // Migration 008 — a lane says where a pass goes, and how much it decides
    // on its own.
    Migration {
        version: 8,
        sql: r#"
-- Where a card goes when this lane's step approves it.
--
-- Declared rather than derived. "The next column by position" was the obvious
-- rule and it is a trap: a column is data — renamed, reordered by dragging —
-- so a card would start advancing somewhere else with nobody having changed
-- anything about it.
--
-- The asymmetry with sending a card *back* is deliberate and it is honest:
-- backward needs no declaration because the card carries where it came from,
-- and forward has no such record.
--
-- NULL is the default and means the step runs and the card stays — which is
-- what every board does today.
ALTER TABLE board_column ADD COLUMN on_pass TEXT REFERENCES board_column(id) ON DELETE SET NULL;

-- How much this lane decides without being asked.
--
--   manual  nothing happens on its own. The default, and the only value any
--           existing board has.
--   ask     the step runs, the verdict is read, and the card waits for a
--           person to accept the advance.
--   auto    the card advances on the verdict.
--
-- On the column and not the board, because the lane that runs tests and the
-- lane that deploys never want the same answer. And not on the card, because
-- a card is a task, not a configuration: the moment one declares how it runs,
-- two identical tasks in identical lanes behave differently for a reason that
-- is written nowhere visible.
ALTER TABLE board_column ADD COLUMN autonomy TEXT NOT NULL DEFAULT 'manual'
    CHECK (autonomy IN ('manual','ask','auto'));
"#,
    },
    Migration {
        version: 9,
        sql: r#"
-- Full-text search over what was said in the CLI's own transcripts.
--
-- Only the text column is indexed; the rest ride along to filter and to open
-- the hit. unicode61 so an accented word matches its plain spelling.
CREATE VIRTUAL TABLE session_text USING fts5(
    path UNINDEXED,
    session_id UNINDEXED,
    project UNINDEXED,
    installation UNINDEXED,
    role UNINDEXED,
    text,
    tokenize = 'unicode61'
);

-- What each transcript looked like when it was last read. Size and mtime
-- together: a rewrite to the same size still moves the mtime.
CREATE TABLE session_file (
    path TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL
);
"#,
    },
    // Migration 010 — a run whose process vanished is lost, not failed.
    Migration {
        version: 10,
        sql: r#"
-- The launch sweep closed runs whose process was gone as `failed`, which tells
-- a person the step failed when nobody knows how it ended.
--
-- SQLite cannot change a CHECK in place, so the table is rebuilt. Nothing
-- references `run`, so dropping it cascades nowhere.
CREATE TABLE run_new (
    id          TEXT PRIMARY KEY,
    card_id     TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    step_id     TEXT NOT NULL REFERENCES step(id) ON DELETE RESTRICT,
    state       TEXT NOT NULL
                CHECK (state IN ('running','ok','failed','cancelled','lost')),
    output      TEXT,
    exit_code   INTEGER,
    cost_usd    REAL,
    duration_ms INTEGER,
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER,
    from_column TEXT REFERENCES board_column(id) ON DELETE SET NULL
);
INSERT INTO run_new (id, card_id, step_id, state, output, exit_code, cost_usd,
                     duration_ms, started_at, ended_at, from_column)
    SELECT id, card_id, step_id, state, output, exit_code, cost_usd,
           duration_ms, started_at, ended_at, from_column FROM run;
DROP TABLE run;
ALTER TABLE run_new RENAME TO run;
CREATE INDEX run_card ON run(card_id, started_at DESC);
CREATE INDEX run_unfinished ON run(state) WHERE ended_at IS NULL;
-- The project's runs view pages newest first.
CREATE INDEX run_started ON run(started_at DESC, id DESC);

-- The sweep's own words mark the rows it wrote. A run that had printed
-- something before it vanished kept that output instead, so it cannot be told
-- apart and stays as it was.
UPDATE run SET state = 'lost'
    WHERE state = 'failed' AND output = 'the app closed while this was running';
"#,
    },
    // Migration 011 — which agent a pane was running, to start it again.
    Migration {
        version: 11,
        sql: r#"
-- The agent devpit started in a pane, and the CLI session it is in.
--
-- tmux keeps a pane's processes while the app is closed, but not across a
-- reboot or a server that died. The layout still names the pane, so the pane
-- comes back; without this it comes back as an empty shell where a
-- conversation was.
CREATE TABLE pane_agent (
    leaf_id         TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    launch          TEXT NOT NULL,
    session_id      TEXT,
    transcript_path TEXT,
    updated_at      INTEGER NOT NULL
);
"#,
    },
    // Migration 012 — a project's folder gets a name a person can find.
    Migration {
        version: 12,
        sql: r#"
-- `projects/<slug>-<suffix>`, named once from the project's name and kept
-- through renames. Syncs: another machine uses the same folder.
--
-- Nullable, and unique through an index rather than the column: SQLite cannot
-- add a UNIQUE column. The rows that exist are named in Rust at launch
-- (`home::settle`), because the slug is not something SQL can spell.
ALTER TABLE project ADD COLUMN folder TEXT;
CREATE UNIQUE INDEX project_folder ON project(folder);
"#,
    },
    // Migration 013 — plugins, turned on per project.
    Migration {
        version: 13,
        sql: r#"
-- Syncs, so the same project shows the same plugins on another machine. The id
-- is checked against the catalogue by the app; a row for a plugin this build
-- does not ship is ignored, not deleted.
--
-- An uninstall keeps the row with both flags off, so the removal syncs too.
CREATE TABLE project_plugin (
    project_id  TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    plugin_id   TEXT NOT NULL,
    installed   INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    revision    INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (project_id, plugin_id)
);

-- Which plugin opens a pinned file. NULL for a file pinned from the checkout.
ALTER TABLE card_attachment ADD COLUMN plugin_id TEXT;

-- Drawings are the Excalidraw plugin's files now; the rows were written out
-- just before this ran (`BEFORE`).
DROP TABLE drawing;
"#,
    },
    // Migration 014 — what links a card to the sessions working on it.
    Migration {
        version: 14,
        sql: r#"
-- The session a run's agent spoke in, and the folder it ran in: a run resumed
-- in a chat has to resume where it ran, because the CLI files sessions by folder.
ALTER TABLE run ADD COLUMN session_id TEXT;
ALTER TABLE run ADD COLUMN cwd TEXT;
-- The same folder for the background session a step started.
ALTER TABLE session_link ADD COLUMN cwd TEXT;

-- The conversations that are a card's. One card to a conversation, and they go
-- with the card. Whether one is working is never stored: the app keeps that.
CREATE TABLE card_chat (
    conversation_id TEXT PRIMARY KEY,
    card_id         TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL
);
CREATE INDEX card_chat_card ON card_chat(card_id, created_at);
"#,
    },
    // Migration 015 — which profile a background session runs under.
    Migration {
        version: 15,
        sql: r#"
-- The profile a step used to start this session, so attaching it later names
-- the same binary and the same account. NULL for links made before this, which
-- fall back to the default runner.
ALTER TABLE session_link ADD COLUMN profile_id TEXT;
"#,
    },
    // Migration 016 — a table nothing ever wrote to.
    Migration {
        version: 16,
        sql: r#"
-- `event` was a feed nothing wrote to and nothing drew. Dropped.
--
-- `scratch` stays, although nothing reads it any more: the window wrote notes
-- to it until the notes commands went, so a store from before then may hold
-- somebody's notes, and a migration does not get to delete what a person
-- wrote. It goes in a later release, after its rows have somewhere to go.
--
-- Destructive for `event` only, and without a downgrade, which costs nothing
-- today: no release has been published, so there is no older devpit to go
-- back to.
DROP TABLE IF EXISTS event;
"#,
    },
    // Migration 017 — what a run actually ran, and the evidence it left.
    Migration {
        version: 17,
        sql: r#"
-- A run used to keep its output and nothing about the circumstances, so a
-- green row could not answer "green on which code, with which command". These
-- say it. Additive: every row written before this has them NULL, which the
-- screen reads as `unknown` and never as an answer.
ALTER TABLE run ADD COLUMN ran_command TEXT;
ALTER TABLE run ADD COLUMN ran_in TEXT;
-- The names of the environment devpit declared, never the values: a snapshot
-- of a step's environment is a snapshot of whatever secret was in it.
ALTER TABLE run ADD COLUMN declared_env TEXT;
ALTER TABLE run ADD COLUMN base_revision TEXT;
ALTER TABLE run ADD COLUMN head_revision TEXT;
-- 1 when the run had a checkout of the card's own, 0 when it ran in the
-- project, NULL for a row from before this migration.
ALTER TABLE run ADD COLUMN in_a_worktree INTEGER;

-- What the run proved, as a versioned payload rather than seven new tables.
-- The version is on the row so a reader that does not know a shape can say so
-- instead of guessing at it.
ALTER TABLE run ADD COLUMN evidence TEXT;
ALTER TABLE run ADD COLUMN evidence_version INTEGER;
"#,
    },
    // Migration 018 — where the working tree stood when the run saw it.
    Migration {
        version: 18,
        sql: r#"
-- The revision alone cannot say whether a result is still about the code in
-- front of somebody: uncommitted work is most of what a person is looking at.
-- This is a digest of every changed path and the size of its edit, so a run
-- from before an edit reads as `stale` instead of looking current.
--
-- Additive, like 017: NULL is `unknown`, which is what the screen says.
ALTER TABLE run ADD COLUMN saw_changes TEXT;
"#,
    },
    // Migration 019 — who asked for a run, and what carried it out.
    Migration {
        version: 19,
        sql: r#"
-- Two different questions a run could not answer. Who **asked** — a board
-- drag, the checkpoint, a chain, a card's own play — and what **carried it
-- out** — a local process or an agent session under some profile. They are
-- different: Claude can ask for a test the local runner executes and Codex
-- reviews, and one column for both would have lost that.
--
-- Written when the run starts, never from a later event: an event that arrives
-- after the fact must not reattribute the run to whatever is active now.
ALTER TABLE run ADD COLUMN asked_by TEXT;
-- The id of the surface that asked — a tab, a pane, a session. Read back as a
-- reference and never resolved by name: a terminal that has closed is
-- `unavailable`, and matching another pane by a coincidence of name would
-- point somebody at work that is not theirs.
ALTER TABLE run ADD COLUMN asked_from TEXT;
ALTER TABLE run ADD COLUMN carried_by TEXT;
-- The profile the agent session ran under, as it stood then. NULL for a run a
-- local process carried out.
ALTER TABLE run ADD COLUMN carried_profile TEXT;
"#,
    },
    // Migration 020 — how a project looks in the rail.
    Migration {
        version: 20,
        sql: r#"
-- The mark a person gave a project: an icon of the app's own set or an emoji,
-- and a colour. Both NULL until somebody chooses — the rail then draws the
-- initials in a hue taken from the id, so an untouched project still looks
-- like itself and not like its neighbour.
ALTER TABLE project ADD COLUMN icon TEXT;
ALTER TABLE project ADD COLUMN color TEXT;
"#,
    },
    // Migration 021 — one running run per card, held by the database.
    Migration {
        version: 21,
        sql: r#"
-- Checked in the app, and only safe while every command ran one at a time on
-- the window's thread. Now they run in parallel, so a double click on Play or
-- a drop followed by Play can both pass the check. The rule lives here, where
-- no two connections can both pass it.
--
-- A card already holding two (from before this) keeps its newest; the others
-- are closed as failed and say why. Run ids are ULIDs, so the greatest is the
-- latest.
UPDATE run
   SET state = 'failed',
       output = COALESCE(output, 'another run was going on this card; this one was closed so only one runs'),
       ended_at = COALESCE(ended_at, started_at)
 WHERE state = 'running'
   AND id NOT IN (SELECT MAX(id) FROM run WHERE state = 'running' GROUP BY card_id);
CREATE UNIQUE INDEX run_one_running ON run(card_id) WHERE state = 'running';
"#,
    },
    // Migration 022 — the terminal a notice is about.
    Migration {
        version: 22,
        sql: r#"
-- An agent waiting, a command that finished: clicking the notice goes to the
-- terminal it happened in. Kept apart from `detail`, which is text to read —
-- the pane id used to be put there and drawn as `leaf_…`.
ALTER TABLE notice ADD COLUMN pane_id TEXT;
"#,
    },
    // Migration 023 — a card's date can carry a time, and reminds at it.
    Migration {
        version: 23,
        sql: r#"
-- `due_time`: whether `due_at` is a moment somebody chose, or a day. A day
-- reminds when it is over; a moment, at the moment.
ALTER TABLE card ADD COLUMN due_time INTEGER NOT NULL DEFAULT 0;
-- When the reminder for the date as it stands went off, and when the person
-- dealt with it. Both are cleared whenever the date changes, so a date moved
-- reminds again — the bell's own record dedupes per card, forever.
ALTER TABLE card ADD COLUMN reminded_at INTEGER;
ALTER TABLE card ADD COLUMN handled_at INTEGER;
-- Dates already past were told by the bell. Reminding about each of them the
-- first time this runs would be weeks of dates nobody asked about again.
UPDATE card
   SET reminded_at = CAST(strftime('%s', 'now') AS INTEGER),
       handled_at = CAST(strftime('%s', 'now') AS INTEGER)
 WHERE due_at IS NOT NULL AND due_at <= CAST(strftime('%s', 'now') AS INTEGER);
-- What the reminders wait on: the next date nobody has been reminded of.
CREATE INDEX card_reminder ON card(due_at)
 WHERE due_at IS NOT NULL AND archived_at IS NULL AND reminded_at IS NULL;
"#,
    },
    // Migration 024 — what a column is for, when the person says so.
    Migration {
        version: 24,
        sql: r#"
-- `backlog`, `doing`, `check` or `done`. Null is "read it from the name":
-- every board names its columns its own way, and an agent moving its card
-- needs to know which one holds work in progress.
ALTER TABLE board_column ADD COLUMN role TEXT;
"#,
    },
    // Migration 025 — a review finding the person set aside.
    Migration {
        version: 25,
        sql: r#"
-- By the run and the finding's place in its review: a review is written once
-- and never changes, so the place is the finding's identity.
CREATE TABLE finding_dismissal (
    run_id       TEXT NOT NULL REFERENCES run(id) ON DELETE CASCADE,
    at           INTEGER NOT NULL,
    dismissed_at INTEGER NOT NULL,
    PRIMARY KEY (run_id, at)
);
"#,
    },
    // Migration 026 — the sessions an orchestrator's account was seen running.
    Migration {
        version: 26,
        sql: r#"
-- The CLI lists only sessions alive now; this keeps the ones that ended, so
-- they can be read and resumed. No foreign keys: a project removed since does
-- not take a session's record with it.
CREATE TABLE seen_session (
    session_id    TEXT PRIMARY KEY,
    profile_id    TEXT NOT NULL,
    name          TEXT NOT NULL,
    cwd           TEXT NOT NULL,
    project_id    TEXT,
    card_id       TEXT,
    first_seen_at INTEGER NOT NULL,
    last_seen_at  INTEGER NOT NULL,
    last_status   TEXT,
    ended_at      INTEGER,
    ended_by      TEXT
);
CREATE INDEX seen_session_ended ON seen_session(profile_id, ended_at);
"#,
    },
    // Migration 027 — what Decisions was asked, what it answered and cost.
    Migration {
        version: 27,
        sql: r#"
-- The state is never kept, only its digest: it is the project's own text, and
-- the log outlives it. Rows older than ninety days are pruned on write.
CREATE TABLE decision (
    id             TEXT PRIMARY KEY,
    at             INTEGER NOT NULL,
    project_id     TEXT,
    card_id        TEXT,
    gate           TEXT NOT NULL,
    mode           TEXT NOT NULL,
    rubric         TEXT,
    rubric_version TEXT,
    questions      TEXT NOT NULL,
    answers        TEXT,
    thresholds     TEXT,
    outcome        TEXT NOT NULL,
    action_taken   TEXT,
    cost_usd       REAL NOT NULL DEFAULT 0,
    latency_ms     INTEGER NOT NULL DEFAULT 0,
    state_digest   TEXT NOT NULL,
    person_verdict TEXT
);
CREATE INDEX decision_at ON decision(at);
"#,
    },
    // Migration 028 — a plan on the board: an epic's cards, and what each waits on.
    Migration {
        version: 28,
        sql: r#"
-- The epic a card was planned under, one at most. A table rather than a
-- column on `card`: an epic deleted takes the row and leaves its cards on the
-- board as work of their own, and `card` keeps the shape older rows had.
CREATE TABLE card_parent (
    card_id    TEXT PRIMARY KEY REFERENCES card(id) ON DELETE CASCADE,
    parent_id  TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    CHECK (card_id != parent_id)
);
CREATE INDEX card_parent_of ON card_parent(parent_id);

-- A card that cannot start until another is done. Rows, not a list in the
-- body: the board draws "blocked by" from them, and the body is the person's.
CREATE TABLE card_blocker (
    card_id    TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    blocked_by TEXT NOT NULL REFERENCES card(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (card_id, blocked_by),
    CHECK (card_id != blocked_by)
);
CREATE INDEX card_blocker_by ON card_blocker(blocked_by);

-- A Devflow plan previewed for an epic, waiting for the person to create its
-- cards or drop it. One per epic; a new preview replaces it.
CREATE TABLE devflow_plan (
    epic_id    TEXT PRIMARY KEY REFERENCES card(id) ON DELETE CASCADE,
    cards      TEXT NOT NULL,
    -- 1 once the person changed it on the board: their edits are what apply.
    edited     INTEGER NOT NULL DEFAULT 0,
    planned_at INTEGER NOT NULL,
    -- What was created, once it was, and by whose yes.
    created    TEXT,
    applied_at INTEGER
);
"#,
    },
    // Migration 029 — a card's number in its project.
    Migration {
        version: 29,
        sql: r#"
-- `#42`: what a person says and types, where `card_<ULID>` is what nothing
-- else can collide with. Numbered in the order the cards were made, archived
-- ones too, so a number is never handed out twice. NULL only for a card an
-- older devpit made on this file.
ALTER TABLE card ADD COLUMN number INTEGER;
UPDATE card SET number = numbered.n
  FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY project_id ORDER BY created_at, id) AS n
          FROM card) AS numbered
 WHERE card.id = numbered.id;
CREATE UNIQUE INDEX card_number ON card(project_id, number);
"#,
    },
    // Migration 030 — why a call to Decisions failed.
    Migration {
        version: 30,
        sql: r#"
-- The provider's own words when a call it was sent came to nothing, so the
-- Jev panel can say why Decisions is failing. NULL for an answer and for
-- every row an older devpit wrote.
ALTER TABLE decision ADD COLUMN failure TEXT;
"#,
    },
    // Migration 031 — a planned card closed without its own evidence.
    Migration {
        version: 31,
        sql: r#"
-- A card whose work went into another (by_card_id), or that was cut from the
-- plan (NULL): it no longer holds the cards waiting on it, and its gate is
-- the other card's, or none.
CREATE TABLE card_superseded (
    card_id    TEXT PRIMARY KEY REFERENCES card(id) ON DELETE CASCADE,
    by_card_id TEXT REFERENCES card(id) ON DELETE CASCADE,
    at         INTEGER NOT NULL,
    CHECK (card_id != by_card_id)
);
"#,
    },
    // Migration 032 — origins hashed by the normalisation sync relies on.
    Migration {
        version: 32,
        sql: r#"
-- The work is `rehash_origins`, in Rust, just before this (`BEFORE`): each
-- origin kept without a password or token, and hashed host/path only.
SELECT 1;
"#,
    },
    // Migration 033 — the branch a card's worktree was made on.
    Migration {
        version: 33,
        sql: r#"
-- What cleanup and a second checkout trust, whatever the branch is called.
-- NULL for a card an older devpit checked out: its `devpit/…-<id>` branch is
-- found by its name.
ALTER TABLE card ADD COLUMN branch TEXT;
"#,
    },
    // Migration 034 — host profiles: machines a project's terminals run on.
    Migration {
        version: 34,
        sql: r#"
-- Reached over the person's own ssh: a target, never a key. The environment
-- is what marks a production host for the person and the orchestrator.
CREATE TABLE host (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL UNIQUE COLLATE NOCASE,
    target        TEXT NOT NULL,
    user          TEXT,
    port          INTEGER CHECK (port BETWEEN 1 AND 65535),
    identity_file TEXT,
    work_root     TEXT NOT NULL,
    environment   TEXT NOT NULL CHECK (environment IN ('dev', 'staging', 'production')),
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
"#,
    },
];
