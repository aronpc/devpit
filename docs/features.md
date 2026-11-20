# What devpit is for

Agents are easy to start and hard to keep track of. Five terminals in, you no
longer know what is running, what it cost, or which one you were reading.

devpit exists to keep **every step of the work visible and under your hand** —
one terminal you are actually watching, a board that says where each piece of
work stands, and a number on every agent call.

## What it does

- **One target terminal per project.** Switching work switches what is attached
  to it. The previous session keeps running in the background; it just stops
  taking up the screen.
- **Sessions that outlive the window.** Close devpit, reopen it, and what was
  running is still running.
- **A git worktree per line of work**, made when a step needs a checkout and
  removed when you say so — never while it still holds uncommitted work.
- **A board per project.** Moving a card runs real work — in the terminal you
  already have open, not in a new one.
- **Your columns.** Rename them, reorder them, add your own, decide which one
  runs what.
- **Each column composes its own step** — which agent, on which model, with
  which slice of the card's context.
- **Every agent attempt keeps its account and model.** A known blocker pauses
  it before a process starts; switch accounts in one click, or opt a project
  in to the first account that can run.
- **A spending cap on every agent call**, and the real cost written on the card
  when it finishes.
- **Agents are files** — markdown with frontmatter in `~/.devpit/agents/`, and
  whatever your tooling already installed. None ship with devpit: the ones you
  see are the ones on your machine. Nothing to recompile, nothing to register.
- **A browser pane**, for the page the project is serving. It opens
  `localhost` over http because that is what a dev server answers, keeps each
  pane's session apart, and can bring a signed-in session over from Chrome,
  Firefox or Safari — per browser and per domain, never on its own. Letting an
  agent drive a page you give it is not built yet.
- **File tree and diffs** beside the terminal, so reviewing what an agent did
  does not mean leaving.
- **Terminals on your other machines.** Link a project to a folder on a
  host you reach with ssh — dev, staging or production — and open a terminal
  there that comes back by itself when the connection drops. The tab says
  which host and environment, production in red; an orchestrator can start a
  session there, on production only with your yes.
- **An online board.** Its board, columns, cards (title, description and plan),
  comments, acceptance criteria, links, history with its author and date, and
  attachments live on devpit.app, where the server can read them. The app
  reaches it through the board API and MCP. Attachments have limits per file,
  per account and across the service.
- **Personal sync stays sealed.** Preferences and project settings are sealed
  with a key held only by your machines; devpit.app never opens those blobs
  (`docs/sync.md`).
- **On your machine.** A local SQLite board, projects, conversations and
  terminal history never leave this computer. An account is optional.
