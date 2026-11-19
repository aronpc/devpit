# Working in this repository

Instructions for any coding agent or contributor. `CLAUDE.md` points here on
purpose: two copies of a rule are two copies that drift.

Read [`docs/the-model.md`](docs/the-model.md) for the domain and
[`docs/architecture.md`](docs/architecture.md) for the boundaries before making
a structural change.

## Language

**Everything versioned is in English** — code, comments, test names, commit
messages, docs, workflows. English is the base and the default: every key is
born in the English catalog, and the other languages live beside it.

The exception is the pt-BR translation catalogs, and only them:
`web/src/i18n/pt-BR/`, `crates/core/src/i18n/pt-BR.json`, the `*_pt_br.md` and
`*_pt_br.rs` agent-text variants, `docs/pt-BR/` and `README.pt-BR.md`.

`.project/` is gitignored — not versioned at all, so outside this rule.

## Commands

```sh
make test     # guards, Rust tests, frontend tests — run this before saying done
make fmt      # rustfmt + prettier; run it after any Rust edit
make dev      # the app, hot reload, on a home of its own (see below)
make build    # release bundle
```

### Working on devpit from inside devpit

A development build and the installed devpit run side by side and **share
nothing of devpit's**. Keep it that way:

- **A debug build keeps `~/.devpit-dev`; a release build keeps `~/.devpit`.**
  The split is by build profile (`devpit_core::ROOT_NAME`), not by a variable,
  so `make dev` cannot open the installed devpit's projects. The store, the
  tmux socket and the hook endpoint all live under that root. Two devpits on
  one root write their listener ports over each other's, and one of them stops
  hearing its agents without a word.
- **`make dev` gives the window its own identifier** (`tauri.dev.conf.json`),
  which is what keeps WebKit's localStorage apart.
- **HOME is shared on purpose.** Agent credentials, git and gh are the same as
  the installed devpit's, so agents launched from the dev build work normally.
- **A dev build says so**: a `dev` badge beside the mark and `devpit (dev)` as
  the window title.
- **`make test` and `make e2e` are always safe** to run from a devpit terminal:
  both use a home under `target/` and the e2e refuses the real one.
- Never run the dev build with `DEVPIT_HOME=~/.devpit`. `DEVPIT_HOME` exists for
  a home somewhere else entirely, and pointing it at the installed one undoes
  the whole separation.

`help setup dev build test fmt clean`, `e2e` for the WebDriver suite, `shots`
for the launch screenshots and clips (`e2e/demo/README.md`), and
for Windows `check-windows`, `test-tmux`, `windows-bin` (the pinned psmux) and
`bundle-windows` (the installer) — what the release's Windows leg runs. The
list is short because the Makefile is the interface everyone reads: a target
earns its place by being something a person runs by hand, and a one-off belongs
in the shell. The other half of the same rule is that **CI never invents a
command nobody can run locally** — every job calls one of these.

CI runs `setup`, `test` and `build`; the end-to-end job calls `e2e`.

## Guards that fail the build

`cargo xtask check` enforces fifteen rules. Each reports file and line,
because "the guard failed" sends someone looking and a location sends someone
fixing.

| Guard | Rule |
|---|---|
| `core_does_not_know_the_shell` | nothing under `crates/` imports `apps/desktop` |
| `only_one_crate_drives_the_agent` | agent CLI invocation lives in `crates/agentcli`, nowhere else |
| `a_command_has_a_caller` | every command in `contract_list.rs` and `handler.rs` is called from `web/src`, or listed with its reason in `xtask/uncalled-commands.txt` |
| `nothing_is_named_after_nothing` | no enum variant, command or type that nothing draws or calls |
| `platform_window_matches_the_base` | the per-platform Tauri config cannot drift from the base |
| `files_only_get_shorter` | every file listed in `xtask/ceilings.txt` stays under its line count |
| `a_control_either_works_or_goes` | no `<button>` in `web/src` without a handler |
| `paths_come_from_home` | a path under devpit's `projects/` is built only in `crates/core/src/home.rs`; the joins onto an agent CLI's own `projects/` are counted in `xtask/cli-roots.txt` |
| `the_csp_forbids_what_the_app_never_needs` | the window's policy allows no remote script and no remote frame |
| `the_version_has_one_source` | the version in the Cargo manifests, the Tauri config and the web package agree |
| `the_bundle_says_what_it_ships` | the release bundle declares the targets and the updater artifacts it is built with |
| `the_release_workflow_keeps_its_promises` | the tag-only release workflow keeps its pinned actions, its tag check and the key in one step |
| `the_app_never_kills_the_tmux_server` | nothing in `apps/desktop` names `kill_server`, because the terminals outlive the window |
| `every_catalog_keeps_up_with_english` | every key of `en/<area>.json` reaches `pt-BR/<area>.json` — translated, or listed with its reason in `untranslated.txt` (twenty keys at most); no empty value, no `{placeholder}` one side lacks, no plural without `.one` and `.other`, no key only pt-BR carries — over `web/src/i18n/` and `crates/core/src/i18n/` |
| `uncatalogued_screen_text_never_grows` | fixed text on the screen — a JSX text child that says words, a `title`/`aria-label`/`placeholder`/`alt`/`label` literal, the first argument of `toast(`/`notify(`/`confirm(` — is counted per file over `web/src/{shell,island,remote}` and only ever goes down (`xtask/ui_text_baseline.txt`); a file not listed is allowed none at all |

The ratchet only tightens: a file over its ceiling fails, and `cargo xtask
ceilings` refuses to raise one. A ceiling *above* what a file needs does not
fail — it used to, and a file shrinking by one line broke the build until
somebody regenerated the list. Regenerate after a file genuinely shrinks.

**A control either works or leaves the screen.** `xtask/dead-controls.txt` is
now empty, so the budget is zero everywhere: a button with no handler fails the
build. A control that cannot be wired yet gets deleted, not disabled and left
looking clickable — the markup is in git and in the prototype when it becomes
real. `cargo xtask controls` regenerates the file and, like the ceilings, only
ever tightens.

**The words on the screen come from the catalog.** Fixed text in a component
renders in one language on a screen that promised another, and the diff that
adds it says nothing. `xtask/ui_text_baseline.txt` counts the stretches of
fixed screen text per file, and the count only ever goes down: translate a
slice and run `cargo xtask ui-text` to take the credit — it refuses to raise a
count or to list a new file, exactly like the ceilings. A line the walk
misreads can say `// ui-text: ok <reason>`.

## Tests

**A test must call the rule, not restate it.** A test that re-implements the
condition it is checking passes whether or not the code still applies it. When
a rule is worth testing, extract it into a named function and have the test
call that function.

Prove a test guards something by breaking the rule on purpose and watching the
test fail. If it still passes, the test is decoration.

Integration tests are named for the behaviour they pin, as a sentence:
`a_card_crosses_the_board.rs`, `an_agent_reaches_the_target_terminal.rs`,
`the_pane_shows_no_tmux_chrome.rs`.

Beware of test data with a load-bearing length. `assert_eq!(len(), 8)` next to a
literal that someone will later rename is a trap; say why the length matters or
pick a literal nobody will touch.

## Contract and types

`crates/rpc` is the contract. Frontend types in `web/src/gen/` are **generated**
— never hand-edit them, never write a second copy of a type by hand.

Responses are objects, never bare lists. Tomorrow's extra field needs somewhere
to live.

A command exists only if something calls it. Command and caller land in the
same commit.

## Security rules that are not negotiable

- Every path is resolved through symlinks and checked against the project root
  before it is read or written. This process runs terminals; reaching it is
  reaching the machine.
- Context reaches a `command` step **only through environment variables**, never
  interpolated into the command string. A branch name with a `;` in it must
  become a variable's value, not shell syntax.
- Every read of an externally supplied size has a ceiling before the allocation.

## Commits

Short, objective, English, imperative or descriptive — match what is already in
`git log`. One theme per commit.

**No trailers.** No `Co-Authored-By`, no attribution footers, no tool
signatures.

Never commit unless asked.

## Style

Match the surrounding code: its naming and its idioms.

Comments explain *why*, not *what*, and they are **short**. One or two lines.
A comment restating the line below it is noise; a paragraph explaining a
three-line function is worse, because the next reader skips both. Record the
reason a decision went one way, then stop.

Commit messages the same: a subject line that says what changed, and a body
only when there is a reason someone would otherwise have to guess at.

**Grep a CSS class name before you write it.** The shell's styles are one global
cascade of well over two thousand lines, split into `web/src/shell/styles/` and
imported in order by `shell.css` — grep the whole folder, since a short name is
a name something already has. A rule goes in the part that styles the same
thing; order across parts is the cascade. This has now shipped twice: `.act` for a transcript row met `.act`
for a sidebar button and put a border around every nav item; `.pane` for a
terminal leaf met `.pane` for the tab wrapper and set `display: none` on the
thing it was introducing. Neither is caught by anything — TypeScript does not
see CSS, and the size ratchet only counts lines. The only guard is looking
first, and a two-word name (`.arow`, `.tleaf`) when the obvious one is taken.

## Environment toggles

Seven, all off by default, none of them a feature:

| | |
|---|---|
| `DEVPIT_ACCOUNT_ORIGIN` | points the account at another server, so a build can be run against a local one without editing `account.rs` |
| `DEVPIT_ACCOUNT_NO_BROWSER` | `1` stops sign-in from opening a browser — for tests, and for a machine with none |
| `DEVPIT_TRACE_HOOKS` | timestamped lines on stderr saying how long each hook took |
| `DEVPIT_GDK_BACKEND` | `wayland` or `x11`: the display a Linux start opens on, over Settings and the automatic fallback |
| `DEVPIT_UPDATE_FEED_FILE` | reads the update feed from a file instead of the network. It only replaces where the answer is read from, the card says "test feed" on screen, and nothing it offers can be installed |
| `DEVPIT_DEVFLOW` | the `devflow` program devpit hands Devflow's calls to, before `~/.local/bin` and the PATH; set to nothing, there is none and the linked engine answers everything |
| `DEVFLOW_NO_UPDATE` | `1` stops devpit installing devflow or asking it to update itself, as it stops devflow's own updates |

## Tests

Fast unit tests, and visual checks where the thing is visual. `make test`
takes seconds per change, reaches no network and exercises no service: that is
the bar every commit clears, and nothing slow belongs in it.

Two things sit outside it, deliberately. `make e2e` drives the built app
through a WebDriver and takes minutes, so it is its own target and its own
workflow, `e2e.yml`: nightly, on request, and called by `release.yml` before
anything is built to ship — not on every push. A suite nobody can run locally
is a suite nobody fixes, so run it before a change that touches what it
drives. And
`release.yml` builds, signs and publishes on a tag; it is the one job that
holds a key, and the one place GitHub is exercised.

The rule is the same as it ever was, stated as its reason rather than as a
count: the Makefile is the interface everyone reads, and CI does not invent
commands that cannot be run by hand.
