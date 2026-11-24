import { useEffect, useState } from 'react'

import type { DevflowPrefs } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'
import { useKnownAgents } from './useKnownAgents'

/*
 * Devflow on this machine: offered to every project or to none, and where a
 * new project's setup starts. Each project's own Devflow is its sidebar's
 * Devflow screen.
 *
 * Codex reads a plugin from its own configuration rather than from a launch
 * line, so adding Devflow's skills to it writes Codex's global settings: that
 * is offered only where Codex is installed, and done only on a yes.
 */

function CodexInstall(): React.JSX.Element | null {
  const codex = useKnownAgents().find((agent) => agent.id === 'codex')
  const [sure, setSure] = useState(false)
  const [said, setSaid] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  if (!codex?.installed) return null
  const install = (): void => {
    setBusy(true)
    void ask(() => commands.devflowCodexInstall()).then((answer) => {
      setSaid(answer.data?.said ?? answer.error)
      setSure(false)
      setBusy(false)
    })
  }
  return (
    <div className="pref pref--stack">
      <div className="pref__body">
        <span className="pref__t">{t('devflow.settings.codexTitle')}</span>
        <span className="pref__d">{tRich('devflow.settings.codexBody', {
          skill: <code>$devflow-&lt;stage&gt;</code>, // ui-text: ok identifier
        })}</span>
        {sure ? (
          <>
            <span className="pref__d">
              {tRich('devflow.settings.codexSure', {
                add: <code>codex plugin marketplace add &lt;devpit home&gt;/devflow-plugin/codex</code>, // ui-text: ok identifier
                plugin: <code>codex plugin add devflow@devpit</code>, // ui-text: ok identifier
              })}
            </span>
            <div className="voice__row">
              <button className="btn" onClick={() => setSure(false)} disabled={busy}>
                {t('devflow.cancel')}
              </button>
              <button className="btn btn--go" onClick={install} disabled={busy}>
                {t('devflow.install')}
              </button>
            </div>
          </>
        ) : (
          <div className="voice__row">
            <button className="btn" onClick={() => setSure(true)}>
              {t('devflow.settings.codexInstall')}
            </button>
          </div>
        )}
        {said && <span className="pref__d">{said}</span>}
      </div>
    </div>
  )
}

export function DevflowSettings(): React.JSX.Element {
  const [prefs, setPrefs] = useState<DevflowPrefs | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  useEffect(() => {
    void ask(() => commands.devflowPrefs()).then((answer) => setPrefs(answer.data))
  }, [])
  const set = (enabled: boolean | null, preset: string | null): void =>
    void ask(() => commands.devflowPrefsSet(enabled, preset)).then((answer) => {
      if (answer.data) setPrefs(answer.data)
      setProblem(answer.error)
    })

  return (
    <>
      <PrefSwitch
        on={prefs?.enabled ?? true}
        disabled={!prefs}
        onFlip={() => prefs && set(!prefs.enabled, null)}
        title={t('devflow.settings.title')}
        said={t('devflow.settings.said')}
      />
      <div className="pref">
        <span className="pref__body">
          <span className="pref__t">{t('devflow.settings.presetTitle')}</span>
          <span className="pref__d">{t('devflow.settings.presetBody')}</span>
        </span>
        <select aria-label={t('devflow.settings.presetLabel')} value={prefs?.preset ?? 'recommended'} disabled={!prefs} onChange={(event) => set(null, event.target.value)}>
          <option value="recommended">{t('devflow.settings.recommended')}</option>
          <option value="custom">{t('devflow.settings.custom')}</option>
        </select>
      </div>
      <CodexInstall />
      {problem && <p className="pref__d voice__bad">{problem}</p>}
    </>
  )
}
