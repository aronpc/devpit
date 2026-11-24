import { useCallback, useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { shellWord, useFileDrop } from '../shell/fileDrop'

import { ask, commands } from '../shell/live'
import { headline, labelled, moodOf, moodOfAll, nameOf } from './sessions'
import { ROWS, WAKE, rectOf, sizeOf, spanning, type Rect, type View } from './shape'
import { useDrag } from './useDrag'
import { useIsland } from './useIsland'
import { Asking, Capsule, Detail, Head, Rows, whereOf } from './views'
import './island.css'

/*
 * The whole of the island window.
 *
 * The window is fixed and mostly glass; what people see is one capsule
 * floating under the top of the screen, grown and shrunk here. Rust only
 * learns where it is, so that the glass around it lets clicks through.
 */

export function IslandWindow(): React.JSX.Element {
  const { island, sessions, questions, chosen, nudge, choose, answer, now, sound, setSound, closesAt } = useIsland()
  /* While a session is open, only its own question takes the view: another
     session asking waits as a badge, so the one being read is not pulled
     away. With none open, the first question is the view. */
  const reading = chosen?.sessionId ?? null
  const question = (reading ? questions.find((one) => one.sessionId === reading) : questions[0]) ?? null
  const elsewhere = reading ? questions.filter((one) => one.sessionId !== reading).length : 0
  const askingIn = new Set(questions.map((one) => one.sessionId))
  const view: View = question ? 'asking' : chosen ? 'session' : 'overview'
  const size = sizeOf(island.mode, view, sessions.length)
  const mood = moodOfAll(sessions, questions, now)
  const was = useRef<Rect>(WAKE)
  const drag = useDrag()
  const shape = useRef<HTMLDivElement>(null)
  const [dropNote, setDropNote] = useState<string | null>(null)

  /* Files dropped on a session go to its agent's prompt; dropped anywhere
     else, to the one session open in the detail, or the only one there is. */
  const onDrop = useCallback(
    (paths: readonly string[], under: Element | null) => {
      const named = under?.closest<HTMLElement>('[data-session]')?.dataset.session
      const sessionId = named ?? chosen?.sessionId ?? (sessions.length === 1 ? sessions[0].sessionId : null)
      if (!sessionId) {
        setDropNote(t('island.drop.noTarget'))
        return
      }
      void ask(() => commands.islandDrop(sessionId, `${paths.map(shellWord).join(' ')} `)).then((answer) =>
        setDropNote(answer.error ?? t('island.drop.inPrompt')),
      )
    },
    [chosen, sessions],
  )
  const dropping = useFileDrop(shape, onDrop)

  useEffect(() => {
    if (!dropNote) return
    const gone = window.setTimeout(() => setDropNote(null), 3000)
    return () => window.clearTimeout(gone)
  }, [dropNote])

  /* While one size turns into the other, both take the mouse; once it has
     settled, only the new one does. Keyed on the numbers, not the object,
     which is new on every render. */
  useEffect(() => {
    const shown = island.mode !== 'hidden'
    const target = shown ? rectOf(size) : WAKE
    void ask(() => commands.islandShape(spanning(was.current, target), shown))
    const settled = window.setTimeout(() => {
      was.current = target
      void ask(() => commands.islandShape(target, shown))
    }, 460)
    return () => window.clearTimeout(settled)
  }, [island.mode, size.width, size.height])

  /* Folding back to the capsule forgets the chosen session: the next open
     starts from every session, which is where somebody coming back looks. */
  useEffect(() => {
    if (island.mode !== 'expanded') choose(null)
  }, [island.mode, choose])

  const asked = question ? (sessions.find((one) => one.sessionId === question.sessionId) ?? null) : null
  const labels = labelled(sessions)
  const projects = new Set(sessions.map((one) => one.projectId).filter(Boolean)).size
  const across =
    sessions.length === 0
      ? t('island.overview.none')
      : t('island.overview.across', {
          sessions: t('island.overview.sessions', { count: sessions.length }),
          projects: t('island.overview.projects', { count: Math.max(1, projects) }),
        }) + (sessions.length > ROWS ? ` · ${t('island.overview.more', { count: sessions.length - ROWS })}` : '')
  const pin = (): void => nudge({ kind: 'pin', pinned: !island.pinned })
  const fold = (): void => nudge({ kind: 'close' })

  return (
    <div className="isl" data-mode={island.mode}>
      <div className="isl-wake" onMouseEnter={() => nudge({ kind: 'enter' })} onDragEnter={() => nudge({ kind: 'open' })} />
      <div
        ref={shape}
        data-drop={dropping ? 'true' : undefined}
        className="isl-shape"
        data-mode={island.mode}
        data-mood={mood}
        style={{ width: size.width, height: size.height, borderRadius: size.radius }}
        onMouseEnter={() => nudge({ kind: 'enter' })}
        onMouseLeave={() => nudge({ kind: 'leave' })}
        onPointerDown={drag.onPointerDown}
        onPointerMove={drag.onPointerMove}
        onPointerUp={drag.onPointerUp}
        onClick={() => !drag.wasDrag() && island.mode === 'compact' && nudge({ kind: 'open' })}
      >
        {dropNote && <div className="isl-dropnote">{dropNote}</div>}
        {island.mode === 'expanded' && <Closing at={closesAt} />}
        {island.mode === 'compact' && <Capsule sessions={sessions} questions={questions} mood={mood} now={now} />}
        {island.mode === 'expanded' && (
          <div className="isl-open" data-view={view}>
            {view === 'asking' && question && (
              <>
                <Head
                  mood="asking"
                  title={t('island.asking.title', { who: asked ? (labels.get(asked.sessionId) ?? nameOf(asked)) : (question.project ?? t('island.asking.anAgent')) })}
                  sub={question.from === 'terminal' ? t('island.asking.fromTerminal') : t('island.asking.fromChat')}
                  color={asked?.color}
                  pinned={island.pinned}
                  onPin={pin}
                  onFold={fold}
                  sound={sound}
                  onSound={() => setSound(!sound)}
                />
                <Asking question={question} onAnswer={(verdict, chosen) => answer(question, verdict, chosen)} />
              </>
            )}
            {view === 'session' && chosen && (
              <>
                <Head
                  mood={moodOf(chosen, now)}
                  title={labels.get(chosen.sessionId) ?? nameOf(chosen)}
                  sub={whereOf(chosen, now)}
                  color={chosen.color}
                  onBack={() => choose(null)}
                  waiting={elsewhere}
                  pinned={island.pinned}
                  onPin={pin}
                  onFold={fold}
                  sound={sound}
                  onSound={() => setSound(!sound)}
                />
                <Detail session={chosen} now={now} />
              </>
            )}
            {view === 'overview' && (
              <>
                <Head mood={mood} title={headline(sessions, questions)} sub={across} pinned={island.pinned} onPin={pin} onFold={fold} sound={sound} onSound={() => setSound(!sound)} />
                <Rows sessions={sessions} now={now} asking={askingIn} onChoose={(session) => choose(session.sessionId)} />
              </>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

/** How long before the island folds the bar shows it coming. */
export const CLOSING_SHOWN_MS = 10_000

/* A thin bar that runs out as the island is about to fold on its own, so the
   fold is never a surprise. Gone while the cursor is on it or it is pinned —
   there is no fold coming then. */
function Closing({ at }: { at: number | null }): React.JSX.Element | null {
  const [, again] = useState(0)
  const left = at === null ? null : at - Date.now()
  /* Asked to draw once more when the last stretch begins. */
  useEffect(() => {
    if (left === null || left <= CLOSING_SHOWN_MS) return
    const wait = window.setTimeout(() => again((n) => n + 1), left - CLOSING_SHOWN_MS)
    return () => window.clearTimeout(wait)
  }, [left])
  if (left === null || left <= 0 || left > CLOSING_SHOWN_MS) return null
  return (
    <div
      key={at}
      className="isl-closing"
      aria-hidden
      style={{ '--from': `${(left / CLOSING_SHOWN_MS) * 100}%`, animationDuration: `${left}ms` } as React.CSSProperties}
    />
  )
}
