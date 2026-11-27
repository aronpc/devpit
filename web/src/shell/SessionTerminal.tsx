import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'

import type { LiveSession } from '../gen/bindings'
import { stateWord } from './liveStatus'
import { Leaf, PANE_FREED } from './Leaf'
import { ask, commands } from './live'
import { branchWords, SessionChangesPanel } from './SessionChangesPanel'
import { SessionName } from './SessionName'
import { useSessionChanges } from './useSessionChanges'
import { useShell } from './useShell'
import { formatRelative } from '../i18n/format'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * A session's own terminal, opened over the orchestrator's chat: the same
 * pane its tab shows, live, and typed into as the person — so it answers what
 * the session asks, which a message from the orchestrator never can.
 *
 * A pane is drawn by one view at a time. Opening it here takes it from its
 * tab; closing this gives it back, and the tab attaches again.
 *
 * Sized by its edges and corners and always centred, so a drag grows it on
 * both sides at once; the size is kept for the next one opened.
 */

const KEPT = 'devpit.sessionTerminal.size'
const SIDE = 'devpit.sessionTerminal.changes'
const SMALLEST = { width: 560, height: 320 }
/* Room left around it, so the window behind stays in view. */
const MARGIN = 32

interface Size {
  width: number
  height: number
}

const fits = (size: Size): Size => ({
  width: Math.round(Math.min(Math.max(size.width, SMALLEST.width), window.innerWidth - MARGIN)),
  height: Math.round(Math.min(Math.max(size.height, SMALLEST.height), window.innerHeight - MARGIN)),
})

function keptSize(): Size {
  try {
    const kept = JSON.parse(localStorage.getItem(KEPT) ?? 'null') as Size | null
    if (kept && kept.width > 0 && kept.height > 0) return fits(kept)
  } catch {
    // Unread, it opens at its default.
  }
  return fits({ width: Math.min(1100, window.innerWidth * 0.86), height: Math.min(760, window.innerHeight * 0.82) })
}

/* Which way an edge grows it: -1 for the left or top, 1 for the right or
   bottom, 0 for an axis the handle does not move. */
type Edge = { x: -1 | 0 | 1; y: -1 | 0 | 1; name: string }
const EDGES: readonly Edge[] = [
  { x: 1, y: 0, name: 'e' },
  { x: -1, y: 0, name: 'w' },
  { x: 0, y: 1, name: 's' },
  { x: 0, y: -1, name: 'n' },
  { x: 1, y: 1, name: 'se' },
  { x: -1, y: 1, name: 'sw' },
  { x: 1, y: -1, name: 'ne' },
  { x: -1, y: -1, name: 'nw' },
]

/** How long since, in the fewest characters that still say it. */
const since = (at: number | null): string | null => {
  if (!at) return null
  const minutes = Math.round((Date.now() - at) / 60_000)
  return minutes < 1
    ? t('sessions.justNow')
    : minutes < 60
      ? formatRelative(-minutes, 'minute', { style: 'narrow' })
      : formatRelative(-Math.round(minutes / 60), 'hour', { style: 'narrow' })
}

const sideKept = (): boolean => {
  try {
    return localStorage.getItem(SIDE) === 'open'
  } catch {
    return false
  }
}

export function SessionTerminal({
  session,
  profileId = null,
  onGo,
  onClose,
}: {
  session: LiveSession
  /** The orchestrator's profile, which a rename is asked through. */
  profileId?: string | null
  onGo: () => void
  onClose: () => void
}): React.JSX.Element | null {
  const pane = session.pane
  const { show, openCard } = useShell()
  const [name, setName] = useState(session.name)
  useEffect(() => setName(session.name), [session.name])
  const { changes, problem, reload } = useSessionChanges(session)
  /* Folded by default: the terminal is what the window is for. */
  const [side, setSide] = useState(sideKept)
  const toggleSide = (): void =>
    setSide((was) => {
      try {
        localStorage.setItem(SIDE, was ? 'shut' : 'open')
      } catch {
        // Unsaved, it is still open now.
      }
      return !was
    })
  /* The window's own title says whose terminal is in front. */
  useEffect(() => {
    const was = document.title
    document.title = `${name} — devpit`
    return () => {
      document.title = was
    }
  }, [name])
  const [size, setSize] = useState<Size>(keptSize)
  const [dragging, setDragging] = useState(false)
  /* Closing the terminal itself — not just this view of it — asks once. */
  const [ending, setEnding] = useState(false)
  const [refused, setRefused] = useState<string | null>(null)
  /* A pane that was closed is not handed back: its tab would attach again to
     a pane that is gone. */
  const closed = useRef<string | null>(null)
  const end = (): void => {
    if (!pane) return
    void ask(() => commands.terminalClose(pane.projectId, pane.paneId)).then((done) => {
      if (done.error) return setRefused(done.error)
      closed.current = pane.paneId
      onClose()
    })
  }
  const latest = useRef(size)
  latest.current = size

  useEffect(() => {
    if (!pane) return
    return () => {
      if (closed.current !== pane.paneId) window.dispatchEvent(new CustomEvent(PANE_FREED, { detail: pane.paneId }))
    }
  }, [pane])

  /* A window made smaller keeps it inside. */
  useEffect(() => {
    const kept = (): void => setSize((was) => fits(was))
    window.addEventListener('resize', kept)
    return () => window.removeEventListener('resize', kept)
  }, [])

  const grab = (edge: Edge) => (event: React.PointerEvent<HTMLSpanElement>): void => {
    event.preventDefault()
    const from = { x: event.clientX, y: event.clientY, ...latest.current }
    const handle = event.currentTarget
    handle.setPointerCapture(event.pointerId)
    setDragging(true)
    /* Centred, an edge moved by d grows it by 2d. */
    const move = (moved: PointerEvent): void =>
      setSize(
        fits({
          width: from.width + edge.x * 2 * (moved.clientX - from.x),
          height: from.height + edge.y * 2 * (moved.clientY - from.y),
        }),
      )
    const done = (): void => {
      handle.removeEventListener('pointermove', move)
      handle.removeEventListener('pointerup', done)
      handle.removeEventListener('pointercancel', done)
      setDragging(false)
      try {
        localStorage.setItem(KEPT, JSON.stringify(latest.current))
      } catch {
        // Unsaved, it is still this size now.
      }
    }
    handle.addEventListener('pointermove', move)
    handle.addEventListener('pointerup', done)
    handle.addEventListener('pointercancel', done)
  }

  if (!pane) return null

  return createPortal(
    <div className="ask sterm__scrim" data-open="true" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="sterm" role="dialog" aria-modal="true" aria-label={t('sessions.term.label', { name: session.name })} data-dragging={dragging ? 'true' : undefined} style={{ width: size.width, height: size.height }}>
        <header className="sterm__bar">
          <i className="deleg__dot" data-state={session.waiting ? 'waiting' : session.status === 'busy' ? 'busy' : 'idle'} />
          <SessionName profileId={profileId} name={name} onRenamed={setName} />
          <span className="sterm__where" title={session.cwd}>
            {[stateWord(session.waiting ? 'waiting' : session.status === 'busy' ? 'busy' : 'idle'), since(session.since), session.projectName].filter(Boolean).join(' · ')}
            {changes && <span className="sterm__branch">{branchWords(changes)}</span>}
          </span>
          <span className="sterm__size" aria-hidden="true">
            {dragging ? `${size.width} × ${size.height}` : ''}
          </span>
          {ending ? (
            <>
              <span className="sterm__warn">{refused ?? t('sessions.term.closeAsk')}</span>
              <button className="sterm__btn" onClick={() => (setEnding(false), setRefused(null))}>
                {t('sessions.keep')}
              </button>
              <button className="sterm__btn sterm__btn--bad" onClick={end}>
                {t('sessions.term.closeNow')}
              </button>
            </>
          ) : (
            <button className="sterm__btn" onClick={() => setEnding(true)} title={t('sessions.term.closeTitle')}>
              {t('sessions.term.close')}
            </button>
          )}
          {session.cardId && (
            <button className="sterm__btn" onClick={() => (onClose(), show('board'), openCard(session.cardId!))} title={t('sessions.term.cardTitle')}>
              {t('sessions.term.card')}
            </button>
          )}
          <button className="sterm__btn" onClick={() => void writeText(name).catch(() => undefined)} title={t('sessions.term.copyTitle')}>
            {t('sessions.term.copy')}
          </button>
          <button className="sterm__btn" data-on={side ? 'true' : undefined} onClick={toggleSide} title={t('sessions.term.changesTitle')}>
            {t('sessions.changes.title')}{changes && changes.changes.length > 0 ? ` ${changes.changes.length}` : ''}
          </button>
          <button className="sterm__btn" onClick={onGo} title={t('sessions.term.goTitle')}>
            {t('sessions.term.go')}
            <Icon size="s"><path d="M7 17 17 7M8 7h9v9" /></Icon>
          </button>
          <button className="sterm__x" onClick={onClose} aria-label={t('sessions.term.hide')} title={t('sessions.term.hideTitle')}>
            <Icon size="s"><path d="M18 6 6 18M6 6l12 12" /></Icon>
          </button>
        </header>
        <div className="sterm__main">
          <div className="sterm__body">
            <Leaf paneId={pane.paneId} projectId={pane.projectId} cwd={session.cwd} />
          </div>
          {side && <SessionChangesPanel changes={changes} problem={problem} onReload={reload} />}
        </div>
        {EDGES.map((edge) => (
          <span key={edge.name} className="sterm__grip" data-edge={edge.name} onPointerDown={grab(edge)} aria-hidden="true" />
        ))}
      </div>
    </div>,
    document.querySelector('.app') ?? document.body,
  )
}
