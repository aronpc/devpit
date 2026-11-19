import { useState } from 'react'

import type { DevflowConfig, DevflowScreen, DevflowTeam, DevflowTeamChange } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'

/*
 * The team or company layers a project extends, each at a fixed ref. A ref
 * the person has not approved applies nothing: they look at what it changes
 * — every file, and the layer's devflow.yaml line by line — and approve it,
 * which pins that commit in devflow.lock. Nothing fetched runs.
 */

const short = (commit: string | null): string => (commit ? commit.slice(0, 12) : '')

export function teamSaid(team: DevflowTeam): string {
  switch (team.state) {
    case 'applied':
      return t('devflow.teams.applied', { wanted: team.wanted, commit: short(team.commit) })
    case 'fetch':
      return t('devflow.teams.fetch', { commit: short(team.commit) })
    default:
      return team.lockedRef
        ? t('devflow.teams.waitsLocked', { wanted: team.wanted, locked: team.lockedRef })
        : t('devflow.teams.waits', { wanted: team.wanted })
  }
}

export function DevflowTeams({
  projectId,
  config,
  onScreen,
}: {
  projectId: string
  config: DevflowConfig
  onScreen: (next: DevflowScreen, note: string) => void
}): React.JSX.Element | null {
  const [changes, setChanges] = useState<readonly DevflowTeamChange[] | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  if (config.teams.length === 0 && config.locked.length === 0) return null

  const look = (): void => {
    setBusy(true)
    void ask(() => commands.devflowTeamFetch(projectId)).then((answer) => {
      setChanges(answer.data?.changes ?? null)
      setProblem(answer.error)
      setBusy(false)
    })
  }
  const approve = (change: DevflowTeamChange): void =>
    void ask(() => commands.devflowTeamApprove(projectId, change.git, change.toCommit)).then((answer) => {
      setProblem(answer.error)
      if (answer.data) {
        setChanges(null)
        onScreen(answer.data, t('devflow.teams.approved', { name: change.name, ref: change.toRef }))
      }
    })

  return (
    <>
      <div className="acc__sub">{t('devflow.teams.layers')}</div>
      {config.teams.map((team) => (
        <div className="pref" key={team.git}>
          <span className="pref__body">
            <span className="pref__t">
              {team.name} · <code>{team.git}</code>
            </span>
            <span className={team.state === 'applied' ? 'pref__d' : 'pref__d voice__bad'}>{teamSaid(team)}</span>
          </span>
        </div>
      ))}
      {config.locked.length > 0 && <p className="acc__note">{t('devflow.teams.locked', { list: config.locked.join(', ') })}</p>}
      {config.teams.some((team) => team.state !== 'applied') && (
        <div className="voice__row">
          <button className="btn" disabled={busy} onClick={look}>
            {t('devflow.teams.look')}
          </button>
        </div>
      )}
      {changes?.map((change) => (
        <div className="pref pref--stack" key={change.git}>
          <div className="pref__body">
            <span className="pref__t">
              {t('devflow.teams.change', { name: change.name, from: change.fromRef ?? t('devflow.teams.nothingYet'), to: change.toRef, commit: short(change.toCommit) })}
            </span>
            {change.files.length === 0 && <span className="pref__d">{t('devflow.teams.noFiles')}</span>}
            {change.files.map((file) => (
              <span key={file} className="pref__d">
                {file}
              </span>
            ))}
            {change.diff.length > 0 && <pre className="errprev__body" aria-label={t('devflow.teams.diffLabel', { name: change.name })}>{change.diff.join('\n')}</pre>}
            <div className="voice__row">
              <button className="btn btn--go" onClick={() => approve(change)}>
                {t('devflow.teams.approve', { ref: change.toRef })}
              </button>
            </div>
          </div>
        </div>
      ))}
      {problem && <p className="pref__d voice__bad">{problem}</p>}
    </>
  )
}
