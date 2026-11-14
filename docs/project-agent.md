# The project agent

A project agent is one Claude Code session per project that holds the
project's context: its architecture, rules, decisions, pitfalls and history.
The orchestrator asks it about the project instead of reading the project
itself, and has it write the brief a card's session starts with instead of
writing that brief by hand.

It answers and briefs. It does not edit files, move cards or start sessions,
and it stays in its own project. The project's own sessions consult it too,
while they work.

## How the orchestrator reaches it

Two MCP tools, offered to devpit's orchestrator only:

| Tool | Takes | Answers |
|---|---|---|
| `devpit_project_agent_ask` | `project`, `question` | `{ project, answer, woke }` |
| `devpit_project_agent_brief` | `project`, `cardId` | `{ project, cardId, brief, woke }` |

`woke` says whether this call started the agent's process. Both answer
whole, never cut to fit: a brief is passed whole as a session's prompt, and a
cut answer would be asked for again, running the agent's whole turn again.

Neither call starts a card session, opens a terminal or touches a checkout.
The brief is text for the orchestrator to pass as `devpit_start_session`'s
`prompt`. The card's own text goes to the agent marked as data, not as
instructions.

## How a project's sessions reach it

A session working in a project, on a card or in the project's folder, has
two MCP tools of its own, apart from the orchestrator's and the assistant's:

| Tool | Takes | Answers |
|---|---|---|
| `devpit_project_agent_consult` | `question` | `{ project, answer, woke, left }` |
| `devpit_project_agent_propose` | `kind` (`fact`, `lesson` or `decision`), `text`, `cardId` | `{ project, proposed, source, waiting }` |

Neither takes a `project`: a session reaches its own project's agent, the
same process the orchestrator asks, and no other. Both are refused to an
orchestrator, which has its own pair, and in a project whose agent was never
asked, which has none yet. The agent is woken under the profile of an
orchestrator linked to the project, as for a closed card.

- **A budget of questions.** A session, known by the pane it runs in (else
  by where it stands), asks at most ten questions of its project's agent;
  `left` says how many remain, and one past the budget is refused without
  reaching the agent. A question the agent could not answer is not counted.
  The answer comes back whole, never cut to fit.
- **Proposals, never writes.** What a session wants remembered goes to
  `proposed.md`, with the card it came from (`cardId`, or the card whose
  checkout the session stands in) as its source, and waits for Approve, Edit
  or Discard like any other proposal. The memory files change only once the
  person approves it. Each is one line, and at most fifty wait at once.
- **Briefed to ask first.** A session the orchestrator starts in a project
  with an agent finds a short section after its task, under
  `## The project agent`: consult the agent before exploring broadly or
  choosing a pattern its memory may settle, and propose rather than edit
  `.devpit/project-agent/`. A brief for a project without an agent goes as it
  was.

## How the person reaches it

Each project's sidebar has **Ask the project**, a chat with its agent: the
same resident process the orchestrator asks, woken under the profile of an
orchestrator linked to the project, read-only like every wake. Its first
question writes the memory, as `devpit_project_agent_ask` does, and after an
idle-out the next question resumes the same conversation. While Labs has the
project agent off, the entry is hidden and the chat is refused, as its tools are.

- **Answers name their files.** Each question asks the agent to name the
  files its answer rests on, and the files of the project an answer names
  show under it, each opening in a tab.
- **Save as fact.** An answer that names files can be proposed as a fact,
  edited first into one line, resting on those files. It waits in
  `proposed.md` for Approve, Edit or Discard like any other proposal, and
  `facts.md` changes only once the person approves it.
- **Each question gets its own answer.** The pane, the orchestrator and the
  sessions ask the same process, and questions wait their turn. A question
  that fails, or that the agent does not answer within fifteen minutes, says
  so in the pane instead of reading forever; the process is then let go and
  the next question starts a new one.

Beside it, **Project agent** is the agent's page: Facts, Lessons, Decisions
and Proposed, each entry with its day and its sources, a file opening in a
tab. A proposal is approved, edited or discarded there as under the top bar.
**Approve all from onboarding** approves at once what the agent's own read
proposed, since that rests only on the repository's files, and **Re-learn**
has the agent read the project again after big changes.

## It learns the project on its own

With Labs on, a project linked to an orchestrator gets its agent without
anyone asking: on the app's next start, or as soon as it is linked. Projects
are read one at a time in the background, and never while their agent is
answering someone. Settings → Maintenance → Labs lists the linked projects,
each with its own switch: a project left out gets no agent, and its tools,
pane and page are refused as with Labs off.

On that first read the agent reads the repository the way it reads it to
answer: its guides, ADRs, build and test commands, layout and conventions.
It answers 15 to 30 facts and decisions, each one line resting on the files
that show it. A line whose files are not files of the project is left out, as
is one the memory already says. Each waits in `proposed.md` with the source
`onboarding, <files>`; once approved, the entry keeps its files and the day,
not the mark. `learned` keeps the day it last read the project, so a project
is read on its own once. Re-learn reads it again, and its proposals take the
place of the earlier read's still waiting.

## The process

The agent is a Claude Code process started in the project's folder, under
the profile of the orchestrator that woke it, with devpit's MCP tools. It
runs in the CLI's plan mode whatever the profile's own settings choose, so it
reads and never writes. Plan mode keeps the profile's Bash allow rules, so the
process is also started with deny rules, which win over any allow rule:
`Edit`, `Write`, `NotebookEdit`, Bash's `rm`, `mv`, `git commit`,
`git push` and any `>` redirect, the sessions' own
`devpit_project_agent_consult` and `devpit_project_agent_propose`, and
`devpit_session_message` and `devpit_session_notify_idle`: it answers, and
never starts a turn in another session. It loads only the person's own
settings (`--setting-sources user`), never a repository's: the agent also
starts with nobody asking, and a repository's allow rules would widen what
plan mode lets it run. What the repository still owes it, devpit hands over
itself, read from the project's root through its links, each file capped:

- **Its deny and ask rules.** `permissions.deny` and `permissions.ask` from
  `.claude/settings.json` and `.claude/settings.local.json` join the deny
  rules above: nobody answers an ask in the agent's process, so it refuses
  as a deny, and a file the repository fenced off, such as `.env`, stays
  unreadable. A settings file longer than 256 KiB or not JSON adds none, nor
  does a rule with a control character. Its allow rules never apply.
- **Its instructions.** The agent's first message carries `CLAUDE.md`,
  `.claude/CLAUDE.md` and `AGENTS.md`, each once (a link to another goes
  once) and up to 32 KiB, marked as the project's instructions, beside the
  memory summary. An `@path` import inside them is not followed.

- **Wakes when called.** The first question starts the process. Further
  questions go to the same process, one at a time. Work an answer leaves
  running in the background ends in a turn of its own, and that turn is still
  the same answer: the next question waits for it rather than receive it.
- **Idles out.** Ten minutes after the last question with nothing new, the
  process is closed.
- **Resumes with its memory.** The next question starts a new process that
  resumes the same conversation (`--resume`), and its first message carries
  the memory summary again, read afresh from the files.

## Memory

Kept in the project's folder, in `.devpit/project-agent/`:

| File | Holds |
|---|---|
| `facts.md` | what is true of the project: guides to read, how things are built and run |
| `lessons.md` | what went wrong before and how to avoid it |
| `decisions.md` | what was decided and why, such as the project's ADRs |

Each entry is one bullet with its date and its source:

```markdown
- 2026-11-13 · AGENTS.md · A guide to read first: "Working in this repository".
- 2026-11-13 · docs/adr/0001-sync-the-persons-workspace.md · 1. Sync the person's workspace
```

The source lists, comma-separated, the files the entry rests on, the cards
it came from (`#12`) and pull requests (`PR #3`):
`- 2026-11-14 · #12, docs/adr/0002-one-queue.md · Proposals share one queue.`
An entry written before sources listed files still rests on its source when
that is a file. A bullet without a date as `YYYY-MM-DD` or without a source is
not an entry: it is left out of what the agent is handed, and counted as left
out.

The files are written on the agent's first wake, from the guides the project
already has (`AGENTS.md`, `CLAUDE.md`, `README.md`, `docs/architecture.md`)
and its ADRs in `docs/adr/`. After that, devpit changes them only as the
person approves a proposal.

The agent is woken with a summary: each file's newest entries, at most twenty
each, and how many more there are. It reads the files themselves when it
needs the rest.

## Re-checked when their files change

`checked` keeps the default branch's commit the memory was last re-checked
against. Each time the agent wakes, and only then, devpit flags the entries
of the three files that rest on a file the default branch changed since that
commit. When any is flagged, the wake's first turn asks the agent to re-check
each against its files as they are now and answer `keep`, `remove` or
`update: <new text>`; the question follows in the next turn. Each stale entry
becomes a proposal in `proposed.md` that names the entry it replaces, and its
file is left as it is until the person decides. The commit is then noted as
checked. No timer re-checks: an agent that idles out waits for its next
question. The first wake of a memory kept from before only notes the commit.

## Lessons and decisions from closed cards

When a card lands in the done column, devpit:

1. Logs one line to `metrics.jsonl`: the day, the card, its review rounds
   (the runs its `[devflow] review:` comments name, each once) and the
   findings those rounds found again (`repeats` in each round's
   `findings.json` still kept), so the lessons' effect can be measured.
2. Asks the project agent, under the profile of an orchestrator linked to the
   project, what the card teaches. It reads the card's comments (its handoff,
   its review and gate results, the person's corrections) and its last
   review's findings, as data, and answers zero to three lessons; a choice
   the card settled that later work should follow comes as a decision.
3. Keeps them in `proposed.md`, each dated and with the card as its source.

Each proposal, a re-check's included, waits under the top bar and on its
card, with Approve, Edit and Discard. Approved, as it is or edited, a lesson
is appended to `lessons.md` and a decision to `decisions.md`, with the card
and the day; a re-check's takes the stale entry's place, or removes it.
Discarded, it is removed and nothing keeps it.

A decision names the cards that made it in its source, and a card shows the
decisions that name it, each with its cards.

The newest twenty approved lessons go into every brief the agent writes and
into every Devflow review of the project, as items of the checklist the change
is held to.

## Projects without an agent

A project gets `.devpit/project-agent/` when it is first read or asked: on its
own once linked to an orchestrator with Labs on. A closed card is measured and
its lessons asked for only in a project that has that folder, so a project
left out, or linked to no orchestrator, works exactly as it did, and every
other tool keeps its shape. A memory kept before onboarding existed is read as
it is: its files are not rewritten, and the read only adds proposals.
