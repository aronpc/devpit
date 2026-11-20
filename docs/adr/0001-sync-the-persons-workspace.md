# ADR 0001 — Sync the person's workspace, not their machine

- Status: **accepted** 2026-10-28, with every recommendation in
  [`docs/sync-workspace.md`](../sync-workspace.md#decisions) and file
  contents on S3
- Date: 2026-10-28
- Builds on: [`docs/sync.md`](../sync.md) (M-10, shipped in 0.1.41)

## Note — 2026-11-19

An online board is intentionally readable by the devpit.app server and travels
through its board API and MCP, not through the sync namespaces described by
this decision. Personal sync remains sealed with a key held only by the
person's machines, and the server never opens those blobs. A local SQLite board
stays on its machine.

## Context

Sync today carries three small namespaces through devpit.app, sealed with a
key devpit.app never sees: preferences, project settings matched by git
origin, and the machines. The person now wants to sit at a new computer,
pick an orchestrator, and carry on: its notes, its decisions, its projects,
its agent profiles, and as much of the agents' memory as can travel safely.

What we measured on the machine that will be the first source (2026-10-28):

| what | size | shape |
|---|---|---|
| one orchestrator folder (`jhol-space-s-1`) | 221 MB | 189 MB of it `artifacts/` (video, audio, images); notes (`context/`, `decisions/`, `docs/`) under 1 MB |
| one Claude config dir's `projects/` | 2.6 GB | transcripts keyed by the encoded absolute cwd; a single session up to 46 MB |
| the anchored memory DB | 5.2 GB | SQLite with embeddings; anchored has its own remote sync |
| sync's blob store | 1 MiB per blob, 16 MiB per account | — |

And in the code: an orchestrator's profile and projects live in a preference
keyed by its absolute folder path, with machine-local `prj_` ids as values;
project ids, card numbers and worktree paths are per machine; agent profile
env values hold tokens; "project memory" does not exist yet as a feature.

## Decision

1. **Three kinds of data, three ways to travel.**
   - *Records* (settings, profiles, orchestrator links, later the board) go
     through the existing round: one sealed blob per namespace, merged field
     by field by the hybrid logical clock.
   - *Files* (an orchestrator's notes, a project's memory, Claude Code's
     auto-memory) go through a new **file tree** namespace: a sealed manifest
     merged per file, and contents as sealed, content-addressed objects on
     devpit.app. Concurrent edits of one file keep both versions; nothing is
     merged silently inside a file except journals devpit itself appends to.
   - *Sessions* (agent transcripts, devpit's own conversation files) **do not
     sync**. A session continues on another machine through what it left in
     synced places: the card's handoff, the orchestrator's notes, the agent's
     memory. Carrying a transcript is a later, opt-in, per-session action, if
     ever.
2. **Identity across machines never uses a path or a local id.** A project is
   its normalised git origin (as today); an orchestrator and a profile get a
   sync id minted once; every link between them is written in those ids and
   resolved to local ones on each machine.
3. **Secrets and paths stay on the machine.** Profile env values sync only
   for names on an allowlist (config dirs as `~`-relative paths, base URLs,
   model names); every other value arrives empty and the new machine asks for
   it. The vault, devpit's token files and the sync key never sync, as today.
   Absolute paths are rewritten to `~` or dropped.
4. **devpit does not sync the anchored memory DB.** It is another tool's
   store, gigabytes in size, with its own remote. devpit's bootstrap points
   at anchored's remote instead.
5. **File contents live in S3, reached only through devpit.app.** A private
   bucket on the company's AWS account holds the sealed objects; devpit.app
   keeps the quota and hands out short-lived presigned URLs, so no machine
   ever holds an AWS credential and devpit.app never carries the bytes.
   Records and the current manifests stay in devpit.app's Postgres, where
   the version check (409) already lives.
6. **A new machine is bootstrapped, not cloned.** Pair, then a guided
   *Set up this computer*: pick the orchestrator, map each linked project to
   a local folder (found by origin, cloned, or skipped), fill each profile's
   missing secrets and sign its CLI in.

## Consequences

- devpit.app needs an object store with its own quota next to the blob
  store; the 1 MiB / 16 MiB caps stay for records. The bucket is
  `devpit-prod-sync-objects-377781325796` (us-east-1); devpit-app gets an
  IAM user scoped to its `v1/` prefix, whose key is made at deploy time.
- An orchestrator's link preference must move from a path key and local ids
  to a sync id and origin hashes, with a store migration.
- `origin_hash` normalisation must be fixed (userinfo, ports) and recomputed
  when a remote changes, because it becomes the only project identity that
  crosses machines.
- `artifacts/` files up to 5 MB sync, behind devpit Plus; larger ones and
  folders marked *keep local* stay on their machine.
- Work in progress on machine A cannot be resumed as the same agent session
  on machine B; it is continued from its handoff. That is a product promise
  the UI must state plainly.
- The board (#198) stays out of this ADR's first steps; it reuses the same
  identity rules and still has its own blockers (per-machine card numbers,
  steps that must not run twice).

## Alternatives considered

- **Git for the orchestrator folder** (a private remote per person). Rejected:
  needs a git host the person configures, is not end-to-end sealed without
  git-crypt, and current builds deliberately stop making the folder a repo.
- **Peer-to-peer (Syncthing-like) between the machines.** Rejected as the
  default: both machines must be on at once, and the relay already exists
  for pairing only. It stays an option for large artifacts.
- **Syncing whole CLAUDE_CONFIG_DIRs.** Rejected: gigabytes, absolute paths
  in every transcript, credentials and hooks with machine paths inside.
- **Cloudflare R2** instead of S3. Cheaper egress, but the company's AWS
  account is where its other buckets, billing and access already are; the
  object routes hide the provider, so moving later is a copy.
- **The client talking to S3 with its own credentials** (as the Remotely
  Save plugin for Obsidian does). Rejected: a credential on every machine,
  no quota, and no server to keep objects immutable.
- **Syncing the anchored DB through devpit.** Rejected: size, embeddings tied
  to a local model, and a tool that already has its own sync.
