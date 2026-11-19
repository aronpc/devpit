import { useEffect, useState } from 'react'

import type { Host, HostLink, HostLinks } from '../gen/bindings'
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
    <div className="pdlg__f">
      <span>Hosts</span>
      {links.map((one) => (
        <span className="voice__row" key={one.hostId}>
          <b>{one.hostName}</b> <span className={`host__env host__env--${one.environment}`}>{one.environment}</span> <code>{one.path}</code>
          <button className="btn" aria-label={`Unlink ${one.hostName}`} onClick={() => unlink(one)}>Unlink</button>
        </span>
      ))}
      <span className="voice__row">
        <select className="use__pick" aria-label="Host to link" value={hostId} onChange={(event) => setHostId(event.target.value)}>
          <option value="">Link a host…</option>
          {hosts.map((one) => <option key={one.id} value={one.id}>{one.name} ({one.environment})</option>)}
        </select>
        <input className="voice__in" aria-label="Path on the host" spellCheck={false} placeholder={chosen ? `${chosen.workRoot}/…` : 'the project’s folder there'} value={path} onChange={(event) => setPath(event.target.value)} />
        <button className="btn" disabled={busy || !hostId || !path.trim()} onClick={link}>{busy ? 'Checking…' : 'Link'}</button>
      </span>
      {said && <span className="pref__d voice__bad" role="alert">{said}</span>}
    </div>
  )
}
