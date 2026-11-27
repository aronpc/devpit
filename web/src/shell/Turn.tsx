import { memo, useCallback, useContext, useEffect, useState } from 'react'

import { Acts, OpensPath } from './ActsRows'
import { drawn, useActivity, type Activity } from './activity'
import { acts, doing, tally, type Act } from './acts'
import { joined, metaOf } from './chat'
import { CopyButton } from './CopyButton'
import type { Message, Part } from '../gen/bindings'
import { inFolder } from './markdown'
import { Markdown } from './MarkdownView'
import { useShellPick } from './shellStore'
import { Rewound } from './Rewound'
import { Said } from './Said'
import { TurnApps } from './TurnApps'
import { TurnDelegations } from './TurnDelegations'
import { TurnDrafts } from './TurnDrafts'
import { TurnChanges } from './TurnChangesView'
import { answerOf, compactedAt, millis, outcome, segments, statusOf, took, type Segment } from './turnFold'
import { foldOf, useDisclosed } from './useDisclosed'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * One message, drawn part by part.
 *
 * What you said sits right, in a bubble. What the agent did sits left, under
 * a rule, as a stack of rows you can open — a tool call, a thought and an
 * answer are three different things, and a transcript that flattens them into
 * paragraphs is one nobody reads twice.
 */

/* Memoized: a chunk of the answer being written changes one message, and
   every turn above it used to parse and draw its markdown again. */
export const Turn = memo(function Turn({
  message,
  onRewind,
  onEdit,
  onAgain,
  folder,
  onOpen,
  onResend,
  steered = false,
}: {
  message: Message
  /** Somebody steered this turn: in Zen its earlier replies stay, as answers to them. */
  steered?: boolean
  /** Present on a message steered in that its turn never took in. */
  onResend?: () => void
  /** Where a file the answer names opens; a tab when absent. */
  onOpen?: (path: string) => void
  /** Where the conversation runs; relative links in the answer are read there. */
  folder?: string | null
  /** Present when this turn can be forked from; called with its turn id. */
  onRewind?: (turnId: string) => void
  /** Present when the message that started this turn can be edited; called with its turn id. */
  onEdit?: (turnId: string) => void
  /** Sends a person's message again. */
  onAgain?: () => void
}): React.JSX.Element {
  const rewind = onRewind && message.turnId ? () => onRewind(message.turnId!) : undefined
  const edit = onEdit && message.turnId ? () => onEdit(message.turnId!) : undefined
  const receipt = message.parts.find((part) => part.kind === 'receipt')
  if (message.role === 'system' && receipt?.kind === 'receipt') return <Receipt part={receipt} />
  const rewound = message.parts.find((part) => part.kind === 'rewound')
  if (message.role === 'system' && rewound?.kind === 'rewound') return <Rewound part={rewound} />
  const compacted = message.parts.find((part) => part.kind === 'compacted')
  if (message.role === 'system' && compacted?.kind === 'compacted') return <div className="rwnd">{compactedAt(compacted.tokens)}</div>

  if (message.role === 'user') return <Said message={message} onResend={onResend} onEdit={edit} onAgain={onAgain} />
  return <Answer message={message} rewind={rewind} edit={edit} folder={folder ?? null} onOpen={onOpen} steered={steered} />
})

function Answer({
  message,
  rewind,
  edit,
  folder,
  onOpen,
  steered,
}: {
  message: Message
  rewind?: () => void
  edit?: () => void
  folder: string | null
  onOpen?: (path: string) => void
  steered: boolean
}): React.JSX.Element {
  const [open, setOpen] = useDisclosed(foldOf(message.id, 'turn'), false)
  const activity = useActivity()
  const live = message.streaming
  const every = joined(message.parts)
  const shown = segments(every)
  const answer = answerOf(shown)
  const ended = live ? null : (message.ended ?? null)
  const failed = ended !== null && outcome(ended) === 'failed'
  const lastWork = shown.findLastIndex((one) => one.kind === 'work')
  const rows = (one: Segment): readonly Act[] => (one.kind === 'work' ? acts(one.parts, every) : [])
  /* What a settled turn folds: the work and earlier prose, never its answer or what a slash command said. A failed one folds nothing. */
  const hides = !live && !failed && shown.some((one, at) => at !== answer && (one.kind === 'prose' || rows(one).length > 0))
  const changed = message.parts.flatMap((part) => (part.kind === 'changes' ? part.files : []))
  const show = useShellPick((shell) => shell.show)
  /* A path the answer or a tool row names, read where the conversation runs. */
  const opens = useCallback(
    (here: string): void => {
      const path = inFolder(folder, here)
      if (onOpen) onOpen(path)
      else show('file', { id: `file:${path}`, path })
    },
    [folder, onOpen, show],
  )

  return (
    <OpensPath.Provider value={opens}>
    <article className="turn" data-ended={ended ? outcome(ended) : undefined}>
      {!live && !failed && activity !== 'zen' && (hides || ended) && (
        <TurnStatus message={message} work={shown.flatMap(rows)} hides={hides} open={open} toggle={() => setOpen((was) => !was)} />
      )}
      <Segments message={message} every={every} shown={shown} answer={answer} folder={folder} folded={hides && !open && activity !== 'zen'} lastWork={failed ? lastWork : -1} activity={activity} steered={steered} />
      <TurnDelegations parts={message.parts} />
      <TurnDrafts parts={message.parts} />
      <TurnApps parts={message.parts} messageId={message.id} />
      {failed && <Failed message={message} />}
      {!live && <TurnChanges files={changed} parts={message.parts} />}
      {live && <Working parts={message.parts} since={millis(message.createdAt)} />}
      {!live && <Foot message={message} rewind={rewind} edit={edit} />}
    </article>
    </OpensPath.Provider>
  )
}

/* The turn in its order. Folded, only the answer is drawn; the status line above it opens the rest. */
function Segments({
  message,
  every,
  shown,
  answer,
  folded,
  lastWork,
  activity,
  steered,
  folder,
}: {
  message: Message
  every: readonly Part[]
  shown: readonly Segment[]
  answer: number
  folder: string | null
  folded: boolean
  /** The work a failed turn ended on, kept open; -1 for none. */
  lastWork: number
  activity: Activity
  steered: boolean
}): React.JSX.Element {
  const live = message.streaming
  const last = shown.length - 1
  return (
    <>
      {shown.map((one, at) => {
        if (folded && at !== answer && one.kind !== 'said') return null
        if (!drawn(one, at, answer, activity, steered)) return null
        if (one.kind === 'prose')
          return <Reply key={one.at} source={one.text} live={live && at === last} folder={folder} />
        if (one.kind === 'said')
          return (
            <pre className="said__cmd" key={one.at}>
              {one.content}
            </pre>
          )
        return (
          <Acts
            key={one.at}
            parts={one.parts}
            every={every}
            live={live && at === last}
            messageId={message.id}
            fold={`acts:${one.at}`}
            kept={at === lastWork}
          />
        )
      })}
    </>
  )
}

/* "Worked for 1m 30s", and what it came to. A button when it hides the work. */
function TurnStatus({
  message,
  work,
  hides,
  open,
  toggle,
}: {
  message: Message
  work: readonly Act[]
  hides: boolean
  open: boolean
  toggle: () => void
}): React.JSX.Element {
  const said = statusOf(message) ?? t('sessions.turn.worked')
  const counted = work.length > 0 ? tally(work) : ''
  const inner = (
    <>
      <span className="tstat__w">{said}</span>
      {counted && <span className="tstat__n">· {counted}</span>}
    </>
  )
  if (!hides) return <div className="tstat" data-how={message.ended ? outcome(message.ended) : undefined}>{inner}</div>
  return (
    <button className="tstat" data-how={message.ended ? outcome(message.ended) : undefined} aria-expanded={open} onClick={toggle}>
      {inner}
      <Icon size="s" className="tstat__v" style={{ transform: open ? 'none' : 'rotate(-90deg)' }}>
        <path d="m6 9 6 6 6-6" />
      </Icon>
    </button>
  )
}

/* A failed turn ends on what it failed on, with nothing to open first. */
function Failed({ message }: { message: Message }): React.JSX.Element {
  return (
    <div className="tstat" data-how="failed" role="alert">
      <span className="tstat__w">{statusOf(message)}</span>
      {message.ended?.error && <span className="tstat__e">{message.ended.error}</span>}
    </div>
  )
}

/* The answer as it has arrived: the backend paces it into batches, and each
   new block fades in rather than the text being typed out on a timer. */
function Reply({ source, live, folder }: { source: string; live: boolean; folder: string | null }): React.JSX.Element {
  const opens = useContext(OpensPath)
  return (
    <div className="reply" data-live={live || undefined} data-fade={(live && !reducedMotion()) || undefined}>
      {/* A path ending in `/` is the folder itself: a picture the answer names is read there. */}
      <Markdown source={source} live={live} opens={opens ?? undefined} path={folder ? `${folder.replace(/\/$/, '')}/` : undefined} />
    </div>
  )
}

const reducedMotion = (): boolean =>
  typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches

/* How long it has been at it, counted from when the turn began rather than
   from when this was drawn: a reload mid-turn does not start again at 0. */
function Working({ parts, since }: { parts: Message['parts']; since: number | null }): React.JSX.Element {
  const [mounted] = useState(() => Date.now())
  const began = since ?? mounted
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    /* Aimed at the next whole second rather than a second from now: an
       interval started mid-second drifts, and a counter that skips a number
       is a counter you stop trusting. */
    let timer = 0
    const tick = (): void => {
      setNow(Date.now())
      timer = window.setTimeout(tick, 1000 - (Date.now() % 1000) + 8)
    }
    timer = window.setTimeout(tick, 1000 - (Date.now() % 1000) + 8)
    return () => window.clearTimeout(timer)
  }, [])

  const said = took(Math.floor(Math.max(0, now - began) / 1000) * 1000)

  return (
    <div className="working">
      <span className="working__dots" aria-hidden="true">
        <i />
        <i />
        <i />
      </span>
      <span className="working__what">{doing(parts)}</span>
      <span>· {said}</span>
    </div>
  )
}

/* Under a settled turn: one quiet line of when it ended, on what, how long
   and for how much, and on hover what can be done with it. */
function Foot({ message, rewind, edit }: { message: Message; rewind?: () => void; edit?: () => void }): React.JSX.Element | null {
  const said = message.parts
    .filter((part) => part.kind === 'text')
    .map((part) => ('text' in part ? part.text : ''))
    .join('\n\n')
  const meta = metaOf(message)

  if (!said && !rewind && !edit && meta.length === 0) return null

  return (
    <div className="turn__foot">
      {meta.length > 0 && <span className="tmeta">{meta.join(' · ')}</span>}
      <span className="hbar">
        {said && <CopyButton className="tfbtn" text={said} />}
        {edit && (
          <button className="tfbtn" aria-label={t('sessions.turn.edit')} title={t('sessions.turn.editTitle')} onClick={edit}>
            <Icon size="s"><path d="M12 20h9" /><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" /></Icon>
          </button>
        )}
        {rewind && (
          <button className="tfbtn" aria-label={t('sessions.turn.fork')} title={t('sessions.turn.forkTitle')} onClick={rewind}>
            <Icon size="s"><circle cx="6" cy="5" r="2" /><circle cx="18" cy="5" r="2" /><circle cx="12" cy="19" r="2" /><path d="M6 7v2a3 3 0 0 0 3 3h6a3 3 0 0 0 3-3V7M12 12v5" /></Icon>
          </button>
        )}
      </span>
    </div>
  )
}

/* One line for a decision a person made: what the agent wanted, and whether
   it was allowed. */
function Receipt({ part }: { part: Extract<Message['parts'][number], { kind: 'receipt' }> }): React.JSX.Element {
  const what = receiptSubject(part.input)
  return (
    <div className="rcpt" data-allowed={part.allowed}>
      <b>{part.allowed ? t('sessions.turn.allowed') : t('sessions.turn.refused')}</b> {part.tool}
      {what && <code className="rcpt__w">{what}</code>}
    </div>
  )
}

/* The part of a tool's input a person recognises: the command, or the file. */
export function receiptSubject(input: string): string {
  try {
    const args = JSON.parse(input) as Record<string, unknown>
    for (const key of ['command', 'file_path', 'path', 'url', 'pattern']) {
      if (typeof args[key] === 'string') return (args[key] as string).split('\n')[0]!.slice(0, 120)
    }
  } catch {
    /* Not JSON: nothing to pick out. */
  }
  return ''
}
