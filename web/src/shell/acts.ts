import type { Part } from '../gen/bindings'
import { t } from '../i18n'
import { took } from './turnFold'

/*
 * What the agent did, as rows rather than as tool calls.
 *
 * A transcript that prints the tool's own name tells you the mechanism and
 * hides the act: `Bash` is not what happened, `cargo test` is. So a call is
 * classified by name into a small set of kinds, and each kind knows where in
 * the arguments its subject lives.
 *
 * The rules are here rather than in the component because a rule you can call
 * is a rule you can test, and because the classifier is the sort of thing that
 * grows a case every time a CLI names a tool differently.
 */

export type Kind = 'think' | 'run' | 'edit' | 'read' | 'find' | 'list' | 'plan' | 'agent' | 'tool'

export interface Act {
  readonly id: string
  readonly kind: Kind
  /* The tool's own name, kept for the rows a kind cannot name better. */
  readonly name: string
  /* The subject: a file, a command, a query. Empty when there is none. */
  readonly target: string
  readonly input: string
  readonly output: string
  readonly failed: boolean
  readonly done: boolean
  /* What a subagent did, for the `Agent` call that started it. */
  readonly children: readonly Act[]
  /* How long a thought or a subagent took, when the stream timed it. */
  readonly durationMs?: number | null
  /* The MCP server a call went to. */
  readonly server?: string | null
  /* The CLI's word for how a subagent's task ended. */
  readonly endedAs?: string | null
}

/* The verb on the row, read from the catalog by its kind. */
export const actionLabel = (kind: Kind): string => t(`acts.kind.${kind}`)

/* The verb a row leads with: a timed thought says how long it took. */
export function rowLabel(act: Act): string {
  if (act.kind === 'think' && act.durationMs != null) return t('acts.thoughtFor', { time: took(Math.max(act.durationMs, 1000)) })
  return actionLabel(act.kind)
}

/* `mcp__figma__get_design_context` went to `figma`. */
export function serverOf(name: string): string | null {
  const [head, server, ...rest] = name.split('__')
  return head === 'mcp' && server && rest.length > 0 ? server : null
}

/* One tool name, in whatever spelling. MCP namespaces it with `__`, some CLIs
   with `:` or `.`, and the same act is `run_command`, `runCommand` or `Bash`
   depending on who is asking — so compare on a flattened leaf. */
function leaf(name: string): string {
  const lower = name.trim().toLowerCase().replace(/[-\s]+/g, '_')
  const afterNamespace = lower.split('__').at(-1) ?? lower
  return (afterNamespace.split(/[:./]/).at(-1) ?? afterNamespace).replace(/_/g, '')
}

const RUNS = ['bash', 'command', 'execute', 'executecommand', 'runcommand', 'runterminalcommand', 'shell', 'shellcommand', 'terminal']
const EDITS = ['applypatch', 'create', 'createfile', 'delete', 'deletefile', 'edit', 'editfile', 'filechange', 'fileedit', 'move', 'movefile', 'multiedit', 'notebookedit', 'patch', 'rename', 'renamefile', 'replace', 'savefile', 'strreplace', 'write', 'writefile']
const READS = ['read', 'fileread', 'readfile', 'readtextfile', 'viewfile']
const FINDS = ['grep', 'glob', 'search', 'filesearch', 'searchfiles', 'findfiles', 'websearch', 'webfetch', 'fetch']
const LISTS = ['ls', 'filelist', 'listfiles', 'listdirectory', 'listdir']
/* `TaskCreate` and `TaskUpdate` are Claude Code's checklist since it stopped
   using `TodoWrite` — measured on 2.1.270. */
const PLANS = ['todo', 'todowrite', 'updateplan', 'plan', 'exitplanmode', 'taskcreate', 'taskupdate']
/* `Agent` today; `Task` is what the same tool was called before. */
const AGENTS_TOOLS = ['agent', 'task', 'subagent', 'spawnagent']

export function kindOf(name: string): Kind {
  const it = leaf(name)
  if (PLANS.includes(it)) return 'plan'
  if (AGENTS_TOOLS.includes(it)) return 'agent'
  if (RUNS.includes(it)) return 'run'
  if (EDITS.includes(it)) return 'edit'
  if (READS.includes(it)) return 'read'
  if (FINDS.includes(it)) return 'find'
  if (LISTS.includes(it)) return 'list'
  return 'tool'
}

/* The last segment of a path. A row has one line and a repository has deep
   folders; the folder is in the arguments a click away. */
const base = (path: string): string => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path

/* One line, and not a long one. A heredoc or a chained command would push the
   rest of the row off the screen. */
function oneLine(text: string, limit = 90): string {
  const first = text.split('\n').find((line) => line.trim()) ?? ''
  const trimmed = first.trim()
  return trimmed.length > limit ? `${trimmed.slice(0, limit - 1).trimEnd()}…` : trimmed
}

/* Where each kind keeps its subject. Tool arguments are the provider's shape,
   not ours, so every field we know of is tried and none is required. */
export function targetOf(kind: Kind, input: string): string {
  const args = parsed(input)
  if (!args) return ''
  const at = (key: string): string => (typeof args[key] === 'string' ? (args[key] as string) : '')

  switch (kind) {
    case 'run':
      /* The human sentence when the caller wrote one: `description` is what
         the agent meant, `command` is how it said it. */
      return oneLine(at('description') || at('command'))
    case 'edit':
    case 'read':
      return base(at('file_path') || at('filePath') || at('path') || at('notebook_path'))
    case 'list':
      return base(at('path') || at('directory'))
    case 'find':
      return oneLine(at('pattern') || at('query') || at('url') || at('regex'), 60)
    case 'agent':
      return oneLine(at('description') || at('subagent_type'), 70)
    default:
      return ''
  }
}

function parsed(input: string): Record<string, unknown> | null {
  if (!input.trim().startsWith('{')) return null
  try {
    const value: unknown = JSON.parse(input)
    return typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : null
  } catch {
    return null
  }
}

/* `mcp__figma__get_design_context` reads as "Get design context". A tool we
   cannot classify at least should not shout its wire name. */
export function toolName(name: string): string {
  const trimmed = name.trim()
  if (/\s/.test(trimmed)) return trimmed
  const words = (trimmed.split('__').at(-1) ?? trimmed)
    .split(/[:./]/)
    .at(-1)!
    .replace(/[_-]+/g, ' ')
    .replace(/([a-z\d])([A-Z])/g, '$1 $2')
    .trim()
  return words ? words[0]!.toUpperCase() + words.slice(1) : t('acts.kind.tool')
}

/* The parts of one message, paired into rows.

   A `tool_result` is not a row: it is the end of the call it answers. Pairing
   them here is what lets the row show `Run cargo test` with the output folded
   underneath, instead of a call and a result sitting apart. */
export function acts(parts: readonly Part[], every: readonly Part[] = parts): readonly Act[] {
  const own = parts.filter((part) => !('parent' in part) || !part.parent)
  const rows = rowsOf(own, every)
  return rows.map((row) => {
    const inside = every.filter((part) => 'parent' in part && part.parent === row.id)
    const ending = endingOf(row.id, every)
    const done = row.done && ending !== null
    const agent =
      row.kind === 'agent'
        ? { ...row, done, durationMs: ranFor(row.id, every), endedAs: ending ?? null, failed: row.failed || (ending != null && ending !== 'completed') }
        : row
    return inside.length ? { ...agent, children: rowsOf(inside, every), done } : agent
  })
}

/* A subagent's state, as the style knows it: an id, never a word to read. */
export type RanState = 'working' | 'done' | 'failed' | 'stopped'

export function ranState(act: Act): RanState {
  if (!act.done) return 'working'
  if (act.endedAs && act.endedAs !== 'completed' && act.endedAs !== 'failed') return 'stopped'
  return act.failed ? 'failed' : 'done'
}

/* The same state, in a word. */
export function ranAs(act: Act): string {
  return t(`acts.state.${ranState(act)}`)
}

/* The CLI's own count for the task a call started, at its latest report. */
function ranFor(callId: string, parts: readonly Part[]): number | null {
  const timed = parts.findLast((part) => part.kind === 'task' && part.call_id === callId && part.duration_ms != null)
  return timed?.kind === 'task' ? (timed.duration_ms ?? null) : null
}

/* How the task a call started ended: `undefined` when it started none, `null`
   while it runs. A subagent is over when its task says so, not when its
   `Agent` call returns: the call answers at once and the work goes on. */
function endingOf(callId: string, parts: readonly Part[]): string | null | undefined {
  const last = parts.findLast((part) => part.kind === 'task' && part.call_id === callId)
  if (last?.kind !== 'task') return undefined
  return last.status === 'started' || last.status === 'running' ? null : last.status
}

function rowsOf(parts: readonly Part[], every: readonly Part[]): Act[] {
  const results = new Map<string, { output: string; failed: boolean }>()
  for (const part of every) {
    if (part.kind === 'tool_result') {
      results.set(part.call_id, { output: part.output, failed: part.is_error })
    }
  }

  const rows: Act[] = []
  /* Placed in the whole message, so a row's id and fold are its own in every segment. */
  parts.forEach((part) => {
    const at = every.indexOf(part)
    if (part.kind === 'thinking') {
      rows.push({ id: `t${at}`, kind: 'think', name: t('acts.doing.thinking'), target: oneLine(part.text), input: '', output: part.text, failed: false, done: true, children: [], durationMs: part.duration_ms ?? null })
      return
    }
    if (part.kind === 'unknown') {
      rows.push({ id: `u${at}`, kind: 'tool', name: t('acts.output'), target: oneLine(part.text), input: '', output: part.text, failed: false, done: true, children: [] })
      return
    }
    if (part.kind !== 'tool_call') return
    const kind = kindOf(part.name)
    const answer = results.get(part.id)
    rows.push({
      id: part.id,
      kind,
      name: part.name,
      target: targetOf(kind, part.input) || (kind === 'tool' ? toolName(part.name) : ''),
      input: part.input,
      output: answer?.output ?? '',
      failed: answer?.failed ?? part.state === 'failed',
      done: answer !== undefined || part.state !== 'running',
      children: [],
      server: serverOf(part.name),
    })
  })
  return rows
}

/* Several of the same act in a row, read as one.

   Eight `Read` rows say nothing eight times over; "Read 8 files" says it once
   and opens to the eight. Only settled rows fold: a running call is what you
   are watching, and hiding it inside a count hides the one row that matters. */
export interface Group {
  readonly id: string
  readonly kind: Kind
  readonly rows: readonly Act[]
}

export type Item = Act | Group

export const isGroup = (item: Item): item is Group => 'rows' in item

/* Fewer than this is a list, not a run. */
const FOLD_AT = 3

export function grouped(rows: readonly Act[]): readonly Item[] {
  const items: Item[] = []
  let run: Act[] = []
  /* Subagents started side by side, running or not: one row says how many are still at it. */
  let team: Act[] = []
  const flush = (): void => {
    if (run.length >= FOLD_AT) items.push({ id: `g${run[0]!.id}`, kind: run[0]!.kind, rows: run })
    else items.push(...run)
    run = []
  }
  const flushTeam = (): void => {
    if (team.length >= 2) items.push({ id: `g${team[0]!.id}`, kind: 'agent', rows: team })
    else items.push(...team)
    team = []
  }
  for (const row of rows) {
    if (row.kind === 'agent') {
      flush()
      team.push(row)
      continue
    }
    flushTeam()
    const joins =
      run.length > 0 &&
      row.done &&
      !row.failed &&
      row.kind !== 'think' &&
      row.kind === run[0]!.kind &&
      row.name === run[0]!.name
    if (joins) {
      run.push(row)
      continue
    }
    flush()
    if (row.done && !row.failed && row.kind !== 'think') run.push(row)
    else items.push(row)
  }
  flush()
  flushTeam()
  return items
}

/* What a group amounts to, as the subject beside its verb. */
export function groupTarget(group: Group): string {
  const counted = t(`acts.group.${group.kind}`, { count: group.rows.length })
  const working = group.kind === 'agent' ? group.rows.filter((row) => !row.done).length : 0
  return working > 0 ? t('acts.group.someWorking', { counted, working }) : counted
}

/* What a settled group of rows amounts to: "Ran 3 commands · 2 file edits". */
export function tally(rows: readonly Act[]): string {
  const counts = new Map<Kind, number>()
  for (const row of rows) counts.set(row.kind, (counts.get(row.kind) ?? 0) + 1)
  return [...counts]
    .map(([kind, count]) => t(`acts.tally.${kind}`, { count }))
    .join(' · ')
}

export function summary(rows: readonly Act[]): string {
  const running = rows.some((row) => !row.done)
  return t(running ? 'acts.summary.now' : 'acts.summary.ran', { what: tally(rows) })
}

/* The header names the newest row while the work is live — that is what you
   look up to check — and the tally once it settles, because by then you want
   to know what it came to, not what it ended on. */
export function headline(rows: readonly Act[], live: boolean): string {
  if (!rows.length) return live ? t('acts.doing.working') : ''
  if (!live) return summary(rows)
  const last = rows[rows.length - 1]!
  return last.target ? `${actionLabel(last.kind)} ${last.target}` : actionLabel(last.kind)
}

/* What a live turn is at, for the line under it: the call still open, or
   thinking, or writing the answer. */
export function doing(parts: readonly Part[]): string {
  const last = parts.filter((part) => !('parent' in part) || !part.parent).at(-1)
  const opened = acts(parts).filter((row) => !row.done)
  if (opened.length > 1 && opened.every((row) => row.kind === 'agent')) return t('acts.doing.waiting', { count: opened.length })
  const open = opened.at(-1)
  if (open) return open.target ? `${actionLabel(open.kind)} ${open.target}` : actionLabel(open.kind)
  if (last?.kind === 'thinking') return t('acts.doing.thinking')
  if (last?.kind === 'text') return t('acts.doing.writing')
  return t('acts.doing.working')
}

/* The agent CLIs, by the name their process wears.

   A terminal sitting at its prompt shows the shell; a terminal with one of
   these in front is a conversation someone is having. The sidebar draws the
   second differently, so the list is here rather than in the component. */
const AGENTS: Readonly<Record<string, string>> = {
  claude: 'Claude Code',
  codex: 'Codex',
  gemini: 'Gemini',
  cursor: 'Cursor',
  amp: 'Amp',
  droid: 'Droid',
  grok: 'Grok',
  opencode: 'OpenCode',
  copilot: 'Copilot',
  aider: 'Aider',
}

/* What to call the process in front of a pane.

   An agent gets the name people use for it; anything else keeps the name it
   has, because `cargo` and `vim` are already what a person would say. */
export function runningName(command: string): string {
  return AGENTS[command.trim().toLowerCase()] ?? command.trim()
}

/* Whether the foreground process is one of the agents. Drawn with the mark
   rather than the terminal glyph — the row is about a conversation, not about
   a command that happens to be running. */
export const isAgent = (command: string): boolean =>
  Object.hasOwn(AGENTS, command.trim().toLowerCase())
