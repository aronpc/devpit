import { useEffect, useState } from 'react'

import type { Guardrail, GuardrailMode, Guardrails } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { useShell } from './useShell'

/*
 * Settings → Orchestrator: the confirmations each orchestrator needs from the
 * person, and when each one asks. What the person says themselves — "stop X",
 * a ✅ — works whatever the mode.
 */

const GUARDRAILS: Readonly<Record<Guardrail, readonly [string, string]>> = {
  stop: ['workspace.guardrail.stop', 'workspace.guardrail.stopSaid'],
  forceStop: ['workspace.guardrail.force', 'workspace.guardrail.forceSaid'],
  sendDraft: ['workspace.guardrail.send', 'workspace.guardrail.sendSaid'],
}

const MODES: ReadonlyArray<readonly [GuardrailMode, string]> = [
  ['follow', 'workspace.guardrail.follow'],
  ['always', 'workspace.guardrail.always'],
  ['never', 'workspace.guardrail.never'],
]

export function GuardrailSettings(): React.JSX.Element {
  const { projects } = useShell()
  const orchestrators = projects.filter((one) => one.orchestrator)
  const [chosen, setChosen] = useState<string | null>(null)
  const project = orchestrators.find((one) => one.id === chosen) ?? orchestrators[0] ?? null
  const [guardrails, setGuardrails] = useState<Guardrails | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const id = project?.id ?? null

  useEffect(() => {
    setGuardrails(null)
    if (!id) return
    void ask(() => commands.orchestratorGuardrails(id)).then((said) => {
      setGuardrails(said.data)
      setProblem(said.error)
    })
  }, [id])

  const pick = (guardrail: Guardrail, mode: GuardrailMode): void => {
    if (!id) return
    void ask(() => commands.orchestratorGuardrailSet(id, guardrail, mode)).then((said) => {
      if (said.data) setGuardrails(said.data)
      setProblem(said.error)
    })
  }

  if (!project) return <p className="acc__note">{t('workspace.guardrail.none')}</p>
  return (
    <>
      <div className="acc__sub">{t('workspace.guardrail.title')}</div>
      <p className="acc__note">
        {t('workspace.guardrail.note')}
      </p>
      {orchestrators.length > 1 && (
        <div className="pref">
          <span className="pref__body"><span className="pref__t">{t('workspace.guardrail.orchestrator')}</span></span>
          <select aria-label={t('workspace.guardrail.orchestratorLabel')} value={project.id} onChange={(change) => setChosen(change.target.value)}>
            {orchestrators.map((one) => <option key={one.id} value={one.id}>{one.name}</option>)}
          </select>
        </div>
      )}
      {problem && <p className="acc__note voice__bad" role="alert">{problem}</p>}
      {guardrails?.rows.map((row) => {
        const [title, what] = GUARDRAILS[row.guardrail].map((key) => t(key))
        return (
          <div className="pref" key={row.guardrail}>
            <span className="pref__body">
              <span className="pref__t">{title}</span>
              <span className="pref__d">{what}</span>
            </span>
            <select aria-label={title} value={row.mode} onChange={(change) => pick(row.guardrail, change.target.value as GuardrailMode)}>
              {MODES.map(([mode, label]) => <option key={mode} value={mode}>{t(label)}</option>)}
            </select>
          </div>
        )
      })}
    </>
  )
}
