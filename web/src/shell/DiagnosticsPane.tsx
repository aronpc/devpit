import { useEffect, useState } from 'react'

import type { Diagnostics } from '../gen/bindings'
import { bytes } from './disk'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'
import { t } from '../i18n'

/*
 * Settings → Diagnostics: what devpit itself is running and costing, the
 * checks `devpit diagnostics` prints in a terminal, and its error log. Read
 * when the pane opens and when asked again, never on a timer.
 */

function percent(tenths: number): string {
  return `${(tenths / 10).toFixed(1)}%`
}

export function DiagnosticsPane(): React.JSX.Element {
  const [shown, setShown] = useState<Diagnostics | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const took = (answer: { data: Diagnostics | null; error: string | null }): void => {
    setBusy(false)
    setError(answer.error)
    if (answer.data) setShown(answer.data)
  }

  useEffect(() => {
    void ask(() => commands.diagnosticsRead()).then(took)
  }, [])

  const look = (): void => {
    setBusy(true)
    void ask(() => commands.diagnosticsRead()).then(took)
  }

  const processes = shown?.processes ?? []
  const cpu = processes.reduce((sum, one) => sum + one.cpuTenths, 0)
  const memory = processes.reduce((sum, one) => sum + (one.memoryBytes ?? 0), 0)
  const totals = shown ? t('workspace.diagnostics.totals', { count: processes.length, cpu: percent(cpu), memory: bytes(memory) ?? '0 B' }) : t('workspace.diagnostics.looking')
  return (
    <>
      <PrefRow title={t('workspace.diagnostics.processes')} said={totals}>
        <button className="btn" disabled={busy} onClick={look}>
          {busy ? t('workspace.diagnostics.looking') : t('workspace.diagnostics.lookAgain')}
        </button>
      </PrefRow>
      {error && <p className="pref__d">{error}</p>}
      {processes.length > 0 && (
        <table className="diag__t">
          <thead>
            <tr>
              <th>{t('workspace.diagnostics.process')}</th>
              <th>{t('workspace.diagnostics.pid')}</th>
              <th>{t('workspace.diagnostics.cpu')}</th>
              <th>{t('workspace.diagnostics.memory')}</th>
            </tr>
          </thead>
          <tbody>
            {processes.map((one) => (
              <tr key={one.pid}>
                <td>{one.name}</td>
                <td>{one.pid}</td>
                <td>{percent(one.cpuTenths)}</td>
                <td>{bytes(one.memoryBytes ?? 0) ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <div className="acc__sub">{t('workspace.diagnostics.checks')}</div>
      {shown?.checks.map((check) => (
        <div className="pref" key={check.name}>
          <span className="pref__body">
            <span className="pref__t">
              {check.ok ? '✓' : '✗'} {check.name}
            </span>
            <span className="pref__d">
              {check.detail}
              {!check.ok && check.fix ? t('workspace.diagnostics.fix', { fix: check.fix }) : ''}
            </span>
          </span>
        </div>
      ))}

      <div className="acc__sub">{t('workspace.diagnostics.errorLog')}</div>
      <p className="pref__d">{shown?.logPath}</p>
      <pre className="diag__log">{shown && shown.log.length > 0 ? shown.log.join('\n') : t('workspace.diagnostics.nothingLogged')}</pre>
    </>
  )
}
