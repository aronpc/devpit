# Changelog

What changed between releases, written for the people who use devpit. The
release notes are taken from here when a version is tagged.

## 0.1.52 — 2026-11-15

### Project agent

- **The project agent reads a project only once you turn it on.**
  Nothing reads a project on its own any more: not when devpit starts,
  not when a project is linked to an orchestrator, and not when Labs
  turns the project agent on. That Labs switch only makes the agent
  available, and each project stays off until you turn it on, under
  Settings → Maintenance → Labs or with **Learn this project** on its
  Project agent page. A project turned on that was never learned is read
  once, and several are read one at a time.
- **devpit asks before a project learns.** When a project is linked to
  an orchestrator, the top bar asks "Turn on learning for this
  project?", with **Turn on** and **Not now**, and Ask the project on a
  project that is off asks the same in its pane. Nothing turns learning
  on without your answer: Not now leaves the project off and is not
  asked again, the question waits across restarts, and the agent's tools
  on a project that is off refuse, saying only you turn it on. None of
  them starts a read, and a card closed in a project that is off asks it
  for no lessons.
- **Facts read from files go straight into memory.** A fact from the
  agent's own read that rests only on the project's files enters its
  memory at once, marked as accepted automatically and undone from its
  page. Decisions, lessons and other facts still wait for you, so the
  count beside the project shows only what needs your judgment.
- **Your earlier choices carry over once.** On the first start of this
  version, a project already learned stays on, unless you had left it
  out, and every other project is off. The proposals still waiting from
  the first read are accepted once, as **Approve all from onboarding**
  would, which clears the counts.

## 0.1.51 — 2026-11-14

### Providers

- **Chat on Claude Code, Codex, OpenCode, Cursor, Grok and Antigravity.**
  A profile of any of these six CLIs now holds a chat, not only a
  terminal. Answers stream in, thinking shows as a timed thought, and
  commands, file edits and MCP tools show as tool rows with their
  output. Stop ends the turn and keeps the agent running, and after an
  idle-out the conversation picks up where it was; an agent that cannot
  resume starts a new one and says so. Claude Code chats are unchanged.
- **Codex chats run on Codex's own app server.** A message steered into
  a running turn reaches it, and a model or mode switched mid-way
  applies from the next turn. Two Codex profiles with their own
  `CODEX_HOME` run side by side without sharing anything.
- **Cursor signs in with its own login.** A Cursor chat reuses the
  sign-in `cursor-agent login` keeps, and devpit never holds a Cursor
  credential. The agent ends when its chat does or when devpit quits.
- **The model picker lists what the agent offers.** A profile that lists
  no models of its own offers the agent's, under the agent's own names.
  A model the agent refuses is said in the chat before the answer and
  shown as unavailable, with the agent's reason. A refused sign-in or
  session ends the turn with the agent's own message, and Antigravity
  below 1.5.0 is refused with "Update the Antigravity CLI".
- **Permission prompts follow the chat's mode on every CLI.** Full
  access answers them, Accept edits also answers file changes, and the
  rest wait for you in the chat. A prompt names the exact command that
  will run, never the agent's summary or title for it, and "Always in
  this chat" covers that command only. A delete, a move, a fetch or a
  command the agent did not show asks every time. A Codex request for
  more reach than the call is never answered on its own, its prompt
  lists what it grants, and in Accept edits a Codex edit is answered
  alone only inside the conversation's folder. Plan puts the agent in
  its own read-only mode, and an agent that has none is refused Plan,
  saying why.
- **Cost and context read n/a when the agent does not report them.** The
  chat header and each turn's line show n/a instead of $0 or 0%. Codex
  turns show how full the context is, and their cost reads n/a until
  Codex's models are priced.
- **Settings → Providers finds your CLIs.** It looks for the six
  supported CLIs on your PATH and in their usual install folders. Each
  row says the CLI's version, whether it holds a chat or only a
  terminal, whether it is signed in, and the line that installs it when
  it is missing. Sign in types the CLI's own login into a terminal tab
  for you to finish; devpit never handles the credentials. Scan again
  looks anew, and the scan repeats at the check interval. Each profile
  keeps a default model and effort, and every CLI gets the model list.
- **Every CLI gets devpit's tools and the orchestrator's brief, and your
  own configuration is left alone.** devpit's MCP tools reach each CLI
  from files devpit keeps in its own folder. An orchestrator's brief
  reaches every CLI through the instructions file it reads, and where no
  hook hands it your preferences, they lead your message. Terminals
  resume Codex, Gemini, OpenCode and Cursor sessions with each CLI's own
  flag.

### Orchestrator

- **An orchestrator runs on any CLI that holds a chat.** Create one on a
  Claude Code, Codex, OpenCode, Cursor, Grok or Antigravity profile; a
  terminal-only CLI is refused, saying so. Runs as… moves an orchestrator
  to another CLI and back: its folder, notes, linked projects and
  settings stay, its next conversation opens with a short handoff from
  its notes, and the sessions it reached in a devpit pane come along
  with their drafts. It starts sessions on any profile, each briefed the
  way its CLI takes a first message, and lists, messages, reads, drafts
  for and stops them by name. Guardrails decide the same on every CLI,
  and only your own message counts as your word. A message that waits
  two minutes for a session's prompt tells the orchestrator once.
- **Orchestrators and sessions on any CLI message each other.** Through
  devpit itself, beside Claude Code's own messaging: an orchestrator
  writes to a session of a linked project, a session writes back, and
  an orchestrator can ask to be told once when a session goes idle or
  ends. A message to a terminal waits until its agent is idle at the
  prompt, then is typed under a heading that names the sender and says
  it is not you. A message to an orchestrator becomes a turn between its
  turns. Neither ever counts as your word: a session's ✅ approves no
  draft and no stop.

### Project agent

- **The project agent starts and learns on its own.** With Labs on,
  every project linked to an orchestrator gets its agent, which reads
  the project in the background, one project at a time and never while
  it is answering someone. That first read proposes facts and
  decisions, each resting on real files of the project. Each linked
  project has its own switch under Settings → Maintenance → Labs.
- **A page for the project's memory.** "Project agent", beside "Ask the
  project" in the sidebar, shows Facts, Lessons, Decisions and Proposed,
  each entry with its day and its sources, which open in a tab. Approve,
  edit or discard each one, approve the first read's proposals at once,
  or have the agent read the project again.
- **Proposals wait on that page, and each says what it is.** The top bar
  no longer shows memory proposals; the project's icon in the rail and
  its Project agent item show a count. Each proposal says whether it is
  a lesson, a fact or a decision (or an update or removal), who proposed
  it, one plain line and its sources, with the full text behind
  "details".
- **The orchestrator decides proposals only on your word.** It can list,
  approve and discard them when your last message is `approve`, `aprova`
  or ✅, or `discard`, `descarta` or ❌, on its own. Another session's
  message, a tool's output, the agent's own words and a word sent from a
  channel never count, and one message decides only once.
- **Facts that rest only on files can be accepted on their own.** A Labs
  switch per project, off by default. While it is on, a new fact whose
  sources are all files of the project goes straight into memory, marked
  as accepted automatically, and Undo puts it back to wait for you.
  Lessons, decisions and every other fact still wait.
- **The project agent keeps to your own settings.** Since nobody asks it
  to start, it loads only your settings, so a repository's allow rules
  and hooks never apply to it. The repository's deny and ask rules still
  bind it, an ask refusing like a deny, and its `CLAUDE.md`,
  `.claude/CLAUDE.md` and `AGENTS.md` reach it in its first message. It
  cannot message or watch other sessions.

### Fixes

- **Ask the project no longer sticks on "Reading…".** An answer reaches
  its pane even when the pane was closed and opened again while it
  waited, an answer left running in the background no longer goes to
  whoever asks next, and a question nobody answers fails after 15
  minutes with an error, the next one starting afresh.
- **Smaller fixes.** The project agent's answers and briefs reach the
  orchestrator whole instead of cut. A session id that starts like an
  option is never passed to a CLI to resume.

## 0.1.50 — 2026-11-14

### Chat

- **Answers stream smoothly, even in a long chat.** An answer moves as
  it arrives instead of standing still and jumping, and each new block
  fades in (at once, with reduced motion). Only the turn being written is
  drawn again, so the rest of the chat stays still, and a turn starts and
  ends without waiting on git. The chat uses one scale for its type,
  corners and spacing, and answers are drawn at full contrast.
- **Every chat keeps its agent running between your messages.** A message
  starts its answer without starting the agent again, a model or mode picked
  mid-conversation applies to the agent already running, and Stop interrupts
  the answer the way it does in an orchestrator's chat. After ten minutes with
  nothing said the agent stops, and your next message resumes the
  conversation. If it ends on its own before answering, your message goes to a
  new one. While it runs it also hears your account's other sessions and its
  own background tasks, as an orchestrator does: what it says then appears in
  the chat, marked as not in answer to you. The supervised mode still starts
  the agent for each message.
- **Stop always stops, and a steer says when it was missed.** A stop sent
  before the agent has started stops it as it starts, a stop the agent does
  not heed can be forced, and Force stop ends the turn's tools on every
  platform. A steer the agent never took in says so under it, with Resend.
- **Act on any message.** Hover over a message to copy it, edit from
  there, fork the conversation or send it again. Edit from here can put
  back the files the later turns changed. Retry sits beside Continue when
  a turn failed, and a refused request can carry your reason to the agent.
  Select words in an answer to quote them, click a `file:line` to open it
  beside the chat, and click a picture to zoom it. Each turn ends with one
  quiet line: when, which model and effort, how long, how full the context
  was and what it cost.
- **Show as much of the work as you want.** The Activity menu shows all
  of it, hides what succeeded, hides it all, or keeps only your messages
  and the answers (Zen). Subagents started together are one row with each
  one's state. Work left running sits behind one chip by the composer,
  with a Stop for each task and the wakeup the turn set. An opened tool
  row shows its whole input and output, with copy buttons, and names its
  MCP server. A thought says how long it took, and every control reached
  by the keyboard shows a focus ring.
- **Conversations are easier to find, keep and share.** Sidebar rows say
  which agent runs, whether it is working, waiting on you or finished,
  and its branch, project and last message time, grouped as In progress
  and Settled. A header over the chat holds its title (click to rename),
  card, context meter, cost and export; right-click it to choose which
  details it shows. Conversations can be pinned, settled, snoozed, marked
  unread or archived, the command palette finds what was said in any
  project's conversations, and export copies a conversation as markdown,
  with or without its tool calls. Alt+↑ and Alt+↓ move between messages.
- **Clearer states.** A turn refused by a usage limit or an outage shows a
  banner with Try again, and Resume when it resets when the time is known.
  Screen readers hear a turn start and end, and each message says who
  spoke. A picture an answer names on this machine is drawn in it. A plan
  is approved as written, edited or refused, and approving it leaves plan
  mode. `/btw` asks a side question while a turn runs, answered apart from
  the conversation.

### Project agent & Assistant

- **Each project has its own agent.** It reads the project, never changes
  it, and keeps a dated, sourced memory of facts, lessons and decisions in
  `.devpit/project-agent/`. The orchestrator asks it about the project and
  has it write the brief a card's session starts with. It wakes when
  asked and rests after ten minutes.
- **Ask the project from its sidebar.** "Ask the project" opens a chat
  with the project's agent. Answers name the files they rest on, and an
  answer can be saved as a fact once you approve it.
- **Sessions consult it while they work.** A session can ask its project's
  agent up to ten questions and propose what to remember. Nothing enters
  the memory until you approve it.
- **The memory learns from finished cards and stays current.** A card that
  reaches done proposes lessons and decisions; you approve, edit or
  discard each, and approved lessons reach briefs and reviews. An entry
  whose files changed is checked again, and a stale one is proposed for
  update or removal. A card shows the decisions that name it.
- **Assistant mode for an orchestrator.** A toggle in the orchestrator's
  chat. Schedules in its sidebar list every reminder and routine, and
  routines run at their time. A heartbeat (Settings → Assistant) looks for
  failing checks on open pull requests, sessions stuck on a question and
  blocked cards, and speaks once about what needs you. Notices reach
  Telegram, where a stuck session's question can be answered with its
  buttons. Back after 30 minutes away, the orchestrator briefs you on what
  ran, what merged and what waits for your decision.

### Settings

- **Merged cards clean up after themselves.** Once a card's branch has
  landed, devpit removes its worktree, build cache and local branch, a few
  minutes after start and every hour. A checkout with uncommitted work or
  a running process is kept and listed. Settings → Worktrees → Cleanup
  says what went and how much space it freed, and Delete now runs it.
- **Branch names you choose.** A new card's branch is named
  `{type}/{number}-{slug}`, such as `fix/351-flaky-tests`, and the pattern
  is a setting in Settings → Worktrees.
- **Defaults per project.** The default model, permission mode and branch
  name can be set for one project, or in the repository's
  `.devpit/settings.json`. Each row says where its value comes from. A
  repository may only start chats in Supervised or Plan.
- **More control over providers.** A Claude profile lists its models, each
  with an on/off switch, and sets an advisor model and a subagent model.
  Sign-ins and MCP servers are checked every 1, 5, 15 or 30 minutes.
- **A long idle Claude conversation can be compacted.** Pick it on a
  Claude profile in Settings → Providers: when you return, or just before
  its prompt cache expires, so the next turn does not read the whole
  context again uncached. Off by default.
- **Maintenance and Diagnostics.** A power saver that slows devpit's
  background checks while you are away or on battery. Logs and run
  artifacts can be kept for 7 to 90 days. Labs turns the project agent
  and the canvas on or off. Diagnostics shows devpit's own processes with
  their CPU and memory, its checks with their fixes, and the newest
  entries of its error log.
- **Your preferences for chat and display.** Send with Enter or
  Ctrl+Enter, and queue or steer while a turn runs. Text and code sizes,
  more contrast, LaTeX maths, and how diffs open. Keyboard shortcuts say
  when they act, and new ones go to a tab by number, reopen a closed tab,
  set a draft aside and send in the background.
- **An orchestrator's guardrails.** Settings → Orchestrator → Guardrails
  sets, for each confirmation, whether it follows the chat's permission,
  always asks or never asks. A stop or a draft your words do not cover
  now waits above the composer for one click, Stop all or Send all,
  instead of being refused. A click stops only on the loss it showed, and
  a stop whose names read more than one way stops nothing.
- **Settings is easier to find your way in.** General keeps only what is
  general; notifications, sessions and the rest have panes of their own,
  and search still finds every row.
- **devpit in a terminal shows a menu.** Open devpit, see its status,
  update it or run its diagnostics from the keyboard. Subcommands and
  scripts get plain lines.
- **Archived conversations and an About page.** An archived conversation
  leaves the sidebar, the palette and every other list, and Settings →
  Archive restores it. Settings → About shows the version and platform,
  and opens the privacy policy, the licence and the open-source notices.
- **Commit messages drafted in the repository's style.** On an empty
  message, the Changes panel offers Draft a message: a subject for the
  staged changes, written the way the repository's history writes them.
  It only fills the field; nothing is committed on its own.

### Fixes

- **devpit keeps the front on GNOME on Wayland.** On a Wayland desktop
  without layers (GNOME, or one without gtk-layer-shell installed) the
  island can only be an ordinary window: it took focus from the window
  you were in, came first when you switched back to devpit, and clicking
  it left devpit behind the window you came from. There it is now off
  unless you turn on "Show the island anyway" under Settings →
  Notifications. Sessions waiting on you still reach you as
  notifications.
- **A chat's cost counts each turn once.** With the agent kept running,
  each turn reported what the whole process had spent. It now counts only
  what it added, in every chat and in an orchestrator's.
- **Coming back to a project draws its terminals sooner.** Switching
  projects moved the terminals of the project you left inside the
  window, and each move made the whole window work out its styles again.
  They now stay in place.
- **A terminal no longer ends by itself when you come back to its
  project.** Each return redrew every terminal by resizing it, and a
  zsh waiting at its prompt now and then quit on one of those resizes,
  leaving the pane empty. A shell at its prompt is no longer resized to
  be redrawn: coming back already draws what it showed.
- **The board redraws only the cards that changed**, so a busy board
  stays quick when it is read again.
- **A safer sign-in from the browser.** "Open in browser" no longer puts
  a lasting token on the browser's command line, and on Linux the
  one-time code is accepted only over your own connection. devpit's local
  servers read a request within a deadline and serve at most 256
  connections, and the terminal keeper's folder is yours alone.
- **devflow in other tools keeps your files as they were.** Codex's
  config is read as TOML, so an entry written another way is found and
  updated in place. JSON files keep their order, indentation and line
  endings, and each file keeps one backup.
- **Smaller fixes.** Scrolling back through a terminal no longer stops at
  a progress bar. An agent run as a script is named by its script. A
  command waiting to be allowed sits in a box of capped height. Two tests
  that failed now and then on macOS are deterministic, and one of them
  found git reading an early `--since` time as now.

### Installer

- **The installer shows what it is doing.** On a terminal, `install.sh`
  draws a banner, says which version it installs or updates from and to,
  shows a download bar and ticks off each step. Piped, in CI or with
  `TERM=dumb` it prints the same plain lines as before, and `NO_COLOR`
  drops the colour.

## 0.1.49 — 2026-11-05

### Fixes

- **A terminal no longer sticks near the top.** If you had scrolled up
  and Claude Code redrew its screen, the view stayed at the top and new
  output did not bring it down. It now follows the end again. The mouse
  wheel scrolls a session's terminal opened over the orchestrator's chat,
  inside Claude Code too.
- **The orchestrator's preferences hold from the next message.** Editing
  `context/preferences.md` by hand takes effect from your next message,
  without reopening the orchestrator.
- **The orchestrator stops a session only when you ask.** Your last
  message has to be only that request, with the session's full name:
  `stop fix-login` or `✅ fix-login`. It can also stop a session when
  another one already works on the same card.
- **Smaller fixes.** An X11 program started in a devpit terminal opens
  its window, once devpit's terminal keeper has restarted. "Open in
  browser" works on macOS and Windows without Chromium. A browser tab
  hears about tabs opened in the window. The Devflow screen no longer
  offers to replace a newer devflow with an older one.

### New

- **A session in the model you choose.** The orchestrator can start a
  session in a given model, such as Opus, without changing the profile.
- **devflow in other tools.** The Devflow screen adds the standalone
  devflow as an MCP server to Codex, Cursor, OpenCode and Gemini CLI,
  shows where it is added, and removes it.

## 0.1.48 — 2026-11-05

### Sessions

- **devpit no longer starts sessions on its own; turn it on in Settings →
  Sessions.** Opening the next card of an epic and handing a full session
  to a fresh one are both off by default. A session is no longer told to
  stop after one card, so it can take the next card the orchestrator gives
  it. When the handoff is on, the new session works in the same folder as
  the one it replaces, and the old one is stopped once the new one is up.

### Fixes

- **Sessions in a devpit terminal are tied to it again.** An agent devpit
  starts in a pane (from the orchestrator, a handoff or a card) shows "Open
  its terminal here" in the session list, "Send as you" works for its
  drafts, its questions and permission requests reach "Waiting on you", and
  the orchestrator can read its screen. 0.1.47 lost this for terminals in
  devpit's own keeper. Sessions started on 0.1.47 are found again on Linux
  by the pane they were started in; on macOS they stay unlinked until they
  are started again. If the keeper stops, only sessions started on 0.1.48
  come back with their agent.
- **The orchestrator keeps your preferences in view.** Its preferences
  reach it on every message, so they hold after a long chat, a compaction
  or a restart, and a preference it notes applies from the next message.
- **A message the orchestrator could not deliver says so.** It shows as
  "not sent" with the reason, instead of a card that looked delivered, and
  a retry keeps one card. A session the orchestrator starts can be messaged
  as soon as the start answers, and Devflow's reviewers no longer show up as
  sessions.
- **A finished command keeps its output.** An agent's spinner no longer
  fills the block with 50,000 lines, and a screen cleared during the command
  no longer erases what was printed before it.

## 0.1.47 — 2026-11-05

### Chat

- **Long chats stay smooth while an answer streams.** Only the part of the
  thread near the view is drawn, the answer being written redraws only its
  last block, and typing in the composer no longer redraws the chat. Older
  pages load before you reach them, and what you are reading stays in place
  when they arrive.
- **A turn reads in the order it happened.** Prose and the work between it
  alternate as they came; once over, the work folds behind a line saying how
  long it took ("Worked for 1m 30s", "Interrupted after…"), and a failed turn
  stays open on its error. A reload keeps how a turn ended and how long it
  has been working. Pictures you sent show as thumbnails and other files as
  chips; an edit's diff can be copied, and one past 24 lines opens folded.
- **The agent's questions and requests are answered in the chat.** A
  question shows its options as buttons: pick with 1–9 or the arrows, write
  your own answer, or tick several and confirm them together. A permission
  request leads with what it runs, the file it writes or the plan, with
  every line of a command shown; Enter allows and Escape refuses.
- **A queue that waits for you.** Messages queued behind a running turn keep
  their files and skills, and survive closing the tab. Stop, or a turn that
  fails, holds the queue until you resume it. Mod+Enter steers the running
  turn with what you typed, Mod+Shift+Enter with the first queued message,
  and Up and Down in an empty composer bring back what you sent. A paste
  over 32 KB becomes a text attachment, and the draft is kept with its
  attachments.
- **Find, a rail of prompts, and more.** Find searches every page of a chat
  and opens the fold a match is in. A rail lists your prompts down the side,
  a code block has a copy button, a stopped turn can be continued, and a tab
  whose turn ended while you were elsewhere is marked.

### Terminals

- **Terminals outlive the window, with no tmux needed.** New terminals,
  agents' included, run in devpit's own terminal keeper on macOS and Linux.
  Quitting, a crash or an update finds them running, with the agent's screen
  as it left it; a keeper that died brings each terminal back in its folder
  and starts its agent again. Terminals made in tmux stay there until you
  close them, and when the keeper cannot start, tmux takes the terminal as
  before. The terminal menu's "Open in tmux" still opens one from a tmux
  client, and copies the line that attaches to it.
- **Lighter terminals.** Every terminal's output reaches the window on one
  connection. A terminal flooding output skips the lines that would only
  scroll out of its history, so a long build log draws about twice as fast
  and no longer freezes the window. A terminal out of sight stops reading at
  once and keeps its screen, so a project you come back to shows its
  terminals quickly. Whether an agent is working or waiting comes from its
  own hooks and the terminal keeper, so devpit no longer polls every pane and
  process to find out.
- **Closing a tab stops only what runs in it.** A tmux server, a gpg agent or
  another daemon started from a terminal is no longer stopped with its tab.

### Remote

- **Typing from another device echoes quickly.** A terminal's output reaches the
  Remote as raw bytes, and is sent every 8 ms instead of waiting up to 40;
  over the tailnet the rest of an echo no longer waits for the device. Older
  viewers keep working as they did.
- **A new device is paired only once you allow it.** A device that enters the
  pairing code, or scans the QR code, waits while Settings → Remote asks you
  to Allow or Refuse it; no answer in two minutes refuses it. The code and
  the QR code are good for ten minutes, once.
- **A device you forget is cut off at once**, even while one of its terminals
  is printing.

### Devflow

- **Devflow is optional, and off until you activate it.** A project without
  it gets none of its tools, hooks or gates, and the Devflow screen leads
  with Activate Devflow. A project that already had it on stays on.
- **Devflow is its own project now.** devpit runs it on the standalone
  devflow engine, and the Devflow screen installs the devflow plugin into
  the project's Claude Code profile and the standalone binary, checked
  against its release's checksums.
- **Fewer rounds to close a card.** An approval closes the review, and a
  blocking finding that comes back asks you for a design instead of looping.
  The gate counts a proof written after the checks ran and reuses a green
  run at the same commit. The plan rubric is advice unless devflow.yaml
  opts into it, and release checks show in the setup.

### Linux

- **Native Wayland on Linux.** On a Wayland desktop the AppImage opens on
  Wayland instead of going through XWayland. It falls back to X11 when there
  is no compositor, when the desktop's GTK settings are known to crash it, and
  after a Wayland start that never loaded its window, for that version or
  until Wayland is chosen again.
  Settings → General → Display chooses Wayland or X11 for the next start, and
  `DEVPIT_GDK_BACKEND` overrides both for one.

### Board and sessions

- **A big board opens a page at a time.** A long lane draws its first 30
  cards and the next as you scroll. Agents read the board 20 cards per lane,
  with the done lane counted rather than listed, and a card moved while they
  page through is read once.
- **One card per session.** When a card is finished, the next card of its
  epic starts in a session of its own, and a card's session past 30% of its
  context window leaves a handoff and is relieved by a fresh one. Each card
  starts light instead of paying for every card before it.
- **devpit's tools answer agents briefly.** The board, a card, evidence and
  reviews answer a compact summary, with the whole answer one argument away,
  and a slow call no longer holds an agent's other calls.
- **Rules for sessions can be per project.** A preference an agent logs no
  longer lands among the rules every session is briefed with, and a rule can
  be kept for one project's sessions only.

### Orchestrator

- **A ✅ sends the orchestrator's draft, in any language.** Replying ✅ (or
  ✔, ☑, ✓) alone sends the one draft waiting, and beside a session's
  name sends that session's. It needs no word of English or Portuguese, and
  "envie", "send it" and the rest still work. As before, only your own
  message counts, never another session's or a tool's.

## 0.1.46 — 2026-10-30

### New

- **An orchestrator's links follow you to another machine.** With sync on,
  an orchestrator set up under the same name on two machines keeps one set
  of linked projects, matched by repository rather than by folder: linking
  or unlinking a project on one machine does the same on the other, a link
  to a repository the other machine does not have waits there until it is
  cloned, and a link to a folder without a git origin stays where it was
  made. Its name travels with it; the account it speaks as stays each
  machine's own choice.
- **Agent profiles follow you, without their secrets.** With sync on, a
  profile made on one machine shows up on the others with its name, agent,
  program, models and the names of its variables; its arguments stay where
  you typed them. Tokens and keys never travel: Settings → Providers says
  what a profile needs on this computer. Nothing another machine sent runs
  without your yes: a profile that arrived waits to be taken, and a change
  elsewhere to its agent, program or base URL waits for you to use it or
  keep yours. A profile or a variable you remove stays removed everywhere.
- **Open devpit in your browser.** The tray's "Open in browser" serves
  devpit's own page on this machine, signed in for this run only and under a
  name that is this install's own. Nothing listens beyond loopback.
- **Claude Code accounts, per project, with their quota.** Settings →
  Accounts lists the Claude Code config folders you use and how much of each
  account's 5-hour and weekly limits is left. A project can name its account,
  and the sessions devpit starts there open in it. When an account is nearly
  out, a banner offers to start the next session in another; the sessions
  already running stay where they are. The account's token is only read on
  this machine and never reaches the window.

### Faster

- **Long chats open and scroll without freezing.** A chat opens on its last
  page, read from the end of the transcript. Long tool outputs arrive cut,
  and arrive whole when you open them. A streamed answer no longer copies the
  whole thread on every batch.

## 0.1.45 — 2026-10-28

### Fixed

- **Devflow's reviewers run on the project's account.** They used to start
  Claude Code on the machine's default account, so a personal project's
  review could run on a company account. They now run as the profile the
  project names (Devflow → Review), else as the session that asked, else as
  the orchestrators linked to the project, and the screen says which.
- **A chat turn that fails says so, and stops working.** A turn refused
  (another one running, an update coming) or failed mid-way no longer leaves
  its answer "working" until a reload; what it said before failing is kept;
  the CLI's own reason (a session it cannot resume, say) shows in the answer
  instead of an empty "interrupted"; and the error shows at the end of the
  thread, not at its top.
- **Publishing a review no longer reports a false error, nor posts it
  twice.** devpit read back the review's comments where GitHub lists them
  without their lines, so a comment that was there was called missing, and
  publishing again posted a second review. Every comment a review carried
  is now kept as posted, and publishing again only edits.
- **One repository is one project on every machine, however it was
  cloned.** A clone over https with a user, a token or a port, or over
  `ssh://` with a port, now matches its `git@` twin; a project whose remote
  moved to another repository takes the new one when it is opened; and a
  token in an origin is no longer kept, nor sent with the project's synced
  settings, which now carry its address so another machine can clone it.

### Better at what it already did

- **Switching projects is quick again.** Opening a project read the git
  status of every checkout of every project, seconds on a machine with many;
  it now reads only the project in front, the only one whose status shows,
  and the orchestrator's boards are not read again for the same projects.
- **A board is lighter to read.** It lists every card's runs without their
  output, which only the card itself shows.

### New

- **A reviewer's model can be a tier.** `model: small` runs a Claude reviewer
  on Sonnet and `large` on Opus; any other value is a model id, as before.
  Other CLIs keep their own default for a tier.

### Removed

- **Devflow's autopilot is gone.** It started each card's session as the
  default agent, not the project's or the orchestrator's profile, so work
  could run on another account. Card sessions are started by the person or
  the orchestrator, as before it existed. It kept nothing on disk, so there
  is nothing to clean up.

## 0.1.44 — 2026-10-16

### Fixed

- **"database is locked" on live sessions is gone.** The poll that lists
  agent sessions rewrote what it had already kept on every pass, taking
  the write lock each time; a poll that sees nothing new now writes nothing.
- **Fewer pauses of the window.** A hover over the terminal reads only the
  rows around the pointer, and pausing or resuming from the tray no longer
  writes from the window's thread. When the page does stall, the report
  says what it was on and which commands were working.
- **A headless reviewer's answer is no longer thrown away** for a field
  devpit fills in itself: the answer is signed before it is checked.
- **Severity is held to what a finding is.** A missing test, a written rule
  or style is at most medium, a problem in test code at most low, and a
  search is not a proof of concept.
- **A newer devflow.yaml still reads.** A field this devpit does not know is
  named and skipped instead of refusing the whole file.
- **An agent moving, starting or finishing a card is quick again.** It reads
  the board without asking every agent CLI for its live sessions.
- **A setup's apply writes only what the person saw.** An agent's apply
  that differs from the last preview comes back as a new preview.

### Better at what it already did

- **Setup proposes only checks that can run here.** A command whose program
  is missing — or a `make` target whose recipe calls one — is listed as
  unavailable with why, on the setup screen too, and is not recommended.
  Setup also finds modules in subfolders (`unit-<folder>`) and adds a `tsc`
  typecheck when TypeScript has none.
- **Checks with prerequisites and shared resources.** `requires:` runs a
  probe first and leaves the check "not run" when it fails, instead of red.
  `exclusive:` makes checks that share a resource (a test database) wait
  for each other, across cards and worktrees.
- **Proof without a test runner.** A criterion can be proved by a
  screenshot, an E2E run or a file, which the gate checks is there. A card
  that only gathers evidence or writes docs closes on its proofs without a
  code review.
- **Reviews that fit the change.** `review.scale` runs one combined
  reviewer on a small change that touches nothing sensitive. After a fix,
  `previous` re-reviews only what changed and carries the rest over.
  `review.verify` has the session run the proof of concept of each critical
  and high, and records it as confirmed, refuted or not run.
- **Findings point at real lines.** Each finding is anchored on a line of
  the diff; one on no line is dropped with why, and the card lists each
  `file:line` once across reviewers. The reviewers start with no MCP
  servers and a turn limit, and all read the same packet.
- **Decisions says where it stands.** Every Devflow answer that asks
  Decisions says whether it is on, off, without a key or failing, and why;
  the Jev panel and the card say the same.
- **Plans across epics, merged and cut cards.** A plan card can wait on a
  card of another epic (`#42`). Once the cards exist, a card can be merged
  into another or cut from the plan, releasing the cards waiting on it.
  `unblock_on: gate` lets them start as soon as the gate passes. An
  orchestrator can pass on the person's yes to a plan, quoted on the epic.
- **Runs stay out of the repository.** A card's runs are kept in devpit's
  folder for the project and attached to the card. A setup copies no
  persona or schema into the repository, keeps the comments in your
  devflow.yaml, and evidence names the devpit and skills that made it.
- **Cards name what blocks them by number,** and an agent can edit or
  delete a comment it wrote — never someone else's.

### New

- **Devflow on demand.** Wherever Devflow is on in Settings, a review can
  run without a card or a setup: on a GitHub pull request, a `base`..`head`
  range or a worktree. A pull request or range is read in a detached
  worktree in devpit's own folder, so your checkout never moves. A plan
  without an epic is a preview. `devpit-agent devflow <stage>` reads a
  stage's skill or calls its tool from a shell.
- **Publish a review on its pull request.** A review of a pull request is
  drafted for it: what needs changing goes on the lines it concerns, one
  short comment per line, and suggestions go in one general comment.
  `devpit_devflow_publish` posts only the draft you said yes to, as a
  review that only comments, and edits those comments when published
  again instead of posting them twice. GitHub posts; GitLab and Bitbucket
  get the text to paste.
- **Review code already in production.** `mode: retro` on a merged range
  answers "follow-up" instead of "request changes", drafts one follow-up
  ticket per medium or worse finding, and asks each defect for a
  read-only query of what it touched since the deploy.

## 0.1.43 — 2026-10-14

### Fixed

- **"devpit is still working on …" should be rare now.** Every question an
  agent asked made devpit run git once for every project you have, and each
  of those copied the whole app in memory to start. Under a build that
  took longer than the agent waited, so a comment or a card move looked
  lost and then landed. devpit now finds the project from the files git
  keeps, starts programs without copying itself, and an agent waits 30 s
  instead of 5 for an answer. Codex waits on devpit's long tools —
  checks, reviews, a new session — instead of giving up after a minute.
- **A card from another project stays out of reach.** Reading the state of
  a card's checkout now checks that the card is on the asking project's
  board.
- **No more leftover processes** from opening a folder, a file or the
  sign-in page.
- **Devflow's headless reviewer works with `"verbose": true`.** With verbose
  on in Claude Code's settings, every review came back as "invalid JSON".
  When an answer is not findings, what the reviewer said is kept in the run
  and named in the reason.
- **A review reads the card's own work.** A card started in a Devflow
  checkout keeps the commit it began at, and a review without a base diffs
  from there instead of from where the branch left `main`. The diff is the
  commits to `HEAD`, a submodule as its log, and work not yet committed in
  a section of its own.
- **The gate holds code nobody reviewed.** A review counts for the commit
  it read. If the code changed after that commit, the gate asks for a new
  review, so a card that used to pass with a fix committed after its
  approval no longer does. Changes under `.devpit/` do not count.
- **Devflow checks with no report can pass.** A check set to `report: none`
  (a Makefile target, a linter) is judged by its exit code: 0 passes, 127
  (command not found) is "not run", and anything else fails. Before, it was
  always inconclusive, so a gate on it could never pass.
- **`go: not found` is gone.** When GNOME starts devpit (it sets
  `SHLVL=0`), devpit now asks your login shell for `PATH`. It also searches
  Go's usual folders and never keeps an AppImage's temporary mount on it.
- **Go and parameterized tests count.** A Go check reads `go test -json`
  test by test, and setup recommends it for a Go module. A criterion's test
  matches every run of a parameterized test: PHPUnit and Pest data sets,
  pytest parameters and Go subtests.
- **Devflow without Decisions, and Decisions that fails, both work.** The
  `devflow-finding` rubric now says what each severity means, so Decisions
  no longer refuses every call. devpit refuses a choice question without
  per-option criteria before sending it. When Decisions fails during a
  review, the findings are consolidated as if it were off, and the note
  says why. Calls Decisions never answered no longer count toward a
  rubric's accuracy.

### Better at what it already did

- **Cards have numbers.** Every card gets a number in its project, shown as
  `#42` on the board and on the card. An agent can name a card by its number
  wherever it took the id, and an orchestrator can write `project#42`.
- **The board keeps up without working so hard.** A burst of changes from
  an agent is one reload, not one per change. A board behind another tab or
  a hidden window reloads when you come back to it. Only the tile whose card
  changed is drawn again. Cost and session readers stop while the window is
  hidden.
- **Agents get a shorter, truer list of tools.** A project session no longer
  sees the orchestrator's tools, and Decisions' tools appear once a key is
  set. That is about half the tokens on every turn. Descriptions now say what
  each tool really does and when it refuses. A card can be passed as
  `cardId`, `card` or `epic`, and a session as `name` or `session`. An error
  says what to do next.
- **Devflow asks with buttons.** In Claude Code, the briefing's questions,
  the setup's decisions and the yes to a plan come through its question
  picker, with the recommended answer first. Other agents get the questions
  as text, as before.
- **Credits.** The Devflow skills now name the MIT skills their method draws
  on, in `flow/NOTICE.md`.

## 0.1.42 — 2026-10-12

### Fixed

- **An update goes straight to the newest release.** With an update
  downloaded and waiting for its restart, devpit stopped looking, so two
  releases in a row took two updates. It keeps looking now, and a newer
  release takes the waiting one's place.

### Better at what it already did

- **Buttons you can see.** Every button has a face and an edge, the main
  action on a screen is filled with the accent, and the controls inside a
  row are smaller and quieter.
- **The Devflow screen, redone.** What a setup found is a table — each
  command with where it came from, each agent CLI with its sign-in, long
  lists folded — and Recommended and Custom are two cards to choose from.
  The stages are numbered cards along the flow, checks and reviewers are
  tables, the severities that hold a card are one control, and a Custom
  setup walks a stepper with Back and Next always at the foot of the pane.
  Its text is a size larger.
- **Your way of reviewing, in every project.** A reviewer persona is now read
  from the project, then the team layers it extends, then your own
  `~/.devpit/devflow/reviewers/`, then devpit's. Name a persona of yours in
  your own `devflow.yaml` and it reviews all your projects; a company's
  persona in its layer reviews the company's.

## 0.1.41 — 2026-10-12

### Fixed

- **The window no longer stops answering when tmux is busy.** A tmux server
  too slow to answer used to hold one of devpit's threads per call until
  none was left, and then nothing reached the board. tmux calls now give up
  after 10 s, the window waits for one answer before asking again, and
  devpit writes `devpit stall:` to its log if anything stands still.
- **"database is locked" is gone again.** Commits and closes no longer wait
  for the disk while holding the store's locks, so a slow disk under a build
  refuses nobody.
- **A file's tab is named after the file**, however it was opened — not
  `files.rs`.

### Better at what it already did

- **Keyboard shortcuts, all in one place.** Settings → Keyboard lists every
  shortcut, lets you record new keys and refuses one already taken, naming
  which. `?` or Ctrl+/ shows them all. Inside a terminal only the shortcuts
  meant for it fire; the rest go to the program. Splitting no longer splits
  every open tab, Ctrl+G no longer opens Claude Code's editor too, and Ctrl
  vs ⌘ follows the platform.
- **The tray icon stays and helps.** Closing the window keeps devpit in the
  tray where there is one (off in Settings → General). The icon shows a dot
  and a count when something waits on you, and its menu opens each item,
  recent projects, a new terminal, pause and Settings.
- **Every control shows the pointer it acts with**, maximize turns into
  restore, icon-only buttons have tooltips, and hover and focus are the same
  everywhere.
- **Sign-ins before a session starts.** A footer chip shows each account's
  sign-in; a start from the window checks it and holds with the command that
  fixes it. The MCP check no longer slows a start.
- **Sessions.** A session's row says whether Remote Control is on, with its
  link, and a card lists the sessions that worked on it, each resumable.
- **The orchestrator keeps one name across restarts**, so sessions can
  always report back, and its chat answers `/sessions`, `/waiting`, `/ended`
  and `/resume-session` itself.
- **Channels escalate.** A notice nobody acts on moves from push to Telegram
  to e-mail; the calendar of card dates is now served by devpit.app, titles
  off unless you switch them on.
- **Secrets can carry a rotation reminder**, and the Remote can add one to a
  project's `.env` without the value passing anywhere else.

### New

- **The Remote from any network.** Without Tailscale, the Remote reaches
  this computer through devpit.app's relay, encrypted end to end: devpit.app
  splices bytes it cannot read. Tailscale stays as the other choice.
- **Chat on your phone.** The Remote talks to the orchestrators and their
  sessions, with dictation transcribed by this computer and photos that land
  in the project.
- **Talk to the orchestrator from Telegram**, in text or voice. Sending a
  draft, stopping a session or moving a card is a button, never a typed yes.
- **devpit's MCP from another computer**, read-only: board, sessions and
  what waits on you, through the same encrypted relay. Off by default.
- **Sync between your computers**: preferences and project settings, sealed
  with a key devpit.app never holds. Off by default; secrets never sync.
- **Devflow**: a short engineering flow served by devpit's MCP — briefing,
  context, plan, execution, review and completion — with the plan on the
  board (epics, child cards, dependencies, acceptance criteria you tick),
  evidence kept on each card, a completion gate before a card is done,
  review by several agent CLIs at once, Jev judging in shadow first, and a
  Devflow screen in every project. Off until you switch it on, with a click.

## 0.1.40 — 2026-10-08

### Channels

- **devpit tells you what waits on you while you are away from it.**
  Settings → Channels sends a session waiting on a question, failing or
  finishing, a draft ready, a reminder, a step failing or devpit's MCP
  restarting to the channels you connect — never while devpit's window is
  in front or devpit is paused.
- **Telegram, through your own bot.** Paste its token from @BotFather and
  send it the code devpit shows: messages go straight from this computer,
  with no server of ours in between. The token is kept on this computer,
  readable by you alone, and never shown again.
- **E-mail and push on your phone, through devpit.app.** Switch on
  *Send notices through devpit.app* and choose at devpit.app/account: e-mail
  (after you confirm the address, with a one-click way out) and push on any
  device that installs devpit.app. Only that something waits leaves this
  computer, unless you switch titles on; devpit.app keeps nothing of it.
- **Buttons that act on this computer.** *Send the draft*, *Done* and *In 15
  minutes* for a reminder, and the first two choices of a session stopped on
  a question — picked only while it is still the question on its screen. A
  button works once, for half an hour, and this computer does it as you.
- **Which event goes where, quiet hours and one message for many.** A
  matrix of events and channels, quiet hours by day that a reminder you set
  still breaks, and a short wait that turns five sessions finishing at once
  into one message.

### Remote

- **Start work from your phone.** A device allowed to type makes a card and
  starts a session, on a card or in the project, checked for the sign-ins
  the project needs like any other start. The board opens with what waits
  on you.
- **Your card dates as a calendar.** Settings → General → Remote gives an
  address a calendar app on one of your tailnet's devices subscribes to;
  *New address* retires the old one.

## 0.1.39 — 2026-10-08

### Sessions

- **The folders of the projects you add are trusted for Claude Code.** Adding
  a project answers "Do you trust this folder?" in every Claude Code account
  devpit knows, for the project, each worktree it makes and every folder it
  starts a session in — a session the orchestrator starts no longer waits on
  that question. One that shows up anyway, in a session devpit started, is
  answered yes, unless the folder brings its own permissions. *Trust the
  folders of the projects I add* in Settings → Providers turns it off.
- **A session is not started without the sign-ins its project needs.** The
  AWS profile a project's `.env` names, and whatever its `needs.txt` lists
  (`aws <profile>`, `gcloud`, `gh`), are checked first; the orchestrator is
  told the command that fixes a missing one, and starts anyway only once you
  say so.
- **A tool that fails for want of a sign-in rings the bell once**, with the
  command: an expired AWS token, gcloud or gh signed out, an MCP server
  answering 401.
- **A new worktree gets the project's `.env` files** when its setup declares
  nothing. Each setup command stops after ten minutes, and the card says what
  the setup did, or where it failed and how its output ended.

### Orchestrator

- **"Envie" works from the Claude app too.** The orchestrator sends a draft
  when your last message asks for it — a bare "envie", or the session named
  with what to do — said in devpit's chat or over Remote Control. devpit reads
  your message in the conversation's own log; another session's words or a
  tool's never count, and one message sends once.
- **It knows where the round stands after a compaction.** devpit keeps
  `context/now.md` in its folder — sessions running, what waits on you, drafts,
  reminders, sessions ended lately — rewritten at each of your turns.
- **A card keeps a handoff note.** A session leaves what it did, what is left,
  how to test it and the risks; the next session handed the card starts from
  it, in another account or after a restart.

### Projects

- **Secrets go in the project's `.env`, never in a chat.** Edit project →
  Secrets takes a name and a value from a password field; the file is yours
  alone and kept out of git in this clone. devpit keeps the name and the date,
  never the value. The chat says so when what you are about to send looks like
  a key.

### Account and community

- **devpit Plus has a waiting list.** Settings → Account, and Remote without
  Tailscale, link to devpit.app/pro: one click to join while signed in.
- **devpit has a community on Discord.** *Community* in the account menu and
  at the foot of Settings opens it in your browser, and each release is
  announced there.

## 0.1.38 — 2026-10-07

### Sessions

- **A command can be sent to a session.** *Send command…* in the Sessions
  panel offers `/remote-control`, `/compact`, `/clear`, `/model`, `/status`
  and `/cost`, or one written by hand, and types it in the session's terminal
  as you once you confirm it.
- **Sessions that ended stay in reach.** The Sessions panel lists them under
  *Ended*: when, whether you or the orchestrator stopped them, what each said
  last and files left uncommitted in its folder. *Resume* opens the same
  conversation in a new terminal of its project, in the folder it ran in.
- **Stopping a session that is working, or has work not committed, asks
  again** and says why, with *Stop anyway*.

### Orchestrator

- **"Send it" sends the draft.** Said alone in the orchestrator's chat (also
  *envie* or *manda*), it sends the one draft waiting. The island shows a
  session's draft with *Send* too, and every draft sent is recorded in the
  orchestrator's sessions log: to whom, from where, and whether it was edited.
- **It reads and resumes sessions that ended**, and a stop it asks for is
  refused while a session is working or has work not committed, unless the
  person said so.

### devpit's own MCP

- **It is checked every 30 seconds and restarts by itself** when it stops
  answering, without restarting the app or dropping the orchestrator's chat;
  the bell says so. A *devpit* chip in the footer shows how fast it answers,
  with *Restart MCP*.

### Remote

- **A terminal's key bar has Claude Code's commands**, each confirmed first;
  the orchestrator's drafts are sent from the *waiting* tab; devpit's MCP is
  restarted from there too.
- **Add to devpit.app** puts this machine on devpit.app/remote. Only its
  address goes, in the link's fragment — no token.

### Files

- **Video and audio play in a tab**, with seeking: MP4, MOV, WebM, MP3, WAV,
  Ogg and FLAC. They are streamed from devpit, never sent whole.
- **Pictures past 8 MB open** instead of being refused. A click switches
  between fitting the pane and the picture's own size, with its dimensions
  under it. A file devpit cannot show offers the system's app.

### Settings

- **Voice tests the microphone in place**: three seconds through the same
  microphone and engine a chat uses, and what was heard. With whisper.cpp,
  *Download base model* fetches one and chooses it.
- **Remote shows what paired devices did lately**, never what they saw.
- **Decisions** (new, off by default): typed decisions by Jev, with your own
  key, model, a daily cap and a switch per project. Agents ask through
  `devpit_decide` and `devpit_rubric_run`. Nothing is sent without a key.

### Fixes

- **Moving a card into a lane that runs a step no longer fails with
  "database is locked"** or takes over 30 seconds while agents are working.
- **A session restored after a restart continues its conversation** on the
  default Claude account; it used to start afresh.

## 0.1.37 — 2026-10-07

### Chat

- **A document opened from an answer is resized by its edge**, and the chat
  narrows beside it instead of running on underneath. The chat's buttons
  stay over the chat, and the document's own over the document.

### Fixes

- **A page in a browser pane no longer draws over the Manager, Settings or
  the command palette.** It steps aside while one is open and comes back
  when it closes, as it does when its tab is not the one showing.

## 0.1.36 — 2026-10-05

### Chat

- **Answers are written out as they arrive.** Text appears word by word at
  a steady pace from the first token, with a caret while the turn runs, and
  says *Thinking* as soon as the agent starts to think. With reduced motion
  it still arrives as it is written, without the animation.
- **An answer to a message from another session is one paragraph again**,
  not one line per piece it was streamed in — answers already saved that
  way read whole too.
- **A reply the orchestrator drafts is a card in its chat**, with *Send*,
  *Edit* and *Drop*: you send it to the session's terminal as yours without
  leaving the chat. A session stopped on a question shows it in the same
  card. The orchestrator still sends nothing itself.

### Sessions

- **What a session has spent is shown where you use it**: in a terminal's
  corner, a chat's, each row of the Sessions panel and the island. Its
  tooltip has the last turn, what was spent since devpit first saw it, the
  tokens and the model. Dollars are API list prices; a click shows tokens
  instead.

### Fixes

- **A session in a project added a moment ago is in an orchestrator's
  reach** right after it is linked; it used to be refused for a few seconds.
- **A page an MCP tool comes with always gets its call and result.** It
  could load before the window listened, and wait for them forever.

## 0.1.35 — 2026-10-04

### Chat

- **Answers stream as they are written.** Text arrives word by word instead
  of all at once when the turn ends, and the line under a live turn says what
  it is at: the command or file it is on, *Thinking* or *Writing*.
- **A queued message can go now.** *Send now* on a message waiting in the
  queue hands it to the turn that is running, without waiting for it to end.
- **Links in an answer open in your browser**, `http://localhost` included.
  A link that cannot be opened says why instead of doing nothing.

### Board

- **A lane's step can start from a recipe**: *Tests* runs the command your
  project's own files name and reads its report; *Review* has an agent file
  findings on the card's change and sends a blocker back; *Prove-It* runs a
  bug's test on the card's base and on its checkout, and is not proven unless
  it failed first.
- **A review's findings sit on their lines in the card's diff**, with how bad
  each one is, why, and whether it was made against older code. A count over
  the diff goes to the first one, and a finding can be dismissed.
- **Command steps read their test reports**: a green run with passing tests
  is *Passed*, not *Inconclusive*.
- **Worktree preparation's shared variables** reach steps and terminals in a
  card's checkout.

### macOS

- **Agents are found when devpit is opened from Finder.** devpit reads your
  login shell's `PATH` and the usual install folders (the native installer,
  npm, bun, nvm, asdf, mise, Homebrew), a command in Settings may be `~/…` or
  absolute, and terminals keep UTF-8 without a locale.
- **Copy environment info**, under the agents in Settings, gives what devpit
  sees of your machine for a bug report; when no agent is found, devpit says
  where it looked.

### Fixes

- **devpit's MCP tools no longer time out after hours open.** Every question
  an agent asked listed the status of every checkout of every project; it now
  answers in a fraction of the time, and says so in time when it is slow.
- **A reloaded window ends the terminal connections its previous page left.**

## 0.1.34 — 2026-10-02

### Orchestrator

- **The Sessions panel lists what runs.** A linked project with nothing
  running no longer takes a group unless you ask for it. A project the
  orchestrator does not reach is linked from its group. A row names its card,
  stops its session from a button, and a search finds a session by name,
  project or card.

### Settings

- **Every row is a card, and only the switch switches**: clicking a row's
  words no longer turns it on. The long half of each row folds under *How it
  works*, and a search scrolls to the row it matched.
- **Remote is one card**: its steps, the address with a Copy button, and
  chips for what each device may do. A paired device is named after its
  system rather than *A device*.
- **Voice looks for a whisper again** from Settings, without reopening it.

### Fixes

- **Closing the window ends devpit**, and *Quit devpit* in the tray always
  does. devpit used to stay running behind the island with no window, and the
  tray could no longer open or quit it. Terminals keep running in tmux.
- **A session resumed after a restart is placed in its own terminal.** It
  could be taken for another pane's: its screen could not be read, and
  stopping it closed the other session's terminal.
- **Error reports leave out what is not an error**: a project that is not a
  git repository, a folder gone from the tree, a download the network cut off,
  a chat whose agent process ended, a sound device that would not start. A
  refused command is reported by its code and message, not as
  `[object Object]`.

## 0.1.33 — 2026-10-02

### Settings

- **General is grouped**: the island and being told, this desktop, chats,
  other devices, updates and privacy. Searching Settings for *island*,
  *pause*, *voice*, *remote*, *shortcut* and the rest finds it.
- **Pause has a row**, with what it holds back and what it does not.
- **The island says how it is drawn on this desktop**: above every window, a
  layer at the top, or, on GNOME, a window wherever the desktop puts it.
- **Ask before stopping a terminal** can be turned back on after *Don't ask
  again*.
- **Remote says how to get in**, step by step, what *type* and *answer*
  allow, and where its log is kept.
- **Voice messages say where the microphone is** and how to install a
  whisper.
- An orchestrator's first screen says where its rules for sessions live.

### devpit.app

- **devpit lives at devpit.app.** Sign-in and error reports go there, and the
  install line is `curl -fsSL https://devpit.app/install.sh | sh`. Installed
  copies that still ask the old address keep working.
- **The installer checks each release's signature** by devpit's key, where
  `minisign` is installed, besides its checksum.

### Fixes

- **A session that finished no longer shows as working.** Claude Code's own
  background forks report a subagent ending after the turn has ended, and that
  put the session back to work on the island, its tab and its card.
- **The microphone and the island's sound work in the AppImage.** It carries
  the GStreamer plugins it records and plays with; the system's are newer than
  its own GStreamer and were refused. The `.deb` recommends them.
- **The tray offers a pause again when one ends on its own**, rather than
  *Resume* for a pause long over.

## 0.1.32 — 2026-10-02

### Remote

- **This machine, from your phone or another computer, over Tailscale.**
  Settings → General → Remote: the machine serves its own viewer to your
  tailnet. Terminals are watched, and typed into when you allow it. The
  board's cards move. Agents' questions are answered. Chats are read.
- **Terminals are not watched from Windows yet**: a remote viewer attaches
  through a grouped session, which psmux does not have.
- **Nothing listens beyond the machine and your tailnet.** The viewer
  answers on loopback, published by `tailscale serve` with HTTPS, or on the
  machine's tailnet address when the tailnet has no HTTPS. Never on every
  interface.
- **Two things to get in.** You must be the machine's owner in the tailnet,
  and the device must be paired with a code (or QR) shown on the machine's
  screen. The code is good once, for two minutes.
- **What each device may do is set on the machine.** Every device can watch.
  Typing and answering are allowed device by device. A device that only
  watches is attached read-only by tmux itself, and a phone never shrinks
  the terminal on the desk.
- The desk says *Watched by 1 device*, and a click drops it. A log keeps who
  connected and what they did, never what was on screen.

### Orchestrator

- **A session's window says where it stands.** It shows the branch, how far
  it is ahead or behind, and whether it runs in its own checkout. A folded
  *Changes* panel holds the files, the commits and the diff. Its name is
  renamed in place (click or F2), through the CLI's own `/rename`.
- **It proposes projects, and you add them in one click.** *"Add
  ~/Workspace/x to the ASC group"* becomes a card above the composer:
  *Yes*, *Edit* or *Drop*. The Boards panel adds a folder, names it, groups
  it and links it in one screen, and links a whole group at once.
- **It hears what its sessions did since its last message** (one finished,
  one is waiting, one ended) at the start of your next one. It is not woken
  to say it.
- **Standing rules for every session it starts.** Whatever its
  `context/preferences.md` keeps under *Rules for sessions* goes with every
  brief.

### The card follows the work

- **A card moves to the lane for work in progress when its work starts, and
  to the one for checking when it is finished.** The agent no longer has to
  remember. A lane says which role it plays from its menu.
- **A card says what its sessions changed in other repositories:** *Also
  changed 13 files in devpit-app*, with which are still uncommitted.

### Chats

- **Attach files with a button** (Ctrl/Cmd+Shift+A), from the system's picker.
- **Voice messages.** Click or hold the microphone. The words land in the
  composer to read and edit before sending. They come from a whisper on this
  machine (whisper.cpp or `whisper`), or from a transcription service only if
  you choose one and keep its key. Without an engine, the recording goes as
  a file.

### The island

- **It shows the question a session asks, and answers it.** Its choices
  become buttons, pressed in that terminal as you.
- **Answer in words.** Reply to a session from the island without opening
  its terminal.
- **A question from another session waits as a badge**, rather than taking
  the island from the one you are reading. A thin bar counts down the last
  seconds before it folds.
- **On Wayland too.** It is a layer at the top of the screen on KDE,
  Hyprland and Sway (with gtk-layer-shell), and a window that never takes
  focus on GNOME.
- It costs nothing while hidden: no timer, no frame, no sound context.

### Desktop

- **One devpit at a time.** Opening it again brings the open one forward.
- **In the tray**, with *Pause for an hour*.
- **Open at login**, and **a shortcut that brings devpit forward** from
  anywhere, both in Settings → General.
- **Pause** notifications, the island and its questions, for a while or until
  you resume. Everything is still heard, so it is right when you come back.

### Other agents

- **Any agent can report to devpit.** `devpit-agent hook --agent <name>`
  says what it is doing, so it shows on the island and on its card like
  Claude Code (see `docs/agents.md`).
- **Gemini CLI reports from devpit's terminals with nothing to set up.**
  Your own `~/.gemini/settings.json` is never written, and your own hooks
  still run.
- **The groundwork for changing an agent's own settings file safely**: a
  diff first, a backup, and no write over a file that changed in between.
  Nothing uses it yet.

### Fixes

- A PDF opens in its tab, page by page, rather than as a file too long to read.
- A big tool report, such as a Read of a large file, reaches devpit cut down
  to what it reads, rather than being refused and leaving its step running.

## 0.1.31 — 2026-10-01

### Reminders

- **Ask to be reminded, and be told at that time.** A card's date can carry
  a time now; a day alone reminds that morning at nine. When it comes, a
  banner under the top bar says so — it has no close, only *15 min*, *1 hour*,
  *Tomorrow* (at nine) and *Done* — with a system
  notification when devpit is behind, and a line in the bell. A focus does
  not hold it back: the time was your own choice.
- **The orchestrator, or any session, can set one** when you ask —
  *"remind me tomorrow at three to review the PR"* — on a card with that
  date and time. devpit sets it off, not the agent, and a reminder only
  tells: it never starts anything.
- One thread waits for the next date and wakes only when a date changes.
  Switch it off in Settings → General → Reminders.

### Orchestrator

- **It drafts what you would say to a session, and you send it as yours.**
  A "go on" or a change of plan said in its chat waits, editable, above the
  composer and in the Sessions panel; *Send as you* types it into that
  session's terminal as your own words. Nothing is sent without your click.
- **A session it starts in a new folder shows the trust question** in
  *Waiting on you* before the session is even up, and is answered there.
- **Remote Control from the Sessions panel**, for a session in one of
  devpit's terminals.
- **What a session is on** — *Edit invoice.ts*, *Run npm test* — shows in
  the Sessions panel and on a card's sessions, not only on the island.
- **Every orchestrator reads the brief of the build that runs it**, however
  it is opened, and the brief says which build wrote it.

### Updates

- **The Mac updates itself.** The app in Applications was taken for a build
  nobody installs over, and was never offered an install.
- **Update in Settings shows the update it starts**: it follows the download
  and brings the update card forward, where it used to say "Working…" and
  then nothing.
- **Which devpit this is**, at the left of the status strip — with a dot when
  a newer one is on its way. A click copies it for a bug report.

### Fixes

- The island shows what is open in devpit: closing a tab takes its session
  off at once, a session idle in an open tab stays, and a terminal shows one
  session rather than every one it ever ran.
- A picture between 2 and 8 MB — a full-screen screenshot — opens in its tab
  rather than being refused as a file too long to read.

## 0.1.30 — 2026-10-01

### The island

- **Every agent at a glance, above your other windows.** A small capsule at
  the top of the screen shows what each session in devpit's terminals and
  chats is doing — the step it is on, like *Edit invoice.ts* or *Run npm
  test* — and opens all the way when one waits on you. Hover or click it for
  every session; open one for its steps and a preview of the change, the
  file or the command. It folds itself away when you move on.
- **Answer from it.** A terminal session's permission question can be
  allowed, denied, allowed always (the rule the agent suggested) or handed
  back to the terminal; unanswered, the terminal asks as it always did, and
  coming back to devpit hands it back too. A chat's question is answered
  there as well.
- **Get to the session.** *Open terminal* and *Open chat* bring devpit to the
  front on it. Drop a file on a session and its path goes into the agent's
  prompt. A session's pull request and the state of its checks show beside
  it.
- **devpit's own face**, in a picture per mood, a few sounds of its own you
  can switch off, and a system notification when a session waits, fails or
  finishes while devpit is behind — held, like the bell, by a focus.
- **Drag it** to the screen you work on; it stays there.
- On by default; switch it off in Settings → General. It sees only what runs
  inside devpit. On Linux it needs X11 or XWayland, which the AppImage uses;
  on a Wayland session without it you get the notifications alone.

### Orchestrator

- **It reads a session's replies**, a project's repository state and keeps
  its own log through devpit, rather than through scripts of its own.
- **Its links and account are kept by devpit**, out of reach of its own chat.
- **Its session tools reach only the projects linked to it**, never type
  control characters into a terminal, give every session a name of its own,
  and wait long enough for a session to start — so a slow start no longer
  ends in two of the same session.
- A message queued while it works goes in as soon as it has answered, not
  once every background task it started has finished.

### Chat

- **Allow always, for the rest of the conversation**: every file edit, the
  exact command, or the same tool — kept for that chat only.

### Fixes

- A card checked out again after its worktree was removed goes back on its
  branch, diffing from where its work began.
- A reply to a session is never typed into a terminal whose shell is in front.

## 0.1.29 — 2026-09-30

### Windows

- **Commands typed in a terminal run.** The line was cleared with keys
  PowerShell does not bind, and they reached the command as invisible
  characters — `ls` and `claude` read as not found.
- **A new terminal opens in its project's folder**, not your home — so an agent
  started there no longer asks to trust your home folder.
- **The chat finds your Claude Code profiles.** Programs are found under their
  `.exe` or `.cmd`, on the PATH Windows keeps for you now.

### Board

- **An agent moves its card with the work**: into the column for work in
  progress when it starts, and on to the one for checking when it finishes —
  by your board's own column names, never into a column that runs a step.

### Releases

- **Each release says what changed**, from this changelog: on the releases
  page and in the update card's notes.

## 0.1.28 — 2026-09-30

### Windows

- **devpit installs on Windows.** A per-user installer on the releases page,
  with psmux — the tmux devpit's terminals run on there — inside it. It adds
  devpit to the Start menu and removes cleanly from Settings → Apps. It is not
  signed with a code-signing certificate, so SmartScreen asks the first time.
- **It updates itself** like the Linux and macOS builds. On Windows an update
  closes the open terminals; the card says so before you choose.
- Terminals run PowerShell with devpit's prompt marks, find what a new Windows
  Terminal tab finds, and nothing devpit starts flashes a console window.
- Closing the last terminal ends the terminal server, and installing over
  devpit or removing it ends it first.
- The agents offered are the ones actually installed.

### Sessions

- A background session can be watched in a terminal, and every session devpit
  starts runs in one you can see (0.1.26).

## 0.1.27 — 2026-09-29

### Windows

- **devpit runs on Windows inside WSL.** The README says how: WSL 2 with
  WSLg, and the same install line as on Linux.
- **On the way to a native build.** The code now builds for Windows, and its
  terminals' layer is tested on every push against psmux, a tmux of Windows'
  own. Nothing is shipped for Windows yet.

### Fixes

- The search lists an agent by its command, without the variables in front of
  it — no more account directories or paths in the list.
- On macOS, a chat that hangs is ended after its grace period, instead of
  being taken for already gone.

## 0.1.26 — 2026-09-29

### Sessions

- **Every session devpit starts runs in a terminal you can watch.** A card's
  work handed on by an orchestrator, and a lane's `session` step, open in the
  card's terminal tab with your profile's own command — the same mode your own
  sessions run in. They used to start in the background, where nothing showed
  them and a permission they asked for waited on a question nobody could see.
- **A background session can be watched.** In the Sessions panel, "Watch it in
  a terminal" attaches it in a tab of its project.

### Fixes

- A program devpit starts — an agent, a chat, the MCP check — finds what your
  own shell finds, even when devpit was opened from the desktop's menu. MCP
  servers installed in a folder your shell adds to `PATH` no longer read as
  failed.
- The MCP list shows the servers that answer apart from the ones that do not,
  and refreshes from its header.

## 0.1.25 — 2026-09-29

### Git

- **Fetch, pull, push and sync** from the Changes panel, with how far ahead
  and behind the branch is. Pull only fast-forwards: a branch that went two
  ways is left for a terminal, never half-merged by a button.
- **The panels follow a commit made anywhere** — in a terminal, by an agent —
  instead of waiting for the window to lose and regain focus.
- **The branch menu scrolls**, and filters once there are more than a few.
- **A deleted file shows its diff** again.

### Terminals

- **A link in a terminal opens in the browser.**
- **A split stays where its pane was** — in a card's worktree, with its branch
  shown at once.
- **Stopping a terminal ends everything it started**, background jobs
  included; removing a project ends its terminals.
- **Drop files** on a terminal to type their paths, on the file tree to copy
  them in, or on Artifacts to keep them.
- **A new project offers its agents** on the empty screen.

### Orchestrator

- **A stopped session stays stopped.** A background session is stopped by its
  own CLI, which would otherwise start it again, and every session of that
  name goes with it.
- **The chat keeps its turns apart**: a turn another session woke and the
  person's own no longer mix, land out of order or stop each other, and a chat
  with Remote Control on is no longer closed by another.
- **Orchestrators stay where they are** in the rail when you pick one.

### The window

- **The right panel reads as one**: the tab in front says its name, Sessions
  counts what is waiting on you, and every view has the same heading and the
  same empty state. Artifacts can be added by hand.
- **A notice goes to the terminal** the agent is waiting in.
- **The status strip shows the MCP servers** — how many answer, and why the
  others do not.
- **A group of projects can show only the active ones.**
- **A new worktree can be set up for you**: files copied or linked from the
  main checkout, commands run, variables set — in Edit project.
- **An update shows as soon as it is found**, and devpit asks every hour.
- Less redrawing: the window no longer re-renders itself for every agent
  report.

### Fixes

- "database is locked" with many terminals open.
- The board's Add card no longer sits under a long lane.
- A card's branch keeps its accented letters (`producao`, not `produ-o`).
- Sessions under a path with `_` are found, and a profile switched off leaves
  every menu that offers one (thanks, @aronpc).

## 0.1.24 — 2026-09-26

### Orchestrator

- **Stop a session.** `devpit_stop_session` — and Stop in the Sessions panel —
  ends a session and closes the terminal it ran in, its tab too when it was the
  last pane. Only when you ask: work in flight is lost.
- **Start a session anywhere you linked.** Asked to, the orchestrator starts
  Claude Code in a new terminal tab of any linked project, in its folder — no
  card and no worktree needed.
- **New terminals and chats from the Sessions panel.** Every linked project is
  listed there, with or without a session, and opens a new terminal right over
  the chat or a new chat in the project.
- **A terminal you can size.** The terminal opened over the chat resizes from
  its edges and corners, stays centred and remembers its size; it is drawn as
  one surface in the terminal's colour. Hiding it keeps it running; "Close
  terminal" ends it.

### Performance

- Idle MCP App hosts are let go after ten minutes instead of staying until
  devpit quits.
- Listing sessions no longer asks git about every project every few seconds,
  and does nothing while the window is hidden.

## 0.1.23 — 2026-09-25

### Orchestrator

- **Sessions in one panel.** The right panel opens on Sessions: grouped by
  project, the ones waiting on you first, each with how long it has been at
  it and what it was last asked. A session opens to its question, a reply
  typed into its terminal as you, and everything that passed between it and
  the orchestrator; sessions no longer running stay below with their history.
  The chat's corner counts them and opens the panel.
- **A session's terminal, right here.** Open any session in a devpit terminal
  over the orchestrator's chat and work in it as you — answer what it asks,
  type a command — without leaving; its tab takes it back when you close it.
  "Go there" now opens its very tab.
- **Questions above the composer.** A session stopped on a question shows
  right above where you type, with its choices and a way into its terminal.
- **Who woke it.** A turn another session woke says which one, and quotes what
  it said.
- **Handed work as cards.** Each message the orchestrator sends shows as a
  card: to whom, what was asked, and how that session stands now.
- **Memory and tools, always.** The orchestrator's brief now has it search and
  save in whatever memory tools the account has, look facts up with the
  account's MCP servers, and keep a notice that adds nothing to one line.

### Chat

- **Remote Control connects visibly.** A chat whose process started with a
  message now shows its claude.ai page as soon as it arrives, instead of
  staying on "connects with the next message".

### Under the hood

- Tests no longer leave tmux servers running behind them.

## 0.1.22 — 2026-09-25

### Orchestrator

- **Projects are linked by hand.** An orchestrator no longer reaches every
  project. Link the ones it works with from its Boards panel: only those show
  there, only their folders are open to its chat, and only they are in reach
  of its tools. An orchestrator made before starts with none linked.
- **Only its own account's sessions.** The Sessions panel is back to the
  sessions of the account the orchestrator speaks as; 0.1.19 listed every
  account's.

## 0.1.21 — 2026-09-25

### Chat

- **Pages that come with MCP tools.** Tools that bring an interactive page —
  a Jira issue, a Confluence page, a Datadog chart — show it in the chat,
  under the answer that called them, as they do on claude.ai. Each page runs
  on its own origin, sandboxed, reaching only the hosts its server declared
  and nothing of devpit's. When a page wants to run one of its server's
  tools, you are asked first: once, or always for that page.

### devpit's own tools

- **The board, a card and the sessions as pages.** In any host that shows
  MCP Apps, devpit's board opens its cards, a card can be commented on, moved
  and handed to a new session (in its own checkout or the project's folder),
  and the sessions show who is busy, idle or waiting on you. A waiting
  question is still answered only in devpit.

## 0.1.20 — 2026-09-25

### Orchestrator

- **Every board beside the chat.** An orchestrator no longer has a board of
  its own. Its right panel opens on Boards: each project with its lanes and
  how many cards each holds, a lane opening to its cards, and a card opening
  on its project's board.

### Projects

- **Artifacts.** Files that belong to a project but not in its repository —
  specs, exports, reports — are kept in its own folder in the devpit
  workspace, safe from clones, new worktrees and cleans, and never committed.
  The right panel's Artifacts tab lists, opens and removes them; sessions
  keep and take them back with devpit's `devpit_artifact_*` tools.

## 0.1.19 — 2026-09-25

### Orchestrator

- **Waiting sessions are found again.** 0.1.18 did not recognise the
  terminals real Claude Code sessions run in, so "Waiting on you" and Reply
  never showed for them. They do now.
- **Sessions of every account.** The Sessions panel lists the Claude Code
  sessions of all your accounts, each marked with the account it runs under.
  Those of another account can be read and answered from the panel; only the
  orchestrator's own account can be messaged.
- **Notes, organised, without git.** An orchestrator reads every project's
  folder, keeps what it learns in `context/projects/`, your preferences in
  `context/preferences.md` and decisions in `decisions/`. Its folder is no
  longer a git repository, and the Changes and History tabs are gone for it.
- **Asks before starting a session.** Work goes to the session already
  running in a project; a new one is started only after asking, in the card's
  own checkout or the project's folder, as you choose.
- **Easier to add and tell apart.** A new orchestrator is one click away in
  the rail, and each one gets a colour of its own.

### Chat

- **Remote Control in any chat.** A project's chat can be reached from
  claude.ai and the Claude app too — outside the Supervised mode, which runs a
  process per turn.
- **Links you can click.** Web addresses and file paths in an answer are
  links, and web addresses in a terminal open in the browser. A file an answer
  names opens beside the chat, and expands into a tab.

## 0.1.18 — 2026-09-25

### Orchestrator

- **Answer a waiting session from the orchestrator.** A session in one of
  devpit's terminals that stops on a question — a permission, a choice Claude
  asked for — shows at the top of the Sessions panel beside the chat, under
  "Waiting on you", with its question and choices. A click answers it in that
  session's own terminal, as you; Esc dismisses it. Nothing is pressed unless
  the screen still shows exactly the question you clicked on, so a late or
  double click never answers the next one. The orchestrator can read the
  question and recommend a choice, but only you answer it.
- **Remote Control in the orchestrator's own chat.** The phone button on its
  chat now makes this same conversation — this chat, this process — reachable
  from claude.ai and the Claude app as `devpit-<name>`, and opens its page from
  the chat. What you write there arrives in this chat as it happens, marked as
  not from here. It replaces 0.1.17's terminal that resumed the conversation
  beside the chat. If the chat's process is not running yet, it connects with
  the next message; a restarted process connects again on its own.

## 0.1.17 — 2026-09-25

### Orchestrator

- **Continue an orchestrator remotely.** A phone button on its chat continues
  the same conversation in a terminal with Remote Control on, named
  `devpit-<name>` in claude.ai and the Claude app — hooks and devpit's tools
  included. Close the terminal and write in the chat to carry on at the desk.

### Fixes

- A choice in a dialog or in Providers is a form field whose list opens under
  it, instead of the chat's chip menu floating over the dialog's buttons.

## 0.1.16 — 2026-09-24

### Fixes

- **No one machine's commands are built in.** A second account used to be
  looked for by one person's command name; now an account is only ever a
  command its owner declares in Providers, and the process it runs is
  recognised as the CLI it is.
- **The default agent is one menu** in Providers, not a row of buttons that grew
  with every profile.
- **An orchestrator runs as the account you pick.** The picker drew no
  selection, so one could start as an account nobody meant. It is now a menu of
  the accounts configured in Providers, with a way to add another. The account
  lives in the orchestrator's own folder and **Runs as…** changes it later.
- **Orchestrators are a group of their own** — Orchestrators, at the top of the
  rail, folding like any group, with a quiet + — and edit like a project: name,
  icon and colour, with a mark of their own by default. The project picker no
  longer lists them.
- **An orchestrator opens with its history begun**: its folder is committed when
  it is made, and devpit's own files are kept out of it, instead of opening on
  a list of untracked files. *(Since 0.1.19 the folder is not a git
  repository.)*
- The project picker says "1 worktree", not "1 worktrees".

## 0.1.15 — 2026-09-24

### Orchestrator

- **Orchestrators**, at the top of the project rail. **New orchestrator** asks
  which account it runs as — by the command that starts it, a shell function
  included — and a name; an account can have several. Each gets a folder of
  its own under `~/.devpit/orchestrator/<account>/<name>/` — a git repository
  with a brief, `docs/`, `artifacts/` and `context/` — and a chat that opens on
  its last conversation. Right-click reveals its folder or removes it; the
  folder is kept. It is not listed among the projects. *(Since 0.1.16 the
  folder is `~/.devpit/orchestrator/<name>/`, and since 0.1.19 it is not a git
  repository.)*
- **An orchestrator keeps listening between your messages.** Its process
  stays, so a session's reply — or the notice it asked for when one went idle
  — wakes it, and what it says appears in its chat, marked as not in answer to
  you. Stop interrupts the turn and leaves it listening. It opens in Accept
  edits; in the supervised mode it cannot listen.
- **An orchestrator hands work to sessions.** Given a card, it starts a new
  Claude Code session of its account in the background, in the card's own
  checkout, linked to the card — on the board like any other — and named, so
  it can message it and hear when it is done. Only an orchestrator can, and
  only on a card.
- **Answer a session from the orchestrator.** A session in a devpit terminal
  that is waiting on you can be answered from the Sessions panel: what you
  type goes into its terminal as yours. A message from the orchestrator
  approves nothing there — that is Claude Code's rule, and it stays.
- **It sees every project and every session of its account.** Its agent can
  list the projects and their lanes, work on any project's board, and see the
  Claude Code sessions running now — by the name it messages them with, busy or
  idle, and the project and card each works in. The same list sits beside its
  chat. It still never moves a card into a lane that runs a step.
- **Its brief keeps up with devpit.** devpit's instructions live in
  `.devpit/orchestrator.md` and are rewritten when devpit updates; `CLAUDE.md`
  imports them and is yours alone.
- An agent started in a card's checkout reaches its own board again; the
  board's tools used to answer that no project contained it.

### Chat

- **A chat you leave keeps going.** Switch project or reload the window in the
  middle of an answer and, coming back, the answer so far is there and the
  rest keeps arriving — Stop included. It no longer says the app closed about
  a turn that was still running.
- **The mode you pick stays picked.** Full access, Accept edits or Supervised
  survives switching tabs, reopening the conversation and restarting devpit,
  and a new conversation starts in the last mode picked on that account.
- **Unsent text is kept** per conversation until you send or clear it.
- **The thread follows the answer as it grows** — text streaming into place,
  a block opening — and stops when you scroll up to read, with a button back
  to the latest.
- **A wider thread**: one 896px column for messages, the composer and replies.
- A relative link in a card's chat opens the file in the card's own checkout.
- **A chat has the board's tools**, as a terminal does, without the Claude
  Code plugin installed.
- **Sessions devpit starts hear the account's other sessions** whatever mode
  either runs in, instead of holding each message until approved by hand.

### Highlights

- **Error reports, opt-in.** A new switch in Settings → General, off unless you
  turn it on. With it on, devpit keeps its own errors — panics, internal
  errors of its commands, background routines that fail, and errors the
  window did not catch — and sends them anonymously so bugs can be found and
  fixed:
  - what is kept is cleaned as it is written: no paths, no credentials or
    tokens, no names quoted by git or the shell, no text from your cards,
    prompts or terminals;
  - a report carries only the error and devpit's version, OS, architecture and
    package type — no account, even when signed in, and no install id;
  - reports go out only while devpit sits idle (nothing running, nobody at the
    window for five minutes), five at a time, and pause when you come back;
  - Settings shows the exact request the next report would send;
  - turning the switch off deletes everything kept, at once. Kept errors never
    grow past 200 entries or 256 KB and are dropped after fifteen days, on the
    server too.
