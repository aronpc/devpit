import { useEffect, useState } from 'react'

import type { Host, HostCheck, HostDraft, HostEnvironment, Hosts } from '../gen/bindings'
import { ask, commands } from './live'

/*
 * Settings → Hosts: the machines a project's terminals can run on, reached
 * with the person's own ssh. devpit keeps the target and a key's path, never
 * a key, and writes nothing on a host.
 */

const ENVIRONMENTS: readonly HostEnvironment[] = ['dev', 'staging', 'production']

type Fields = { name: string; target: string; user: string; port: string; identityFile: string; workRoot: string; environment: HostEnvironment }

const EMPTY: Fields = { name: '', target: '', user: '', port: '', identityFile: '', workRoot: '', environment: 'dev' }

const fieldsOf = (host: Host): Fields => ({
  name: host.name,
  target: host.target,
  user: host.user ?? '',
  port: host.port === null ? '' : String(host.port),
  identityFile: host.identityFile ?? '',
  workRoot: host.workRoot,
  environment: host.environment,
})

/* A port that is not one, out of range included, is sent as zero: the
   backend refuses it with what a port is. */
const portOf = (text: string): number => {
  const port = Number(text)
  return Number.isInteger(port) && port >= 1 && port <= 65535 ? port : 0
}

/* Empty optional fields are left out. */
const draftOf = (id: string | null, fields: Fields): HostDraft => ({
  id,
  name: fields.name,
  target: fields.target,
  user: fields.user.trim() || null,
  port: fields.port.trim() ? portOf(fields.port) : null,
  identityFile: fields.identityFile.trim() || null,
  workRoot: fields.workRoot,
  environment: fields.environment,
})

const LABELS: ReadonlyArray<readonly [keyof Omit<Fields, 'environment'>, string, string]> = [
  ['name', 'Host name', 'web staging'],
  ['target', 'SSH target', 'an ssh config alias, or a host name'],
  ['user', 'User', 'from ssh config when empty'],
  ['port', 'Port', '22'],
  ['identityFile', 'Identity file', '~/.ssh/id_ed25519'],
  ['workRoot', 'Work root', '/srv/apps'],
]

export function HostsSettings(): React.JSX.Element {
  const [hosts, setHosts] = useState<readonly Host[]>([])
  const [editing, setEditing] = useState<string | null>(null)
  const [fields, setFields] = useState<Fields>(EMPTY)
  const [said, setSaid] = useState<string | null>(null)
  const [checks, setChecks] = useState<Readonly<Record<string, HostCheck | 'checking'>>>({})

  useEffect(() => {
    void ask(() => commands.hostsList()).then((answer) => setHosts(answer.data?.hosts ?? []))
  }, [])

  const landed = (answer: { data: Hosts | null; error: string | null }, done: boolean): void => {
    setSaid(answer.error)
    if (!answer.data) return
    setHosts(answer.data.hosts)
    if (done) {
      setEditing(null)
      setFields(EMPTY)
    }
  }
  const save = (): void => void ask(() => commands.hostSave(draftOf(editing, fields))).then((answer) => landed(answer, true))
  const remove = (host: Host): void => void ask(() => commands.hostRemove(host.id)).then((answer) => landed(answer, host.id === editing))
  const check = (host: Host): void => {
    setChecks((was) => ({ ...was, [host.id]: 'checking' }))
    void ask(() => commands.hostCheck(host.id)).then((answer) =>
      setChecks((was) => ({ ...was, [host.id]: answer.data ?? { rows: [{ what: 'check', ok: false, said: answer.error ?? 'the check failed' }] } })),
    )
  }
  const edit = (host: Host): void => {
    setEditing(host.id)
    setFields(fieldsOf(host))
    setSaid(null)
  }

  return (
    <>
      <h1 className="prefs__h">Hosts</h1>
      <p className="acc__note">
        Machines a project&apos;s terminals can run on, reached with your own ssh: its config, agent and known hosts. devpit
        keeps the target and a key&apos;s path, never a key, and creates nothing on a host.
      </p>

      {hosts.map((host) => (
        <div className="pref pref--stack" key={host.id}>
          <div className="pref__body">
            <span className="pref__t">{host.name}</span>
            <span className="pref__d">
              <code>{host.user ? `${host.user}@` : ''}{host.target}{host.port ? `:${host.port}` : ''}</code> · <code>{host.workRoot}</code> ·{' '}
              <span className={`host__env host__env--${host.environment}`}>{host.environment}</span>
            </span>
            <span className="voice__row">
              <button className="btn" aria-label={`Check ${host.name}`} disabled={checks[host.id] === 'checking'} onClick={() => check(host)}>
                {checks[host.id] === 'checking' ? 'Checking…' : 'Check'}
              </button>
              <button className="btn" aria-label={`Edit ${host.name}`} onClick={() => edit(host)}>Edit</button>
              <button className="btn" aria-label={`Remove ${host.name}`} onClick={() => remove(host)}>Remove</button>
            </span>
            <CheckRows check={checks[host.id]} />
          </div>
        </div>
      ))}

      <div className="pref pref--stack">
        <div className="pref__body">
          <span className="pref__t">{editing ? 'Edit host' : 'Add a host'}</span>
          {LABELS.map(([key, label, hint]) => (
            <label className="voice__row" key={key}>
              <span className="voice__l">{label}</span>
              <input className="voice__in" aria-label={label} placeholder={hint} spellCheck={false} value={fields[key]} onChange={(event) => setFields({ ...fields, [key]: event.target.value })} />
            </label>
          ))}
          <label className="voice__row">
            <span className="voice__l">Environment</span>
            <select className="use__pick" aria-label="Environment" value={fields.environment} onChange={(event) => setFields({ ...fields, environment: event.target.value as HostEnvironment })}>
              {ENVIRONMENTS.map((one) => <option key={one} value={one}>{one}</option>)}
            </select>
          </label>
          <span className="voice__row">
            <button className="btn" disabled={!fields.name.trim() || !fields.target.trim() || !fields.workRoot.trim()} onClick={save}>
              {editing ? 'Save host' : 'Add host'}
            </button>
            {editing && <button className="btn" onClick={() => { setEditing(null); setFields(EMPTY); setSaid(null) }}>Cancel</button>}
          </span>
          {said && <span className="pref__d voice__bad" role="alert">{said}</span>}
        </div>
      </div>
    </>
  )
}

/* A host's check, one line each: what, passed or not, and what was found. */
function CheckRows({ check }: { check: HostCheck | 'checking' | undefined }): React.JSX.Element | null {
  if (!check || check === 'checking') return null
  return (
    <ul className="host__check">
      {check.rows.map((row) => (
        <li key={row.what} data-ok={row.ok ? 'true' : 'false'}>
          <span className="host__mark">{row.ok ? '✓' : '✗'}</span> <b>{row.what}</b> <span className="pref__d">{row.said}</span>
        </li>
      ))}
    </ul>
  )
}
