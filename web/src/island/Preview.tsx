import { useEffect, useState } from 'react'

import type { IslandSession, IslandStep, Touch } from '../gen/bindings'
import { t } from '../i18n'
import { ofPath } from '../shell/languages'
import { ask, commands } from '../shell/live'
import { Painted } from '../shell/Painted'
import { AskedHere } from './AskedHere'
import { verbOf } from './sessions'

/*
 * What a step touches, drawn beside the session: the change an edit makes,
 * the lines a read reads, the command a shell runs.
 *
 * An edit and a write carry their text in the hook, so they are drawn from
 * that and nothing is read. A read is the one that asks for the file, through
 * `island_peek`, which reads only inside the session's own project.
 */

/** How many lines the panel has room for. */
export const LINES = 11

/** The lines of an edit, marked: what goes, then what comes. */
export function changed(before: string, after: string): { mark: '-' | '+'; text: string }[] {
  const gone = before.split('\n').map((text) => ({ mark: '-' as const, text }))
  const come = after === '' ? [] : after.split('\n').map((text) => ({ mark: '+' as const, text }))
  const room = Math.max(1, Math.floor(LINES / 2))
  return [...gone.slice(0, come.length === 0 ? LINES : room), ...come.slice(0, LINES - Math.min(gone.length, room))]
}

/** The lines a read covers, numbered from where it started. */
export function linesFrom(text: string, offset: number | null): { line: number; text: string }[] {
  const from = Math.max(1, offset ?? 1)
  return text
    .split('\n')
    .slice(from - 1, from - 1 + LINES)
    .map((one, at) => ({ line: from + at, text: one }))
}

const pathOf = (touch: Touch): string | null => ('path' in touch ? touch.path : null)
const baseName = (path: string): string => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path

/** A path from the session's own folder, the way the file tree shows it. */
export function fromRoot(path: string, root: string | null): string {
  const base = root?.replace(/[\\/]+$/, '')
  return base && path.startsWith(`${base}/`) ? path.slice(base.length + 1) : path
}

export function Preview({ session, step }: { session: IslandSession; step: IslandStep | null }): React.JSX.Element {
  const touch = step?.touch ?? null
  const path = touch ? pathOf(touch) : null
  const language = path ? ofPath(path) : null

  return (
    <div className="isl-peek">
      <div className="isl-peek__bar">
        {path ? (
          <>
            <span className="isl-peek__tab">{baseName(path)}</span>
            {/* Cut from the left so the file end stays; the mark keeps the slashes in order. */}
            <span className="isl-peek__path">{`\u200e${fromRoot(path, session.root)}`}</span>
          </>
        ) : (
          <span className="isl-peek__tab">{step ? verbOf(step.tool) : t('island.preview.nothingYet')}</span>
        )}
      </div>
      <div className="isl-peek__code">
        {!touch && <div className="isl-peek__none">{step ? t('island.preview.nothingForStep') : t('island.preview.stepsShowHere')}</div>}
        {touch?.kind === 'edit' &&
          changed(touch.before, touch.after).map((one, at) => (
            <div className="isl-peek__ln" data-mark={one.mark} key={at}>
              <span className="isl-peek__no">{one.mark}</span>
              <span className="isl-peek__tx">
                <Painted text={one.text} language={language} />
              </span>
            </div>
          ))}
        {touch?.kind === 'write' &&
          linesFrom(touch.after, 1).map((one) => (
            <div className="isl-peek__ln" data-mark="+" key={one.line}>
              <span className="isl-peek__no">{one.line}</span>
              <span className="isl-peek__tx">
                <Painted text={one.text} language={language} />
              </span>
            </div>
          ))}
        {touch?.kind === 'read' && <ReadLines session={session} touch={touch} language={language} />}
        {touch?.kind === 'run' && (
          <div className="isl-peek__ln isl-peek__ln--run">
            <span className="isl-peek__no">$</span>
            <span className="isl-peek__tx">{touch.command}</span>
          </div>
        )}
        {touch?.kind === 'ask' && (
          <AskedHere session={session} questions={touch.questions} waiting={session.state === 'waiting' && !step?.done} />
        )}
        {touch?.kind === 'plan' && <div className="isl-peek__plan">{touch.plan}</div>}
      </div>
    </div>
  )
}

function ReadLines({
  session,
  touch,
  language,
}: {
  session: IslandSession
  touch: Extract<Touch, { kind: 'read' }>
  language: ReturnType<typeof ofPath>
}): React.JSX.Element {
  const [text, setText] = useState<string | null>(null)
  const [refused, setRefused] = useState<string | null>(null)

  useEffect(() => {
    let current = true
    setText(null)
    setRefused(null)
    void ask(() => commands.islandPeek(session.sessionId, touch.path)).then((answer) => {
      if (!current) return
      if (answer.data?.text != null) setText(answer.data.text)
      else setRefused(answer.data?.notShown ?? answer.error ?? t('island.preview.notShown'))
    })
    return () => {
      current = false
    }
  }, [session.sessionId, touch.path])

  if (refused) return <div className="isl-peek__none">{refused}</div>
  if (text === null) return <div className="isl-peek__none">{t('island.preview.reading')}</div>
  return (
    <>
      {linesFrom(text, touch.offset).map((one, at) => (
        <div className="isl-peek__ln" data-first={at === 0 && touch.offset !== null ? 'true' : undefined} key={one.line}>
          <span className="isl-peek__no">{one.line}</span>
          <span className="isl-peek__tx">
            <Painted text={one.text} language={language} />
          </span>
        </div>
      ))}
    </>
  )
}
