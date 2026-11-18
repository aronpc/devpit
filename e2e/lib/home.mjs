/*
 * The machine the suite pretends to be on.
 *
 * Everything the app reads — state, git config, the CLI's own directory, the
 * shell's rc files — is under `target/e2e-home`, recreated from nothing on
 * every run. Two reasons, and the second is the one that matters: a suite that
 * reads the real `~/.devpit` tells you about that machine and not about the
 * build, and a suite that *writes* to it has deleted somebody's board.
 */

import { execFileSync, spawnSync } from 'node:child_process'
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

/** A port nothing listens on, so a sign-in attempt fails instead of leaving. */
const CLOSED_PORT = 'http://127.0.0.1:9'

/**
 * The home, wiped and rebuilt. Returns where things are.
 *
 * `name` because test files run in their own processes: two of them seeding
 * the same directory means one wipes the other's git repository halfway
 * through, and the error that comes out is about an ignored file.
 */
export function seedHome(root, name = 'e2e-home') {
  const home = join(root, 'target', name)
  // Before the folder goes: deleting it takes the socket and leaves the server
  // running with no way to reach it — that is how a run left one behind.
  endTerminals(home)
  rmSync(home, { recursive: true, force: true })

  for (const dir of [
    '.cache',
    '.config',
    '.local/share',
    '.local/bin',
    '.claude',
    '.devpit',
    'work',
  ]) {
    mkdirSync(join(home, dir), { recursive: true })
  }

  // A login shell reads these, and the app asks a login shell what `claude`
  // is. Empty is not enough: the file has to exist, or zsh falls back to the
  // system one, which is the machine's and not this home's.
  const bin = join(home, '.local/bin')
  const line = `export PATH="${bin}:/usr/local/bin:/usr/bin:/bin"\n`
  for (const rc of ['.zshenv', '.zshrc', '.bashrc', '.profile']) {
    writeFileSync(join(home, rc), line)
  }

  writeFileSync(
    join(home, '.gitconfig'),
    '[user]\n\tname = devpit e2e\n\temail = e2e@devpit.invalid\n[init]\n\tdefaultBranch = main\n',
  )

  // Two of them: a focus is about one project being interrupted by another,
  // and one repository can only ever be one project.
  const made = (name) => {
    const where = join(home, 'work', name)
    mkdirSync(where, { recursive: true })
    const git = (...args) => execFileSync('git', args, { cwd: where, env: gitEnv(home) })
    git('init', '-q')
    writeFileSync(join(where, 'README.md'), `# ${name}\n\nA repository with one commit.\n`)
    git('add', 'README.md')
    git('commit', '-qm', 'first')
    return where
  }
  const repo = made('fixture')
  const other = made('fixture-two')

  // The `claude` a login shell in this home finds. A one-line shim rather
  // than a copy: the stub is source in the tree, and a copy would go stale the
  // first time it changed.
  const stub = join(bin, 'claude')
  const script = join(dirname(fileURLToPath(import.meta.url)), '..', 'stub', 'claude.mjs')
  writeFileSync(stub, `#!/bin/sh\nexec "${process.execPath}" "${script}" "$@"\n`)
  chmodSync(stub, 0o755)

  const feed = join(home, 'feed.json')
  writeFileSync(
    feed,
    `${JSON.stringify({ version: '99.0.0', notes: 'A version from a file, for the suite.' }, null, 2)}\n`,
  )

  return { home, repo, other, feed, bin, stub }
}

/**
 * Another fixture repository in a seeded home, for a test that needs a project
 * of its own — the tests share a home and a driver, and one test archiving the
 * card another one photographs is a flaky suite.
 */
export function seedRepo(home, name) {
  const repo = join(home, 'work', name)
  rmSync(repo, { recursive: true, force: true })
  mkdirSync(repo, { recursive: true })
  const git = (...args) => execFileSync('git', args, { cwd: repo, env: gitEnv(home) })
  git('init', '-q')
  writeFileSync(join(repo, 'README.md'), `# ${name}\n`)
  git('add', 'README.md')
  git('commit', '-qm', 'first')
  return repo
}

function gitEnv(home) {
  return { ...process.env, HOME: home, GIT_CONFIG_GLOBAL: join(home, '.gitconfig') }
}

/**
 * The variables a window needs from the machine, and nothing else.
 *
 * An allowlist, not the parent's environment minus a few names: the review
 * found ZDOTDIR passing through, which the terminal honours, so the panes read
 * the developer's own zsh config — their PATH, their aliases, their tokens.
 * Whatever is not named here does not reach the app.
 */
const FROM_THE_MACHINE = [
  'DISPLAY',
  'WAYLAND_DISPLAY',
  'XDG_RUNTIME_DIR',
  'XAUTHORITY',
  'LANG',
  'LC_ALL',
  'LC_CTYPE',
  'TERM',
  'SHELL',
  'USER',
  'LOGNAME',
  'TZ',
]

/** The environment the app is started with. */
export function seedEnv({ home, feed }) {
  const env = {}
  for (const key of FROM_THE_MACHINE) {
    if (process.env[key] !== undefined) env[key] = process.env[key]
  }
  return {
    ...env,
    HOME: home,
    XDG_CACHE_HOME: join(home, '.cache'),
    XDG_CONFIG_HOME: join(home, '.config'),
    XDG_DATA_HOME: join(home, '.local/share'),
    // The system's own directories after the stub's, never the developer's
    // PATH: a real `claude` in ~/.local/bin is exactly what must not answer.
    PATH: `${join(home, '.local/bin')}:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`,
    GIT_CONFIG_GLOBAL: join(home, '.gitconfig'),
    // Same root as HOME gives; set so the app never hands its window to an
    // installed devpit over D-Bus.
    DEVPIT_HOME: join(home, '.devpit'),
    DEVPIT_ACCOUNT_ORIGIN: CLOSED_PORT,
    DEVPIT_ACCOUNT_NO_BROWSER: '1',
    DEVPIT_UPDATE_FEED_FILE: feed,
    // The hook timings, on the app's stderr, which the driver keeps in a file.
    DEVPIT_TRACE_HOOKS: '1',
    // No devflow program: Devflow runs on the linked engine, and nothing is
    // downloaded or updated.
    DEVPIT_DEVFLOW: '',
    DEVFLOW_NO_UPDATE: '1',
  }
}

/** Ends what a seeded home's terminals run on: its tmux server and its daemon. */
export function endTerminals(home) {
  endTmux(home)
  endPtyd(join(home, '.devpit'))
}

/** Ends the tmux server on a home's devpit socket, if one is running there. */
export function endTmux(home) {
  const socket = join(home, '.devpit', 'tmux.sock')
  if (!existsSync(socket)) return
  spawnSync('tmux', ['-S', socket, 'kill-server'], { stdio: 'ignore' })
}

/**
 * Ends the devpit-ptyd daemon of a devpit home (its `DEVPIT_HOME`), and its socket.
 *
 * The app starts it in a session of its own so terminals outlive the window.
 * Left behind, the next run on the same home connected to it and found the
 * last run's panes, and every run added one more.
 */
export function endPtyd(devpit) {
  // The kernel names the executable by its real path, and a demo's root is a link.
  const copies = [devpit, existsSync(devpit) ? realpathSync(devpit) : devpit].map((dir) => join(dir, 'ptyd') + sep)
  const daemons = ownProcesses().filter((pid) => copies.some((copy) => exeOf(pid)?.startsWith(copy)))
  // The socket is the daemon's last argument: read before it goes.
  const sockets = daemons.map((pid) => argsOf(pid).at(-1)).filter(Boolean)
  for (const pid of daemons) signal(pid, 'SIGTERM')
  for (let look = 0; look < 40 && daemons.some(alive); look += 1) pause(50)
  for (const pid of daemons.filter(alive)) signal(pid, 'SIGKILL')
  for (const socket of sockets) rmSync(socket, { force: true })
}

/** Every process this user runs, by pid; none where there is no `/proc`. */
function ownProcesses() {
  try {
    return readdirSync('/proc')
      .filter((name) => /^\d+$/.test(name))
      .map(Number)
  } catch {
    return []
  }
}

function exeOf(pid) {
  try {
    return readlinkSync(`/proc/${pid}/exe`)
  } catch {
    return null
  }
}

function argsOf(pid) {
  try {
    return readFileSync(`/proc/${pid}/cmdline`, 'utf8').split('\0').filter(Boolean)
  } catch {
    return []
  }
}

function signal(pid, name) {
  try {
    process.kill(pid, name)
  } catch {
    // Already gone, or not ours.
  }
}

function alive(pid) {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

/** A synchronous wait: `seedHome`, which ends the last run's daemon, is synchronous. */
function pause(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
}
