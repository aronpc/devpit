import { useCallback, useEffect, useRef, useState } from 'react'

import type { DevflowAsk, DevflowChange, DevflowError, DevflowScreen } from '../gen/bindings'
import { t } from '../i18n'
import { onCameBack } from './cameBack'
import { devflowQuestion } from './DevflowAsks'
import { YAML_FILE, type Edit } from './devflowEdit'
import { DevflowJev } from './DevflowJev'
import { DevflowReviewers } from './DevflowReviewers'
import { DevflowSetup } from './DevflowSetup'
import { DevflowBinaryRow, DevflowMcpRow, DevflowPluginRow } from './DevflowStandalone'
import { DevflowStages } from './DevflowStages'
import { DevflowTeams } from './DevflowTeams'
import { inOrder } from './inOrder'
import { ask, commands } from './live'
import { ReviewersAccount } from './ReviewersAccount'
import { useShell } from './useShell'
import { onCarried } from './window'

/*
 * A project's Devflow, as its devflow.yaml says it: a setup when there is no
 * file, the file and an Activate when it is off, an editor when it is on, and
 * the error when it does not read. The file is the source of truth: every
 * edit writes it through the same engine the MCP tools use, and a change made
 * anywhere else is read again and said.
 *
 * Progress lives on the board and the cards, not here.
 */

/** How often an open screen looks at the file again, for an edit made in an
 *  editor that never took the window's focus away. */
export const LOOK_EVERY_MS = 4000

/** Where the file is refused and why, as the MCP's status says it. */
export function errorSaid(error: DevflowError | null): string {
  if (!error) return ''
  const at = [error.line != null ? t('devflow.pane.errorLine', { line: error.line }) : null, error.field ? `\`${error.field}\`` : null].filter(Boolean).join(', ')
  return at ? `${at}: ${error.message}` : error.message
}

/** Said when the file changed somewhere else. A function, not a constant: it is said in the language of the moment. */
export const outsideSaid = (): string => t('devflow.pane.outside')

function Waiting({ asks, onAnswer }: { asks: readonly DevflowAsk[]; onAnswer: (id: string, yes: boolean) => void }): React.JSX.Element | null {
  if (asks.length === 0) return null
  return (
    <>
      {asks.map((one) => {
        const said = devflowQuestion(one)
        return (
          <div className="pref" role="alert" key={one.id}>
            <span className="pref__body">
              <span className="pref__t">{said.question}</span>
              <span className="pref__d">{said.why}</span>
            </span>
            <button className="btn" onClick={() => onAnswer(one.id, false)}>
              {t('devflow.notNow')}
            </button>
            <button className="btn btn--go" onClick={() => onAnswer(one.id, true)}>
              {said.yes}
            </button>
          </div>
        )
      })}
    </>
  )
}

export function DevflowPane(): React.JSX.Element {
  const { project, show, openPrefs, active } = useShell()
  const projectId = project?.id ?? null
  const inFront = active?.kind === 'devflow'
  const [screen, setScreen] = useState<DevflowScreen | null>(null)
  const [note, setNote] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  /* The text this screen last read or wrote: anything else is a change made elsewhere. */
  const known = useRef<string | null | undefined>(undefined)

  const took = useCallback((next: DevflowScreen, said: string | null) => {
    known.current = next.yaml
    setScreen(next)
    setNote(said)
  }, [])

  /* Reads and writes in one queue: a read heard while an edit is on its way
     waits for it, so the screen's own edit is never mistaken for an outside one. */
  const look = useCallback(() => {
    if (!projectId) return
    void inOrder(`devflow:${projectId}`, async () => {
      const answer = await ask(() => commands.devflowScreen(projectId))
      if (answer.data) {
        const outside = known.current !== undefined && answer.data.yaml !== known.current
        known.current = answer.data.yaml
        setScreen(answer.data)
        if (outside) setNote(outsideSaid())
      }
      setProblem(answer.error)
    })
  }, [projectId])

  useEffect(() => {
    known.current = undefined
    setScreen(null)
    setNote(null)
    look()
    const back = onCameBack(look)
    const changed = onCarried<string>('devflow:changed', look)
    const asked = onCarried<null>('devflow:asked', look)
    return () => {
      back()
      changed()
      asked()
    }
  }, [look])

  const missing = screen?.state === 'missing'
  /* The pane stays mounted behind other tabs, where nobody reads the file; back
     in front it looks at once rather than a beat late. */
  const behind = useRef(false)
  useEffect(() => {
    if (!inFront) {
      behind.current = true
      return
    }
    if (behind.current) {
      behind.current = false
      look()
    }
    if (missing) return
    const every = window.setInterval(() => document.visibilityState === 'visible' && look(), LOOK_EVERY_MS)
    return () => window.clearInterval(every)
  }, [look, missing, inFront])

  const edit: Edit = (path, value) => {
    if (!projectId) return
    void inOrder(`devflow:${projectId}`, async () => {
      const answer = await ask(() => commands.devflowEdit(projectId, path, JSON.stringify(value)))
      if (answer.data) took(answer.data, null)
      setProblem(answer.error)
    })
  }
  const askFor = (change: DevflowChange): void => {
    if (projectId) void ask(() => commands.devflowAsk(projectId, change)).then(() => look())
  }
  const answer = (id: string, yes: boolean): void =>
    void ask(() => commands.devflowAnswer(id, yes)).then((answered) => {
      setProblem(answered.error)
      look()
    })
  const openFile = (): void => {
    if (screen) show('file', { id: `file:${screen.file}`, path: screen.file, title: YAML_FILE })
  }

  const body = (): React.JSX.Element | null => {
    if (!screen) return null
    switch (screen.state) {
      case 'missing':
        return <DevflowSetup screen={screen} onScreen={took} />
      case 'invalid':
        return (
          <>
            <div className="pref dfstate" data-tone="bad" role="alert">
              <span className="dfstate__dot" aria-hidden />
              <span className="pref__body">
                <span className="pref__t">{t('devflow.pane.invalidTitle')}</span>
                <span className="pref__d voice__bad">{errorSaid(screen.error)}</span>
                <span className="pref__d">{t('devflow.pane.invalidBody')}</span>
              </span>
              <button className="btn" onClick={openFile}>
                {t('devflow.openInEditor')}
              </button>
            </div>
            {screen.yaml && <pre className="errprev__body">{screen.yaml}</pre>}
          </>
        )
      case 'disabled':
        return (
          <>
            <div className="pref dfstate" data-tone="off">
              <span className="dfstate__dot" aria-hidden />
              <span className="pref__body">
                <span className="pref__t">{t('devflow.pane.disabledTitle')}</span>
                <span className="pref__d">{t('devflow.pane.disabledBody')}</span>
              </span>
              <button className="btn" onClick={openFile}>
                {t('devflow.openInEditor')}
              </button>
              {/* Gone while the confirmation it would raise is already here. */}
              {!screen.waiting.some((one) => one.change === 'enable') && (
                <button className="btn btn--go" onClick={() => askFor('enable')}>
                  {t('devflow.pane.activate')}
                </button>
              )}
            </div>
            <Waiting asks={screen.waiting} onAnswer={answer} />
            {screen.config && <DevflowTeams projectId={screen.projectId} config={screen.config} onScreen={took} />}
            {screen.yaml && <pre className="errprev__body" aria-label={YAML_FILE}>{screen.yaml}</pre>}
          </>
        )
      case 'active': {
        const config = screen.config
        if (!config) return null
        return (
          <>
            <div className="pref dfstate" data-tone="on">
              <span className="dfstate__dot" aria-hidden />
              <span className="pref__body">
                <span className="pref__t">{t('devflow.pane.activeTitle')}</span>
                <span className="pref__d">{t('devflow.pane.activeBody')}</span>
              </span>
              <button className="btn" onClick={openFile}>
                {t('devflow.openInEditor')}
              </button>
              <button className="btn" onClick={() => edit('enabled', false)}>
                {t('devflow.pane.deactivate')}
              </button>
            </div>
            <Waiting asks={screen.waiting} onAnswer={answer} />
            <DevflowTeams projectId={screen.projectId} config={config} onScreen={took} />
            <div className="acc__sub">{t('devflow.pane.stages')}</div>
            <DevflowStages config={config} edit={edit} projectId={screen.projectId} arrange />
            <div className="acc__sub">{t('devflow.pane.review')}</div>
            <DevflowReviewers config={config} edit={edit} />
            <ReviewersAccount projectId={screen.projectId} />
            <div className="acc__sub">{t('devflow.pane.jev')}</div>
            <DevflowJev config={config} edit={edit} projectId={screen.projectId} onEnforce={() => askFor('enforce')} />
          </>
        )
      }
    }
  }

  return (
    <div className="scroll">
      <div className="prefs__in dfscreen">
        <h1 className="prefs__h">
          {t('devflow.name')}{project && <span className="dfscreen__project">{project.name}</span>}
        </h1>
        {!project && <p className="acc__note">{t('devflow.pane.noProject')}</p>}
        {screen && !screen.globallyOn && (
          <div className="pref">
            <span className="pref__body">
              <span className="pref__t">{t('devflow.pane.offEverywhereTitle')}</span>
              <span className="pref__d">{t('devflow.pane.offEverywhereBody')}</span>
            </span>
            <button className="btn" onClick={() => openPrefs('devflow')}>
              {t('devflow.pane.settings')}
            </button>
          </div>
        )}
        {note && (
          <p className="acc__note" role="status">
            {note}
          </p>
        )}
        {problem && <p className="pref__d voice__bad">{problem}</p>}
        {body()}
        {screen && (
          <>
            <div className="acc__sub">{t('devflow.pane.outsideDevpit')}</div>
            <DevflowPluginRow projectId={screen.projectId} devpitOn={screen.state === 'active'} onDeactivate={() => edit('enabled', false)} />
            <DevflowBinaryRow />
            <DevflowMcpRow />
          </>
        )}
      </div>
    </div>
  )
}
