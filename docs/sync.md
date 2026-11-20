# Sync between machines

Two machines signed in to the same devpit account share preferences and
project settings through devpit.app, which keeps them sealed and cannot read
them. Off unless turned on in Settings → Account → Sync, on each machine.

Boards are separate from personal sync. The server can read an online board,
which the app reaches through devpit.app's board API and MCP. Personal sync
remains sealed on the same `/v1/sync` routes, and the server never opens those
blobs. A local SQLite board stays on its machine; an online board is not a
`board:` namespace in this sync.

## What each party sees

| | sees | cannot |
|---|---|---|
| devpit.app | an account's blobs: opaque names, sizes, versions, when each was written | open one, tell which holds what, or which project a name stands for |
| someone with devpit.app's database | the same | anything more |
| a machine of the account without the key | that the account syncs | open a blob, or start a second copy beside the account's |
| a machine with the key | everything the account syncs | — |

## The key

One key per account: 32 random bytes, made on the first machine that turns
sync on while the account syncs nothing yet. It is kept in devpit's home,
`sync-key.json`, at mode 600, and never sent to devpit.app.

Nothing seals with the key itself. HKDF-SHA256, with the key as its
pseudorandom key, draws three keys from it:

| purpose | what it does |
|---|---|
| `devpit-sync/1 seal` | seals each blob |
| `devpit-sync/1 key id` | eight bytes written on every blob, so a machine holding another key knows at once |
| `devpit-sync/1 names` | HMAC-SHA256 under it names each blob, so the name says nothing — not even which project a record belongs to |

A second machine gets the key one of two ways, never through devpit.app:

- **An offer through the relay.** On a machine with the key, *Pair another
  computer* goes online on devpit.app's relay under an id made for this one
  offer, with a Noise key made for it, and shows a line of text: that id, the
  key's public half, and a 32-byte one-time secret. The person copies it to
  the other machine, signed in to the same account, and pastes it under
  *Join*. That machine reaches the first through the relay as a device of the
  account — the relay joins nothing across accounts — and runs the Remote's
  handshake, `Noise_XX_25519_ChaChaPoly_BLAKE2s`
  (`docs/remote-relay.md`): it holds the other end to the key the offer
  names before saying anything, then sends the secret inside the session;
  the first machine compares it in constant time and answers with the sync
  key, inside the same session. An offer is good once, for two minutes, and
  is withdrawn after five wrong secrets. devpit.app carries frames it holds
  no key to.
- **The recovery code**, shown only when asked for: the key in groups of
  letters (no `0`/`O`, no `1`/`I`), with two bytes of a hash of it at the end,
  so a mistyped letter is caught before it is tried. It is the key: whoever
  holds it reads everything the account syncs.

A machine turning sync on when the account already syncs is never given a
new key: it waits to be joined, and says so. *Start over from this computer*
deletes the account's blobs (`DELETE /v1/sync`) and makes a new key; every
other machine then finds its blobs gone, stops, forgets its key and waits to
be joined again — it never writes a second copy beside the new one.

## A blob

Each namespace is one blob, sealed with XChaCha20-Poly1305 under the sealing
key and a nonce drawn at random for every seal:

```text
"dps1" | key id (8) | nonce (24) | ciphertext and tag
```

The header, the blob's name and the version devpit.app is to keep it as are
bound in as associated data. A blob served under another name, or as another
version than it was sealed for, does not open, and a machine refuses a
version older than one it has already read. What it holds is JSON with a
`format`; a machine that reads a later format than its own stops rather than
write over it.

## What syncs

| namespace | record | fields |
|---|---|---|
| `prefs` | `app` | the allowlist in `crates/sync/src/prefs.rs`: theme, the terminal's stop confirmation and contrast, rebound keys, focus mode, agent hooks, folder trust and the agents kept out of menus, the transcription language, Decisions' provider, model, address, daily cap and modes |
| `projects` | `origin:<hash>` | name, group, icon, colour, each plugin's state, whether Decisions may read it, and the origin's address without a password or token (read only from each machine's own git) |
| `machines` | the machine's id | its name, and to the hour when it last synced |
| `orchestrators` | `orch:<id minted from its folder under orchestrator/>` | its name, and each link the person made or undid as `link.<origin hash>` on or off; a repository nobody linked sends nothing, a link to a repository this machine lacks waits as *elsewhere* until it is cloned, and a link to a folder without an origin stays on its machine. The account it speaks as is a profile of each machine and stays there |
| `profiles` | `profile:<id minted when it was made>` | label, agent, program when it is one bare word, models, and every environment variable's name; a value only for `CLAUDE_CONFIG_DIR` (as `~/…`), a `*_BASE_URL` without credentials, a `*_MODEL*` name and `API_TIMEOUT_MS`, never for a name that reads as a secret. Arguments stay where they were typed. Every other value arrives empty and is asked for here; one filled here is never emptied from elsewhere. A profile that arrives runs nothing until the person takes it; a change elsewhere to its agent, program, base URL or config folder waits for their yes, one change at a time, as shown; a variable kept out stays out, and a value kept here (a config folder outside home, a program by path) stays. A profile or variable removed is a tombstone, except that a profile a step here still runs under stays on this machine, out of sync, still waiting for what it waited for. What sync keeps of each is `agents.profiles.synced` |

Everything else stays on its machine, and the allowlist says why for every
preference: paths, window and tray state, remote access, the calendar's
address, agent profiles (config folders and tokens), channels, which would
send a notice twice, consent to error reports, and sync itself. A preference
added later stays local until someone puts it on a list; a test fails until
they do. **The vault never syncs**: secrets are in each project's `.env` and
their names in its `secrets.json`, and no namespace reads either.

A project is the same project on two machines when it is a clone of the same
repository: its record is the hash of its normalised git origin, which devpit
already keeps (`project.origin_hash`). A project without an origin has
nothing another machine could match and stays local; so does a second clone
of one repository on the same machine, whose settings are its own — the
oldest clone carries the repository's. Where a project lives on each machine
never leaves it.

## A round

A machine keeps, per namespace, a mirror of everything it knows from every
machine and the version of devpit.app's copy it last read
(`sync-state.json`). A round:

1. lists the account's blobs and reads those whose version moved on;
2. stamps every field that differs from the mirror as changed here;
3. merges, field by field, the later stamp winning;
4. writes what won to this machine — preferences, project rows, plugin rows
   — checking each value again against the allowlist and the edit dialog's
   rules;
5. sends a namespace whose mirror holds more than devpit.app's copy, naming
   the version it read. Another machine writing first is a 409; the round
   reads again, up to three times.

A stamp is a hybrid logical clock: milliseconds, a counter and the machine
that wrote it. A change is placed by when it was made here (a preference's
`updated_at`), and is always after every stamp this machine had seen before
it, whatever the wall clock says; changes found in the same look are measured
from the same floor, so each keeps its own moment. Two changes at the same
moment go to the same machine everywhere. So concurrent edits resolve the
same way on every machine, in whatever order they sync. A machine joining —
or a project it adds that the others already have — takes what the others
settled before adding its own, so pairing a new computer does not overwrite
the account with its defaults.

The app runs a round a few seconds after a setting, a project's name or mark,
or a rebound key changes here, every three minutes otherwise, and on *Sync
now*; one at a time, on a thread of its own.
When values arrive, the window reads its settings, its projects and its keys
again (`sync:written`).

## devpit.app's side

`GET /v1/sync` lists the account's blobs; `GET`/`PUT /v1/sync/{name}` reads and
writes one, a write naming the version it replaces (`?version=`, 0 for a new
blob); `DELETE /v1/sync` removes them all. A blob is at most 1 MiB, an account
keeps at most 16 MiB in 256 blobs, both refused with 413, and an account
makes at most 600 sync requests in ten minutes. A machine refuses to send a
blob past 1 MiB before a byte leaves it, and reads no answer past its
ceiling. The table is devpit-app's migration `0010_sync`.

Synced files go to an S3 bucket instead, and devpit.app never carries their
bytes: `POST /v1/sync/objects/put` reserves room and signs a PUT that writes
exactly the sealed bytes named (length, SHA-256, `If-None-Match: *`), once;
`/landed` counts what S3 holds; `/get` signs reads of the account's own;
`/keep` takes one tree's live set and deletes what no tree named for a week.
Every key is under the caller's own account. The design and its limits are in
[`sync-workspace.md`](sync-workspace.md#objects-on-s3); the table is
devpit-app's migration `0013_sync_objects`.

## Where it is

| | |
|---|---|
| `crates/sync` | the key, sealing, the clock, the merge, a round, the namespaces, the client for devpit.app, and the offer through the relay |
| `crates/core/src/store/synced.rs`, `store/orchestrators.rs` | the rows and settings a namespace reads and writes |
| `apps/desktop/src/sync.rs`, `sync_pairing.rs` | the rounds' thread, the commands, offers and joining |
| `web/src/shell/SyncSettings.tsx` | Settings → Account → Sync |
| devpit-app `crates/api/src/routes/sync.rs`, `crates/db/src/sync.rs` | the blob store |
| devpit-app `crates/api/src/objects/`, `routes/objects.rs`, `crates/db/src/objects.rs` | synced files: the bucket (S3, and one in memory for tests) and the room |

No primitive is devpit's own: XChaCha20-Poly1305 and HKDF/HMAC-SHA256 are
RustCrypto's, the handshake is `snow`'s.

The tests that hold it, in `make test`:

| | |
|---|---|
| `a_preference_crosses_to_another_machine.rs` | two homes, a preference and a project's settings going both ways |
| `devpit_app_reads_nothing_it_keeps.rs` | every blob sent, captured and searched for the words that went in; opened with other keys; changed, rolled back, written beside |
| `what_belongs_to_one_machine_stays_there.rs` | machine-specific preferences and a project's vault files |
| `concurrent_edits_settle_the_same_everywhere.rs` | two machines changing the same things apart, synced in both orders |
| `an_orchestrator_follows_the_person.rs` | one orchestrator on two homes: links settle by repository, a joining machine keeps its own, clones, unlinks of a repository one machine lacks, the account staying where it was chosen |
| `a_profile_follows_without_its_secrets.rs` | a profile arrives on another home without its tokens and waits to be taken; a token filled there stays; a program by path, a config folder outside home and secret-looking values stay; a base URL, config folder or program changed or added elsewhere waits, taken or not yet; a kept-out variable is not asked again; a deleted profile or variable stays deleted and stops writing; a profile a step uses survives a removal from elsewhere |
| `one_repository_cloned_two_ways_settles.rs` | an https clone and an ssh clone of one repository settle on one address |
| `size_caps_are_kept.rs` | a preference past 64 KiB, a namespace past 1 MiB, the client's refusal |
| `pairing_tests.rs` | the key handed over a Noise session; a wrong secret; an impostor never told the secret |
| `apps/desktop/src/sync_tests.rs` | a secret kept through the vault's own write, absent from every blob |

The whole path — devpit-app's API over a Postgres, its relay carrying the key
from one home to the other, and every blob read back raw — runs with a
devpit-app checkout and a database it may write to:

```sh
DEVPIT_APP_DIR=../devpit-app DATABASE_URL=postgres://…/devpit_app_test \
  cargo test -p devpit-sync --test two_homes_through_devpit_app
```

## Not yet

- **The Local board.** The SQLite board is the next namespace, one per project
  (`board:<record>`, named through the key like the others), and the round
  already merges what it needs: a card a record, a field per thing a card
  holds, a cleared field as a tombstone. What is still to decide before it
  syncs is what makes it safe: matching columns that each machine seeded on
  its own, a card's place among positions written apart, a card moved into a
  lane with a step on the other machine (it must not run there), dates that
  would remind on both machines, and which of a card's edges and comments
  travel. Until those are settled and tested, the Local board stays on each
  machine.
- **A project's memory** arrives the same way: a namespace per project, its
  blob named through the key, and nothing else to build in the round or on
  devpit.app.
- **The rest of the workspace** — files and agent memory —
  is designed in [`sync-workspace.md`](sync-workspace.md)
  ([ADR 0001](adr/0001-sync-the-persons-workspace.md)); devpit.app's bucket
  for it is built, the machines do not send files yet. Orchestrators and
  agent profiles sync (above).
