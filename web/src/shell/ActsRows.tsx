import { createContext, useContext, useEffect, useRef, useState } from 'react'

import { imagesOf, isCut, namedImages, textOf, WholeContext } from './cutOutput'
import { ask, commands } from './live'
import { acts, actionLabel, grouped, groupTarget, headline, isGroup, ranState, rowLabel, type Act, type Group, type Kind } from './acts'
import { shownRows, useActivity } from './activity'
import type { Part } from '../gen/bindings'
import { t } from '../i18n'
import { editDiff } from './editCard'
import { EditCard } from './EditCardView'
import { Markdown } from './MarkdownView'
import { Pathed } from './MdTick'
import { took } from './turnFold'
import { useDisclosed } from './useDisclosed'
import { Zoomable } from './Zoom'

/*
 * What the agent did, as a foldable stack of rows.
 *
 * Open while it works, because that is when you want to watch; closed once it
 * is done, because by then the answer is what you came for.
 */

/* Where a file the rows name opens: beside the chat, read in its folder. */
export const OpensPath = createContext<((path: string) => void) | null>(null)

/* The message the rows are of, to ask for an output the window got cut. */
const MessageContext = createContext<string | null>(null)

/* A fold's key: its message and its row, or none outside a message. */
function useFoldKey(id: string): string | null {
  const messageId = useContext(MessageContext)
  return messageId && `${messageId}:${id}`
}

export function Acts({
  parts,
  every = parts,
  live,
  messageId = null,
  fold = 'acts',
  kept = false,
}: {
  parts: readonly Part[]
  /** The whole message, where results and a subagent's parts are looked up. */
  every?: readonly Part[]
  live: boolean
  messageId?: string | null
  /** This stack's fold within its message. */
  fold?: string
  /** Stays open once it settles: the work a failed turn ended on. */
  kept?: boolean
}): React.JSX.Element | null {
  const rows = shownRows(acts(parts, every), useActivity())
  const [open, setOpen] = useDisclosed(messageId && `${messageId}:${fold}`, live || kept)
  /* Open while it runs and folded once done; a turn drawn again keeps what was chosen. */
  const was = useRef(live)
  useEffect(() => {
    if (was.current !== live) setOpen(live || kept)
    was.current = live
  }, [live, kept, setOpen])

  if (rows.length === 0) return null

  return (
    <MessageContext.Provider value={messageId}>
    <div className="acts">
      <button className="acts__h" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
        <span className="acts__t">{headline(rows, live)}</span>
        <Chevron open={open} />
      </button>
      {open && (
        <div className="acts__body">
          {grouped(rows).map((item) =>
            isGroup(item) ? <GroupRow key={item.id} group={item} /> : <Row key={item.id} act={item} />,
          )}
        </div>
      )}
    </div>
    </MessageContext.Provider>
  )
}

function AgentRow({ act }: { act: Act }): React.JSX.Element {
  const [open, setOpen] = useDisclosed(useFoldKey(act.id), false)
  return (
    <div className="arow">
      <button className="arow__h" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
        <Icon kind={act.kind} />
        <span className="arow__k">{actionLabel(act.kind)}</span>
        {act.target && <span className="arow__d">{act.target}</span>}
        <Ran act={act} />
        {act.done ? <Chevron open={open} className="arow__v" /> : <span className="arow__live" aria-label={t('acts.running')} />}
      </button>
      {open && (
        <div className="arow__body">
          {grouped(act.children).map((item) =>
            isGroup(item) ? <GroupRow key={item.id} group={item} /> : <Row key={item.id} act={item} />,
          )}
        </div>
      )}
    </div>
  )
}

/* A run of one act, closed to its count and open to its rows. */
function GroupRow({ group }: { group: Group }): React.JSX.Element {
  const [open, setOpen] = useDisclosed(useFoldKey(group.id), false)
  return (
    <div className="arow">
      <button className="arow__h" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
        <Icon kind={group.kind} />
        <span className="arow__k">{actionLabel(group.kind)}</span>
        <span className="arow__d">{groupTarget(group)}</span>
        <Chevron open={open} className="arow__v" />
      </button>
      {open && (
        <div className="arow__body">
          {group.rows.map((row) => (
            <Row key={row.id} act={row} />
          ))}
        </div>
      )}
    </div>
  )
}

function Row({ act }: { act: Act }): React.JSX.Element {
  /* A subagent is a stack of its own, folded until asked for: its reads and
     searches are its business, and the thread is about what it came back with.
     Two components, so the row that gains its first child keeps its hooks. */
  return act.children.length > 0 ? <AgentRow act={act} /> : <LeafRow act={act} />
}

function LeafRow({ act }: { act: Act }): React.JSX.Element {
  const parts = sections(act)
  /* An edit shows what it changed as soon as it is on screen: the diff is the
     point of the row, and a chevron in front of it is a click for nothing. */
  const edited = act.kind === 'edit' ? editDiff(act.input) : null
  const [open, setOpen] = useDisclosed(useFoldKey(act.id), edited !== null)

  return (
    <div className="arow" data-failed={act.failed || undefined}>
      <button
        className="arow__h"
        aria-expanded={parts.length ? open : undefined}
        disabled={parts.length === 0}
        onClick={() => setOpen((was) => !was)}
      >
        <Icon kind={act.kind} />
        <span className="arow__k">{rowLabel(act)}</span>
        {act.target && <span className="arow__d">{act.target}</span>}
        {act.server && <span className="arow__s">{act.server}</span>}
        <Ran act={act} />
        <State act={act} open={open} hasBody={parts.length > 0} />
      </button>
      {open && parts.length > 0 && (
        <div className="arow__body">
          {edited ? (
            <EditCard diff={edited} failed={act.failed} error={act.output} />
          ) : act.kind === 'think' ? (
            <div className="arow__think">
              <Markdown source={act.output} />
            </div>
          ) : (
            <>
              {parts.map((part) => <Section key={part.label} label={part.label} body={part.body} />)}
              {isCut(act.output) && <Whole callId={act.id} output={act.output} />}
            </>
          )}
        </div>
      )}
    </div>
  )
}

/* A subagent's state and the CLI's count of how long it ran. The state the
   style reads is an id; the word beside it is the catalog's. */
function Ran({ act }: { act: Act }): React.JSX.Element | null {
  if (act.kind !== 'agent') return null
  const state = ranState(act)
  const said = t(`acts.state.${state}`)
  return (
    <span className="arow__t" data-state={state}>
      {act.durationMs != null ? `${said} · ${took(act.durationMs)}` : said}
    </span>
  )
}

/* An output the window got cut, whole on a click: the text in full, and the
   images it named, drawn only then. */
function Whole({ callId, output }: { callId: string; output: string }): React.JSX.Element | null {
  const where = useContext(WholeContext)
  const messageId = useContext(MessageContext)
  const [whole, setWhole] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const images = namedImages(output)
  if (!where || !messageId) return null
  if (whole !== null) {
    const shown = imagesOf(whole)
    const text = textOf(whole)
    return (
      <>
        {shown.map((src, at) => <Zoomable key={at} src={src} name={t('acts.imageName', { n: at + 1 })} className="asec__img" />)}
        {text.trim() && <Section label={t('acts.outputFull')} body={text} />}
      </>
    )
  }
  const said = images.length ? t('acts.showImages', { count: images.length, size: kb(images) }) : t('acts.showAll')
  return (
    <div className="asec">
      <button
        className="btn"
        onClick={() => {
          setError(null)
          void ask(() => commands.chatPartFull(where.projectId, where.conversationId, messageId, callId)).then((asked) => {
            if (asked.data?.kind === 'tool_result') setWhole(asked.data.output)
            else setError(asked.error ?? t('acts.notInTranscript'))
          })
        }}
      >
        {said}
      </button>
      {error && <span role="alert">{error}</span>}
    </div>
  )
}

const kb = (images: readonly { bytes: number }[]): string =>
  `${Math.round(images.reduce((all, one) => all + one.bytes, 0) / 1024)} KB`

/* What opens under a row. A command shows the command it ran and what came
   back; anything else shows the arguments it was given. Both are worth
   copying, so each carries its own button. */
interface Piece {
  readonly label: string
  readonly body: string
}

function sections(act: Act): readonly Piece[] {
  if (act.kind === 'think') return act.output.trim() ? [{ label: t('acts.thought'), body: act.output }] : []
  const out: Piece[] = []
  const command = act.kind === 'run' ? commandOf(act.input) : ''
  if (command) out.push({ label: t('acts.command'), body: command })
  /* The whole input, unless the command said all of it. */
  if (act.input.trim() && act.input.trim() !== '{}' && !(command && onlyCommand(act.input))) {
    out.push({ label: t('acts.input'), body: pretty(act.input) })
  }
  if (act.output.trim()) out.push({ label: t('acts.output'), body: act.output })
  return out
}

function commandOf(input: string): string {
  try {
    const value = JSON.parse(input) as Record<string, unknown>
    return typeof value.command === 'string' ? value.command : pretty(input)
  } catch {
    return input
  }
}

/* A command whose input holds nothing but it and the sentence the row already shows. */
function onlyCommand(input: string): boolean {
  try {
    return Object.keys(JSON.parse(input) as object).every((key) => key === 'command' || key === 'description')
  } catch {
    return true
  }
}

const pretty = (input: string): string => {
  try {
    return JSON.stringify(JSON.parse(input), null, 2)
  } catch {
    return input
  }
}

function Section({ label, body }: Piece): React.JSX.Element {
  const [copied, setCopied] = useState(false)
  const timer = useRef<number | null>(null)
  useEffect(() => () => {
    if (timer.current !== null) window.clearTimeout(timer.current)
  }, [])

  return (
    <div className="asec">
      <div className="asec__h">
        <span>{label}</span>
        <button
          className="asec__c"
          aria-label={copied ? t('acts.copied') : t('acts.copy', { what: label.toLowerCase() })}
          title={copied ? t('acts.copied') : t('acts.copy', { what: label.toLowerCase() })}
          onClick={() => {
            void navigator.clipboard?.writeText(body)
            setCopied(true)
            if (timer.current !== null) window.clearTimeout(timer.current)
            timer.current = window.setTimeout(() => setCopied(false), 2000)
          }}
        >
          {copied ? <Tick /> : <Copy />}
        </button>
      </div>
      <Scroller body={body} />
    </div>
  )
}

/* Long output scrolls inside the row rather than pushing the thread apart,
   and the fades say there is more above or below without a scrollbar having
   to appear over the text. */
function Scroller({ body }: { body: string }): React.JSX.Element {
  const opens = useContext(OpensPath)
  const box = useRef<HTMLPreElement>(null)
  const [edge, setEdge] = useState({ top: true, bottom: true })

  const measure = (view: HTMLPreElement): void => {
    const top = view.scrollTop <= 1
    const bottom = view.scrollHeight - view.scrollTop - view.clientHeight <= 1
    setEdge((was) => (was.top === top && was.bottom === bottom ? was : { top, bottom }))
  }

  useEffect(() => {
    if (box.current) measure(box.current)
  }, [body])

  return (
    <div className="ascroll">
      <pre className="ascroll__v" ref={box} onScroll={(event) => measure(event.currentTarget)}>
        <Pathed text={body} opens={opens} />
      </pre>
      <span className="ascroll__f ascroll__f--t" aria-hidden="true" data-on={!edge.top} />
      <span className="ascroll__f ascroll__f--b" aria-hidden="true" data-on={!edge.bottom} />
    </div>
  )
}

/* Running is a pulse, failed is a mark, and a row you can open says so with a
   chevron. A row that finished quietly says nothing at all. */
function State({ act, open, hasBody }: { act: Act; open: boolean; hasBody: boolean }): React.JSX.Element | null {
  if (hasBody) return <Chevron open={open} className="arow__v" />
  if (act.failed) {
    return (
      <svg className="arow__x" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.1" strokeLinecap="round" aria-label={t('acts.failed')}><circle cx="12" cy="12" r="9" /><path d="M12 8v5M12 16.5v.01" /></svg>
    )
  }
  if (!act.done) return <span className="arow__live" aria-label={t('acts.running')} />
  return null
}

function Chevron({ open, className }: { open: boolean; className?: string }): React.JSX.Element {
  return (
    <svg
      className={className}
      width="10"
      height="10"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ transform: open ? 'none' : 'rotate(-90deg)' }}
    >
      <path d="m6 9 6 6 6-6" />
    </svg>
  )
}

/* One glyph per kind, so the shape of the row is legible before the words
   are. Drawn rather than typed: a text glyph picks up whatever font the
   platform has for it and lands at a different size on every machine. */
const PATHS: Readonly<Record<Kind, React.JSX.Element>> = {
  think: <path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" />,
  run: <><path d="m4 17 6-5-6-5" /><path d="M12 19h8" /></>,
  edit: <><path d="M12 20h9" /><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" /></>,
  read: <><path d="M14 3v4a1 1 0 0 0 1 1h4" /><path d="M19 21H5a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h9l6 6v11a1 1 0 0 1-1 1Z" /></>,
  find: <><circle cx="11" cy="11" r="7" /><path d="m21 21-4.3-4.3" /></>,
  list: <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />,
  plan: <><path d="M8 6h13M8 12h13M8 18h13" /><path d="M3 6h.01M3 12h.01M3 18h.01" /></>,
  agent: <><circle cx="12" cy="8" r="4" /><path d="M4 21a8 8 0 0 1 16 0" /></>,
  tool: <path d="M14.7 6.3a4 4 0 0 1 5 5l-9.6 9.6a2.1 2.1 0 0 1-3-3l9.6-9.6a1 1 0 0 0-1.4-1.4L5.7 16.5a4 4 0 0 1-1-5.4" />,
}

const Icon = ({ kind }: { kind: Kind }): React.JSX.Element => (
  <svg className="arow__i" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
    {PATHS[kind]}
  </svg>
)

const Tick = (): React.JSX.Element => (
  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round"><path d="m5 13 4 4L19 7" /></svg>
)

const Copy = (): React.JSX.Element => (
  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="12" height="12" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></svg>
)
