# ADR 0002 — Hosts: tmux on the host, driven over the person's ssh

- Status: **accepted** 2026-11-19
- Date: 2026-11-19

## Context

The person wants a project's sessions on remote machines (dev, staging and
production) driven from the devpit on their own computer, the same way as
local ones, with the orchestrator knowing which machine and environment each
session is on.

devpit's terminals are tmux windows or devpit-ptyd panes on this computer.
Its agents report back through hooks and the `/agent` endpoint, both behind
one secret. Handing that secret to a host would open the whole agent API to
that host.

## Decision

- A host is a saved profile: a target that ssh resolves (an alias or a host
  name), with an optional user, port and identity file path, a work root and
  an environment. devpit stores no key and reads none.
- A terminal on a host is a tmux window on the host, on a server of devpit's
  own (`tmux -L devpit`), driven with `ssh … tmux …` over one ControlMaster
  connection per host. The window attaches with `ssh -t`, and reattaches when
  the connection drops. The session on the host lives on.
- Nothing of devpit's is installed or written on a host. A project's path on a
  host is set by the person. devpit never clones, creates or moves anything
  there.
- A session on a host reaches nothing in devpit: no MCP and no hooks. Its
  state is read from its screen. A later release may add hooks only, with a
  token per host that opens nothing else, and never on production.
- A session on a production host starts only when the start says so
  explicitly. Its tab is marked, and its brief carries the production rules.

## Consequences

- No hook events from a host: cards, the bell and notifications see a host
  session only through its screen until hooks arrive.
- Every value sent to a host is shell-quoted, because ssh joins its arguments
  into one command line.
- A host session's pid belongs to the host. Nothing on this computer signals
  it; it is stopped through tmux on the host.
- Removing a host profile touches nothing on the host.
