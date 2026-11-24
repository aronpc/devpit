import { useEffect, useState } from 'react'

import type { Host, HostLink, HostLinks } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'

/*
 * Project → Hosts: where this project is on each host it runs on. The path is
 * the person's; devpit checks it is a folder there and never creates one.
 */

export function ProjectHosts({ projectId }: { projectId: string }): React.JSX.Element | null {
  const [hosts, setHosts] = useState<readonly Host[]>([])
  const [links, setLinks] = useState<readonly HostLink[]>([])
  const [hostId, setHostId] = useState('')
  const [path, setPath] = useState('')
  const [said, setSaid] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    void ask(() => commands.hostsList()).then((answer) => setHosts(answer.data?.hosts ?? []))
    void ask(() => commands.projectHosts(projectId)).then((answer) => setLinks(answer.data?.links ?? []))
  }, [projectId])

  const landed = (answer: { data: HostLinks | null; error: string | null }): void => {
    setBusy(false)
    setSaid(answer.error)
    if (answer.data) setLinks(answer.data.links)
  }
  const link = (): void => {
    setBusy(true)
    void ask(() => commands.projectHostLink(projectId, hostId, path)).then((answer) => {
      landed(answer)
      if (answer.data) setPath('')
    })
  }
  const unlink = (one: HostLink): void => void ask(() => commands.projectHostUnlink(projectId, one.hostId)).then(landed)

  if (hosts.length === 0 && links.length === 0) return null
  const chosen = hosts.find((one) => one.id === hostId)
  return (
    <details className="pdlg__f wtsetup" open={links.length > 0}>
      <summary>{t('workspace.projectHosts.title')}</summary>
      <p className="wtsetup__d">{t('workspace.projectHosts.said')}</p>
      {links.map((one) => (
        <div key={one.hostId} className="wtsetup__row">
          <span className="wtsetup__d">
            <b>{one.hostName}</b> <span className={`host__env host__env--${one.environment}`}>{one.environment}</span> <code>{one.path}</code>
          </span>
          <button className="btn" aria-label={t('workspace.projectHosts.unlinkLabel', { name: one.hostName })} onClick={() => unlink(one)}>{t('workspace.projectHosts.unlink')}</button>
        </div>
      ))}
      <label className="wtsetup__f">
        <span>{t('workspace.projectHosts.host')}</span>
        <select aria-label={t('workspace.projectHosts.hostToLink')} value={hostId} onChange={(event) => setHostId(event.target.value)}>
          <option value="">{t('workspace.projectHosts.chooseHost')}</option>
          {hosts.map((one) => <option key={one.id} value={one.id}>{one.name} ({one.environment})</option>)}
        </select>
      </label>
      <label className="wtsetup__f">
        <span>{t('workspace.projectHosts.path')}</span>
        <input aria-label={t('workspace.projectHosts.pathOnHost')} spellCheck={false} placeholder={chosen ? `${chosen.workRoot}/…` : t('workspace.projectHosts.pathHint')} value={path} onChange={(event) => setPath(event.target.value)} />
      </label>
      <button className="btn" disabled={busy || !hostId || !path.trim()} onClick={link}>{busy ? t('workspace.projectHosts.checking') : t('workspace.projectHosts.link')}</button>
      {said && <p className="wtsetup__d voice__bad" role="alert">{said}</p>}
    </details>
  )
}
