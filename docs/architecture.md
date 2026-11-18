# Architecture

Where things live, and the rules the build refuses to let you break.

## Layout

```
crates/core      domain, SQLite store, event bus
crates/git       status, worktrees, clone, log
crates/pty       pty handling, ring buffer, coalescing
crates/rpc       the contract; frontend types are generated from it
crates/tmux      the sessions that outlive the window
crates/agentcli  the agent catalogue, headless turns, hooks
crates/steps     what a column runs, and the runner that runs it
crates/remote    the Remote through the relay: Noise, frames, the relay client, MCP between machines
crates/sync      sync between machines: the key, sealing, the clock, the merge, the namespaces
apps/desktop     the shell — the only crate allowed to know it
web              React + Vite + xterm.js
xtask            the architectural guards
```

## The rules that have teeth

`cargo xtask check` runs thirteen guards, each one a rule that failed silently
before it existed. It names the file and the line. They are not style
preferences:

| | |
|---|---|
| shell boundary | nothing under `crates/` imports the shell |
| agent boundary | one crate drives the agent CLI |
| uncalled commands | a command in the contract is called from `web/src`, or listed with a reason in `xtask/uncalled-commands.txt` |
| dead controls | a button with a handler, or no button — a budget per file that only shrinks |
| size ratchet | a file past its ceiling in `xtask/ceilings.txt` is split, never given a bigger ceiling |
| naming | nothing is named after nothing |
| home paths | paths come from the state root, not from `$HOME` by hand |
| csp | the window's policy allows no remote script and no remote frame |
| versions | the version has one source |
| packaging | the bundle ships what it says it ships |
| release workflow | the release workflow keeps its promises |
| platform window | the platform window matches the base |
| tmux survives | nothing in `apps/desktop` names `kill_server` |

Two more rules are enforced by tests rather than guards: **types come from
Rust** (`web/src/gen/` is generated, and a stale contract fails `make test`),
and **every path is resolved through symlinks and checked against the project
root** — this process runs terminals and writes files, so reaching it is
reaching the machine.

## A card follows its sessions

A card knows what is working on it from links that already live elsewhere and
from what its agents say — never from a copy of their state in a table, which
would still say `working` after the app had closed.

- **Links are rows.** A card's terminal tab (`tab_card_<id>`), a run's own
  session (`run.session_id`, with the folder it ran in, `run.cwd`), a background
  session (`session_link`, with its `cwd`) and a conversation about the card
  (`card_chat`). A card's sessions are these, joined when read.
- **State is heard.** Hooks from panes, headless turns and background sessions,
  runs starting and ending, chat turns and the process table all land in one
  in-memory registry, stamped by one counter, so a late word never undoes a
  newer one. After a restart it is empty, so the app rebuilds what it can on
  its own thread: for the project you were last in, tmux and the process table
  say which card panes have an agent in front, and those read as open until
  the agent says more. Other projects are rebuilt when they are opened. What
  an agent was *doing* — working, waiting, done — waits for its next hook.
- **The window hears it on `card:happening`.** One event per change, with the
  card's sessions and what they add up to. The tile's dot and the open card's
  Sessions section read it.
- **A conversation runs where its session ran.** Its folder is fixed when it
  begins or takes a session in, and no turn makes a checkout.

## What a run writes down

A run is a row, opened when a card lands on a lane with a step and closed when
the step returns. It holds the state (`running`, `ok`, `failed`, `cancelled`,
`lost`), the output as it came, the exit code for a command, the cost and the
duration for an agent turn, the lane it came from, the folder it ran in and the
session it ran as. The card shows the last one; what a run cost is on the card,
and what your plan has left is on the Usage page, which reads the CLI's own
credentials and asks Anthropic.

A `session` step is not limited to one at a time. Each one starts a detached
tmux session for its card and writes a link to it; what is limited to one is
the target terminal, which is the session you are looking at.

## How an update goes in

The check is `tauri-plugin-updater` against a signed feed, and the bytes are
verified against the release public key before anything is written. What
happens then depends on what this build is:

- an **AppImage** is installed over itself and the window restarts. The
  terminals are tmux sessions on a server devpit never kills, so they are still
  there when it comes back — no restore step, because nothing was lost.
- a **`.deb`** is checked again, kept `0644` in a cache directory, and shown to
  you as a command to run. devpit never runs it: installing a system package
  means root, and devpit does not ask for root on anybody's behalf.

From the moment an update is ready, nothing new starts — a turn begun then
would die with the process.

## Context never becomes shell syntax

Context reaches a `command` step **only through environment variables, never
interpolated into the command string**. A branch name containing a space or a
`;` becomes a variable's value, not shell syntax — that removes an entire class
of injection.

## Nothing runs that you did not set up

There is no "work a hundred iterations and tell me at the end" mode, and
nothing schedules or retries work. A step that fails leaves the card where it
is with the reason written on it.

A step that passes does what its lane says: `manual` leaves the card alone,
`ask` asks before moving it, `auto` moves it and runs the next lane's step. A
chain of `auto` lanes is one somebody built lane by lane, and the board says
which lanes those are.

That shape is deliberate. Orchestration that keeps itself going is
orchestration you cannot see, and the problem this project attacks is losing
sight of what is happening.

## An orchestrator is a chat you drive

The orchestrator is you — or a chat you drive, whose every session is on the
board. It is a project devpit keeps for itself, under `orchestrator/<account>/
<name>`, known by its folder rather than by a column, so it gets the chat, the
files and the agent API's scope the way any project does. From there, and only
from there, the agent API reaches past one project: `devpit_projects`,
`devpit_sessions`, a `project` on the board tools, and `devpit_start_session`,
which starts a background session in a card's own checkout and links it to the
card.

The limits are the point:

- **Visible.** Every session it starts is on a card, on the board.
- **No steps.** The gate that refuses an agent a lane with a step applies to it
  too; starting a step stays the person's move.
- **No consent by proxy.** A message between sessions approves nothing — the
  CLI's rule. A session waiting on the person is answered from the
  orchestrator's Sessions panel, which types into that session's own terminal as
  the person. That path is the window's alone; no agent reaches it.
- **Heard, not polling.** Its conversation keeps one process between turns, so a
  session's reply wakes it; nothing here runs it on a timer.

## A channel talks back

The Telegram chat paired with devpit is the person, and no other chat is:
anything else written to the bot is counted and never read. What the
person writes or says there becomes their turn in one orchestrator's chat —
the one chosen in Settings, or the only one — through the composer's door,
marked as sent from that channel, and the turn's last words go back at the
privacy chosen there: nothing named, names and a first line, or whole.
A voice note is fetched up to 3 MB, heard by the engine Settings set up and
deleted.

From a channel a typed yes approves nothing. Sending a draft, stopping a
session and moving a card into a lane with a step become buttons under the
answer, and the press does it, as the person. `channel_talk` takes any
transport; Telegram through the person's own bot is the first.

**devpit's own bot** is the second, for a person who would rather not make
one: Settings → Channels → Telegram offers it only when devpit.app says it has
one, and opens `t.me/<bot>?start=<code>` with a single-use code. It is the
same Telegram channel, through devpit.app: notices go with the hub's
(`via: telegram`), answers to the hub's bot, and what the chat writes comes
back with the hub's pressed buttons, taken only by the machine that linked
it. devpit.app carries what the person lets it — by default only that
something waits, names and an answer's first line once titles are on, never
a whole answer — and keeps none of it; a message or a press the machine
never takes, because it is off, lapses there with a notice to the chat.
At the minimal level a question's choices stay on the machine too, on every
channel: their words are what waits.

## Devflow is switched on by a click

Devflow — briefing, context, plan, execution, review, completion — is a
module of devpit's MCP. A project configures it in
`.devpit/devflow/devflow.yaml`, which is the source of truth: the store keeps
no copy, only that the person clicked.

The rules are [devflow](https://github.com/jholhewres/devflow)'s engine,
`devflow-engine`, the same one devflow's own MCP server runs. devpit passes
it its words (`devflow_dialect.rs`: a card where the engine says item, the
`devpit_devflow_*` tools, `DEVPIT_*` variables). The flow's texts (skills,
personas, rubrics, schema) are devflow's own, said in the words devflow ships
for devpit; devpit keeps none of them. devpit pins every
project's folder to `.devpit/devflow/`, and keeps cards on its board as the
engine's work items (`devflow_work.rs`). The board, the clicks, the gate on
`devpit_finish_card`, the Stop hook, the autopilot, runs attached to cards,
Jev's decision log and the Devflow screen stay devpit's.

devflow is moving out of devpit's build, one method at a time, behind a
process boundary (`devflow_host.rs`). devpit runs `devflow host --root <dir>`
once per call: JSON-RPC 2.0, one line each way on stdio, protocol 1.
`initialize` carries `dialect: {host: "devpit"}`, devpit's folders for runs
and the person's own skills, and the person's switches, and devflow
asks back for what only devpit has. A method devflow serves goes there, and
one it does not serve yet stays on the linked engine.
- **Lookup order:** `DEVPIT_DEVFLOW`, then `~/.local/bin/devflow`, then the
  login PATH. A devflow is used only if it answers `version --json` with
  0.5.0 or later and protocol 1. `DEVPIT_DEVFLOW` set to nothing means there
  is none.
- **Install and update:** with Devflow on and no devflow, or only an older
  one, devpit installs the release itself, once per run, checksummed as the
  Install button does. A program called devflow that does not answer as one
  is left alone. With a usable one, devpit runs `devflow update` once per run
  and day. Both ring the bell. `DEVFLOW_NO_UPDATE` or an empty
  `DEVPIT_DEVFLOW` turns both off.
- **Same file only:** devflow stands in the git repository's top folder and
  reads a `devflow.yaml` at its root or in `.devflow/` first. So a project
  that is a repository's subfolder, or keeps such a file, is never handed to
  devflow.
- **Served today:**
  - `config/write` edits devflow.yaml in place, keeping comments and unread
    fields. devpit refuses a `security.*` path before devflow sees it.
  - `skills/get` gives the text of `devpit_devflow_skill`. A project with a
    team layer is the exception: devpit fetched that layer itself, so it keeps
    reading the text on its own side.
- **Security targets:** devpit holds none in force. Their confirmation is the
  person's, and devflow keeps it in a file any shell can write.

- **Off until activated.** A project with no devflow.yaml, or one that says
  `enabled: false`, gets exactly what devpit listed before Devflow: no
  tools, no plugin folder, no Stop hook, no gate on `devpit_finish_card`.
  So does every project while Settings switches it off. The person starts
  it with Activate Devflow on the project's Devflow screen. A file waiting
  for that click, or one that does not read, gets `devpit_devflow_status`,
  `_setup` and `_set`, and `_skill`, `_plan` (a preview without an epic)
  and `_review`, which run a stage on demand without a card, as
  `devpit agent devflow <stage>` does from a shell; the tools of its stages
  come only where it is on, stage by stage.
- **Agents write files, people switch it on.** Setup and set preview unless
  told to apply, and never commit. `enabled: true` and Decisions in
  `enforce` are never written by a tool: they wait for the person's click on
  a card under the top bar. A file that says `enabled: true` by itself — a
  hand edit, a fresh clone — waits for that click too.
- **A screen beside the board.** Each project's sidebar has Devflow next
  to Board: the setup from what was detected (Recommended in one click, or
  Custom in four steps and a commented preview), the file read-only with
  Activate, an editor, or the error with its line. It calls what the tools
  call — setup, set, the person's ask — so it has no rule of its own, and
  reads the file again when it changes elsewhere. A card's pane shows its
  newest gate, reviewers and files. Settings → Devflow switches it for every
  project, picks where a setup starts, and adds its skills to Codex on a yes.
- **Layers, the project last.** devflow.yaml is read over devpit's defaults,
  the person's own `<home>/devflow/devflow.yaml`, and each team layer it
  `extends` at a fixed ref; a later layer wins except where an earlier one
  `locked` a field, and a change that weakens a lock is refused before it is
  written. A team layer is fetched by git into a bare cache under the home,
  never checked out, capped in size, and read blob by blob at the commit
  `.devpit/devflow/devflow.lock` pins with every file's hash; a new ref
  applies nothing until the person approved what it changes. Version 2 lists
  the stages, each `uses:` a skill — devpit's, the project's, the person's, a
  team's, or one already installed, read where it is — and an own stage's
  gate (a command or a rubric) is run by `devpit_devflow_stage` and required
  by the completion gate. A command a team brought runs only after the
  person confirmed it at that commit. Version 1's map is still read.
- **Open sessions hear it.** The MCP server declares `listChanged` for
  tools, resources and prompts, and watches the project's devflow.yaml and
  the app's stamp file, so switching Devflow on or off reaches a session
  already running.
- **The skills are devflow's text, in devpit's words.** They ship inside
  the binary with devflow-engine, and devflow host serves a newer one when
  installed. The MIT skills their method draws on are credited in
  [`NOTICE`](../NOTICE).
- **The board is the plan.** `devpit_devflow_brief` writes the brief as an
  epic card, `[devflow] Brief: <title>`. `devpit_devflow_plan` previews the
  epic's cards and creates none: the preview waits on the epic, where the
  person moves, cuts or edits it and clicks Create. An agent's `apply`
  creates only the plan the person last saw, and only once. A card's epic
  and blockers are rows (`card_parent`, `card_blocker`); its criteria stay
  in its body. An agent cannot start a card, or move it into the lane for
  work in progress, while a card blocking it is not done.
- **The card keeps its proof.** `devpit_devflow_evidence` runs the card's
  checks through the step runner, in the checkout the agent stands in, the
  card reaching them only through the environment. Each run is a folder in
  devpit's home for the project, `devflow/runs/<card>/<time>/`, never in the
  repository: each check's report and log, the screenshots and video an E2E
  check keeps, and `evidence.json` with the criteria's tests and how they
  ended. A check's verdict is a command step's — exit zero with no report is
  not a pass. The files are attached among the project's artifacts under
  `devflow/<card>/<time>/` as hard links, one copy on disk, and linked from a
  comment devpit signs, so they open on the card; the newest five runs stay
  in each place. Runs an older devpit left in `.devpit/devflow/runs/` are
  still read. With
  Playwright the setup writes `.devpit/devflow/playwright.config.ts`: the
  project's own config as its base, Devflow's specs from `e2e_dir`, a JUnit
  report and every screenshot and video.
- **Several reviewers, one list.** `devpit_devflow_review` runs each of
  `review.reviewers` headless on the person's own CLI and subscription
  (`claude -p`, `codex exec`, `cursor-agent -p`, `gemini`), all at once and
  only through `crates/agentcli`, from one packet in the run: BASE and HEAD,
  the diff, the card, its criteria and the persona — never the session's
  history; what the review is about reaches them in their environment. Each
  answer is held to the findings schema and kept as `findings-<agent>.json`;
  a CLI not installed or not signed in is skipped and said. The findings are
  merged — within an axis, the same file, lines that meet and the same cause
  kept once with `agreement` ("2 of 3"), the cause judged as
  `review.consolidate` says (`jev`, else one model pass, or titles alike) —
  and `block_on` gives the verdict in `findings.json`, the gate's file. An
  axis nobody answered for stays `pending`, which holds the gate, until the
  session hands back its own answers. With `review.scale` on, a small diff
  that touches no sensitive path is read once by `devflow-all`, which
  answers for every axis; the reply says which way it went. The card's diff draws the findings,
  and the person's "false positive" there is kept on the finding, which then
  holds nothing.
- **A review without a card never moves a checkout.** Given a pull request
  (`gh` reads it; BASE is its merge-base with the base branch), a range or a
  worktree of the project, the review reads a pull request or a range in a
  detached worktree under devpit's home, removed once it is done, and keeps
  the run under the project's `reviews/<pr-or-range>/`, attached and on no
  card. Without devflow.yaml it runs on the person's own layer.
- **A pull request hears a review only after the person's yes.**
  `devpit_devflow_publish` writes `publish.json` beside `findings.json`:
  each critical, high and medium on a line of the diff as one comment per
  line, everything else in one general comment, in the destination's
  template (devflow's, replaced by a layer's `reviewers/publish/`).
  Applied, it posts only the draft last shown, with `gh`, as a review that
  only comments; `published.json` keeps the ids, so publishing again edits.
- **Code in production is followed up, not held.** A retro review
  (`mode: retro`, a merged `base` and `head`) answers `follow_up` where it
  would have said `request_changes`; each medium or worse defect carries the
  read-only `impact` query for what it touched since the deploy, and its
  draft one ticket per medium or worse finding.
- **A card is done at the gate.** `devpit_devflow_gate` checks what devpit
  can check itself: the card's checks ran after the last change to the code
  (each changed file by when it was written, so a commit is not a change)
  and passed, every criterion maps to tests that passed, the card's E2E spec
  kept a screenshot, and `findings.json` has nothing in `review.block_on`.
  Then, where Decisions is set up, the `devflow-done` rubric — `done` asked
  once per criterion — which in `shadow` only records and in `enforce`, the
  person's click, can hold the card; without a key the checks alone decide.
  It writes `gate.json` into the run, attaches it, and signs the comment
  `[devflow] gate: n of m criteria with evidence`. `devpit_finish_card` waits
  for it.
- **Jev judges, the person labels.** Devflow asks Decisions four rubrics,
  shipped with devflow-engine and overridden by the project's
  `.devpit/devflow/rubrics/`: `devflow-brief` before planning,
  `devflow-plan` before the cards are made, `devflow-finding` of each
  finding the review consolidates (real, severity, duplicate, inside the
  diff) and `devflow-done` at the gate. The state is small and capped — a
  finding with the lines its file changed, the criteria with their evidence,
  the brief's fields, the plan's cards — never the diff, and every request
  asks for no retention and no training. In `shadow` an answer is recorded
  and nothing else; in `enforce`, the person's click, a failing brief or plan
  holds the agent, and a finding not real or outside the change stops
  holding the card. Every decision is logged; what the person does then is
  its label — reopening a card Jev called done, calling a finding a false
  positive, finishing a card — and Settings → Decisions shows each rubric's
  agreement with them and the `min` and `grey` that would agree most. Only
  the threshold moves; the model is never trained. Without a key, an opt-out
  or `decisions.use`, nothing is asked and the flow runs as it did.
- **Hooks only on devpit's own launches.** A session devpit starts in a
  project where Devflow is on gets its hooks on the launch line, never in a
  CLI's settings: Claude Code a plugin folder (`--plugin-dir`, skills
  `/devflow:<stage>`), Codex the hooks with `-c` and the trust Codex keys
  them by. `Stop` and `SubagentStop` call the gate (`mcp_tool`); a failing
  one sends the turn back with what is missing. A stop the CLI did not
  continue starts the count again; past the cap (Claude Code's own, 8), or
  sent back and back with nothing changed, the turn ends and the card and a
  notice say so. A subagent is held only for code it changed. `SessionStart`
  runs `devpit agent devflow-context`, which gives the session its card.
  Switched off, the next launch has none of it, and a running session's
  gate answers nothing.
- **Skills by layer.** Where it is on, each stage's skill is a
  `skill://devflow/<name>/SKILL.md` resource carrying its SEP-2640 entry
  (digest and size), the `devpit_devflow_skill` tool, and a prompt such as
  `/devpit:devflow-plan`. The app resolves it from the project root: the
  project's `.devpit/devflow/skills/`, then the person's under devpit's home,
  then devflow's built-in one. When hosts declare
  `io.modelcontextprotocol/skills`, `skills/list` and `skills/get` join
  these with the same entries, and the tool is deprecated.
- **Personas by layer too.** A reviewer's persona is read from the project's
  `.devpit/devflow/reviewers/<name>.md`, then each team layer's
  `reviewers/<name>.md`, then the person's `<home>/devflow/reviewers/`, then
  devpit's own. The setup copies no persona and no findings schema: they are
  read from devpit, so a new release's text reaches every project, and a
  project's copy identical to a text devpit once shipped (an older setup made
  it) counts as devpit's. A project customizes a persona by writing one under
  a name of its own and naming it in `review.reviewers`; a persona of another
  name also lets one way of
  reviewing follow a person into all their projects, named in their own
  devflow.yaml, and a company's into its projects, named in its layer.

## Remote

devpit can be reached from the person's other devices over their Tailscale
tailnet, or from anywhere through devpit.app's relay. It is off unless turned
on in Settings, and one way in is open at a time.

**Through the relay** the machine connects out to devpit.app as the
person's account, and a device signed in to the same account is joined to it
there. Inside, a Noise session (`Noise_XX_25519_ChaChaPoly_BLAKE2s`) proves
the machine's key — named by the pairing QR code — to the device and the
device's key to the machine; devpit.app carries frames it holds no key to.
A device is known by its key, and forgetting it ends its session. The protocol
is `docs/remote-relay.md`.

**MCP from another machine** goes the same way and nowhere else. An MCP
client is a device the machine paired as one; `devpit mcp --remote` on the
other computer is a stdio MCP server that seals each message for this
machine, and this machine answers it from a read-only allowlist —
projects, boards, cards, sessions, what waits on the person — refusing every
other tool before the app is asked. Off unless allowed in Settings → Remote;
turning it off or forgetting the client cuts it at once. claude.ai and the
Claude app, which would have to end the encryption themselves, stay out.

**Over the tailnet** the machine serves its own viewer (`web/remote.html`):

- **Where it listens.** On loopback, published by `tailscale serve --https`.
  When the tailnet has no HTTPS, it listens on the machine's tailnet IP
  alone. It never listens on every interface; `cargo xtask check` enforces
  this with `nothing_listens_beyond_loopback`.
- **Who gets in.** A caller must be the machine's own owner in the tailnet
  (the `Tailscale-User-Login` header, or `tailscale whois`). The caller must
  also be a paired device, holding a token from a one-time code shown on
  the machine.
- **What a device may do.** Every device can watch. Typing and answering
  are granted per device on the machine.
- **Terminals.** Each watched terminal is a tmux client of its own,
  `ignore-size`, and `read-only` for a device that may not type. The desk
  never changes.
- **Secrets.** A device allowed to type can keep a secret in a project's
  `.env` through the window's own write. The pairing is read again first, a
  device keeps at most five a minute, and the value is never said back,
  logged or sent on.
- **Talking.** The orchestrators, their accounts' sessions and a project's
  chats open as a chat on the phone. A message to a chat is the person's
  turn through the composer's own `chat.send`, marked for the agent as sent
  from the Remote, and streams to the phone the way a chat reopened
  mid-turn joins it; a message to a session is typed into its terminal as
  the person.
  A photo or a recording goes in pieces under the message ceiling, at most
  8 MB and 3 MB: the photo is kept with the project's pasted pictures, the
  recording is heard by the engine Settings set up and then deleted.

The protocol is `RemoteIn`/`RemoteOut` in `devpit-rpc`, one JSON message per
WebSocket frame on the tailnet and sealed in Noise frames through the relay;
the viewer is the same either way.

## Sync

Two machines of one account share preferences and project settings through
devpit.app, sealed on each with a key devpit.app never holds. Off unless
turned on in Settings → Account → Sync. The design is `docs/sync.md`.

- **One key per account**, made on the first machine and carried to the
  next by the person: an offer the first makes on the relay, taken inside
  the Remote's Noise session, or the recovery code. A machine joining an
  account that syncs is never given a second key.
- **Sealed blobs.** Each namespace — preferences, project settings, the
  machines — is one blob, XChaCha20-Poly1305 under a key drawn from the
  account's, named by an HMAC under another, bound to its name and version.
- **Allowlists.** What syncs is listed per namespace, and a preference added
  later stays local until someone lists it; paths, windows, remote access and
  the vault never sync.
- **Field by field.** A round merges each field by a hybrid logical clock, the
  later change winning and a tie going to the same machine everywhere, so
  every machine settles on the same values in any order. A project is the
  same project on two machines when it clones the same repository.
- **The board** is the next namespace, and stays on each machine until it is
  safe to sync.

## devpit Plus

Plus pays for what devpit.app carries for the person away from the machine:
the relay (the Remote, and MCP from another machine), sync, notices
through the hub and devpit's Telegram bot. Nothing that runs on this machine is ever part of it.

- **Checked offline.** devpit.app signs an entitlement (ed25519);
  `plus.refresh` fetches it with the account's token and keeps it, and
  `plus.status` checks it against a public key built in. It holds until three
  days after the paid period, and past that only what needs devpit.app stops.
- **Off until billing is.** With no key built in (`PUBLIC_KEY` in
  `plus.rs`), Plus is neither checked nor drawn, and devpit.app gates
  nothing until its own billing is switched on. Then an account without Plus
  is refused with `needs_plus` — a 402, or close code 4402 on the relay — and
  the Remote's viewer, the remote MCP client and the machine's relay
  connection say so in words rather than as a status; the viewer stops
  asking until the person acts.
