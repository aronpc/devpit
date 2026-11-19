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
    <details className="pdlg__f wtsetup" open={links.length > 0}>
      <summary>Hosts</summary>
      <p className="wtsetup__d">Where this project is on each host it runs on. devpit checks the folder is there and never creates one.</p>
      {links.map((one) => (
        <div key={one.hostId} className="wtsetup__row">
          <span className="wtsetup__d">
            <b>{one.hostName}</b> <span className={`host__env host__env--${one.environment}`}>{one.environment}</span> <code>{one.path}</code>
          </span>
          <button className="btn" aria-label={`Unlink ${one.hostName}`} onClick={() => unlink(one)}>Unlink</button>
        </div>
      ))}
      <label className="wtsetup__f">
        <span>Host</span>
        <select aria-label="Host to link" value={hostId} onChange={(event) => setHostId(event.target.value)}>
          <option value="">Choose a host…</option>
          {hosts.map((one) => <option key={one.id} value={one.id}>{one.name} ({one.environment})</option>)}
        </select>
      </label>
      <label className="wtsetup__f">
        <span>Path</span>
        <input aria-label="Path on the host" spellCheck={false} placeholder={chosen ? `${chosen.workRoot}/…` : 'the project\u2019s folder there'} value={path} onChange={(event) => setPath(event.target.value)} />
      </label>
      <button className="btn" disabled={busy || !hostId || !path.trim()} onClick={link}>{busy ? 'Checking…' : 'Link'}</button>
      {said && <p className="wtsetup__d voice__bad" role="alert">{said}</p>}
    </details>
  )
}
