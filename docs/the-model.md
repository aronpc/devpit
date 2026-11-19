# The model

Seven objects. If something is not one of these, it does not get in.

```
project ──┬── target terminal   (one, and only one)
          ├── board ── column ──┬── card ── run*
          │                     └── step?
          └── session*
```

## `project`

A git repository and its preferences. **The axis of parallelism is the
project**, not the branch — the hard part is not producing five versions of one
feature, it is not losing the thread when you switch projects and come back two
days later.

A project's terminals run on this computer, or on a **host**: a machine
reached over the person's own ssh, kept as a profile in Settings → Hosts with
its environment (dev, staging or production). A host is a preference, like an
agent profile, not an eighth object: it says where a terminal runs, not what
the work is.

## `terminal` — the target

One tmux *window* per project. The layout is drawn by the application, not by
tmux: a tmux split would show every leaf in a single client and fight the layout
the application persists.

Output is read through `pipe-pane`, which needs no attached client — recognition
has to work with the window closed, or a notification can never fire.

## `board`, `column`

A project's columns, in the order they appear.

**Columns are data, not code.** You create, rename, reorder and delete them, and
you decide whether a column runs a step and which kind. The default board of a
new project is a starting point, and nothing in the code may assume its column
names.

## `card`

The unit of work: title, markdown description, the column it sits in.

**The card is the trigger, not the record.** That distinction is what keeps it
from going stale: a board that merely describes what happens elsewhere is out of
date by the first busy day. If moving the card is what starts the work, it has
no way to lie.

A card can belong to an epic and wait on other cards. Those are edges between
cards, not a second kind of card: an epic counts its cards done by the lane they
sit in, and a card says what blocks it until that card is done. Where Devflow
is on, an agent cannot start a card that is still blocked; the person can still
move it. Its acceptance criteria are checkboxes under `## Acceptance criteria`
in its own description, so ticking one edits the text everyone reads.

## `step`

A column's rule: *on arrival here, run this*. A step is one of three kinds.

| | `agent` | `session` | `command` |
|---|---|---|---|
| Takes the target terminal | no | when you attach it | no |
| Good for | refine, review, verify | implementing | tests, builds, deploys |
| Returns | schema-validated JSON | a session you drive | streamed output and an exit code |
| Concurrent | several | several, one attached | several |

A `command` step receives its context **only through environment variables**,
never interpolated into the command string. A branch name containing a space or
a `;` becomes a variable's value rather than shell syntax, which removes an
entire class of injection.

Output streams while the command runs. A test suite that takes twenty minutes
shows its first line immediately, not twenty minutes from now.

## `run`

One execution of a step, and what it cost: the state it ended in, the output as
it came, the exit code of a command, the spend and duration of an agent turn,
the lane it was sent from, the folder it ran in and the session it ran as.

A card shows its runs. That is how "the agent is doing something" becomes a
sentence with numbers in it.

A run also records the circumstances it happened in — the command, the folder,
the revision, whether it had a checkout of its own — so a green row can answer
*green on which code*. Those are recorded before the command starts: a step
that commits would otherwise move `HEAD` out from under its own snapshot.

**Exit code zero is not a pass.** It says the command succeeded, which is a
different thing from the tests passing: a runner that matched no test files
exits zero and checked nothing. So a run carries three answers rather than one
— how the process ended, what the check said, and whether that is still about
the code in front of you — and a run that left nothing devpit can read reports
no result rather than a green tick.

What devpit reads: every file a command step writes into `$DEVPIT_REPORT_DIR`
— vitest or jest JSON (`--reporter=json --outputFile=…`), JUnit XML from pytest,
Node's runner, go-junit-report, Maven or Gradle — the JSON lines of
`go test -json`, and the `test result:` lines `cargo test` prints, or its JSON
lines on nightly. The folder is the run's own
and empty when it starts, so a report left by an earlier run is never read as
this one's. Zero tests passed is still no result.

A Devflow check set to `report: none` is the one exception: it says up front
that it writes nothing to read — a Makefile target, a linter — so its
exit code is its verdict: 0 passed, 127 not run (no such command), anything
else failed. A board step and a check set to write a report keep the rule.

**Secrets devpit handed out are taken out of a run's output before it is
stored**: a profile's environment, the account token, the hook secret. This is
not detection. A secret the command read from a file or fetched itself is one
devpit never saw, and it passes through.

## `session`

A background agent session, tied to a card and a worktree.

**The application does not own session state.** The agent CLI already answers
whether a session is busy or idle; keeping that in a database would create a
second truth that drifts from the first. The database stores only the link —
which card, which worktree.

What a session you drove yourself cost is not read back. The CLI writes a
transcript and devpit writes down where it is, but nothing reads the totals out
of it — so a card says what its *runs* cost, and is quiet about the rest. Being
quiet is the honest half of that; the other half is not written yet.

## What is not an object

Open file, tab, pane, window layout. That is interface state and it lives with
the interface. Promoting it to a domain object is how a workspace for agents
turns into an IDE — and an IDE is the weight this project is running from.
