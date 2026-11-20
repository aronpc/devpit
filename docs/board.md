# How the board works

```
      [inbox]       [refine]       [review]        [doing]        [check]        [ship]
         │              │              │              │              │              │
     you write       planner        critic        executor       verifier       your own
     the card        · opus         · opus        · sonnet       · sonnet        command
         —            agent          agent         session         agent         command
                        └──────────────┘                             └──────────────┘
                          no terminal                                  no terminal
                                                      ▲
                                           the one target terminal
```

**That is a board somebody set up, not the one you get.** A new project gets
those six columns with nothing behind them: every lane runs nothing until you
give it a step, which is one menu on the lane.

A column either does nothing, or runs one of three kinds of step:

| | `agent` | `session` | `command` |
|---|---|---|---|
| Takes the terminal? | no | **yes** — it is the target terminal | no |
| Good for | refine, review, verify | implementing | tests, builds, deploys, lint |
| Gives you back | schema-validated JSON, with cost and duration | a session you drive | streamed output and an exit code |
| How many at once | several | several &mdash; one of them attached | several |

The flow above is just the default board for a new project.

## A column composes the step

A step is not "call an agent". It is a recipe the column holds, and moving a
card resolves it into one invocation:

| | |
|---|---|
| `agent` | who does it — `architect`, `executor`, `reviewer` |
| `model` | which model that call runs on |
| `inject` | which of the card's context reaches it |
| `capUsd` | the ceiling it may spend |
| `expects` | the shape the answer has to satisfy |

Agents come from your machine — the ones you wrote in `~/.devpit/agents/`, and
the ones your tooling already installed. devpit **references them, never copies
them**: whatever owns a catalogue keeps owning it.

The point is scope. Twenty agents available everywhere end up loaded
everywhere, and you pay for all of it on every call. A column says *this* step
is that one agent, on that model, with that slice of the card — and that is all
that gets sent.

Skills are not on that list. The CLI offers a turn all of them or none, so a
step asks for one in its prompt — `/tdd` — the way you would in a session, and
a step that tries to declare them is refused when it is saved.

## An agent run keeps the account it started with

The account and model are fixed on the run before its process starts. Editing
or deleting the profile later does not rewrite that history, and the model the
CLI reports is kept separately from the one devpit requested.

Before starting, devpit checks only blockers it knows: a missing or disabled
profile, no saved sign-in, an exhausted Claude quota, or an unavailable model.
If one is present, the attempt is paused by default, with the reason recorded;
an unknown or merely low quota and an expired access token do not block it.

The pause offers every account that passes the same check. One click starts
the next numbered attempt under the chosen account. Automatic switching is an
opt-in per project, off by default; when enabled, it tries the first account
that can run. Retrying without a switch keeps the paused attempt's account and
model.

## A lane moves a card only as far as you let it

A step that finishes writes down what happened — output, exit code, real cost.
What happens next is the lane's setting, and you choose it per lane:

| | |
|---|---|
| `manual` | nothing moves. The card stays where it is until you drag it. |
| `ask` | devpit asks, naming the lane it would move the card to. |
| `auto` | it moves, and the next lane's step runs. |

`manual` is the default, and a lane set to `auto` says so on the board. There
is no orchestrator behind this: nothing schedules a failed or finished step
for another run, and a chain of `auto` lanes is one you built lane by lane and
can see.

## An orchestrator, when you want one

An orchestrator is a chat you drive that sees every project at once. Made from
the top of the rail — which Claude Code account it runs as, and a name — it
lists the projects and their boards, sees the account's sessions running now,
and can hand a card's work to a new session: in the card's own checkout, linked
to the card, on the board like any other. It hears those sessions between your
messages and says what they reported in its chat.

It is still you deciding. It never moves a card into a lane that runs a step,
nothing it does runs on a timer, and a message it sends approves nothing in
another session — when one waits on you, you answer it from the orchestrator's
Sessions panel, as yourself.

**No card, just a terminal.** Open devpit, type into it, and your agent CLI
behaves exactly as it always has. The board is an optional source of work, not a
toll gate.
