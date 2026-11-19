import { useEffect, useState } from 'react'

import type { Host, HostCheck, HostDraft, HostEnvironment, Hosts } from '../gen/bindings'
import { t } from '../i18n'
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
  ['name', 'workspace.hosts.name', 'workspace.hosts.nameHint'],
  ['target', 'workspace.hosts.target', 'workspace.hosts.targetHint'],
  ['user', 'workspace.hosts.user', 'workspace.hosts.userHint'],
  ['port', 'workspace.hosts.port', 'workspace.hosts.portHint'],
  ['identityFile', 'workspace.hosts.identity', 'workspace.hosts.identityHint'],
  ['workRoot', 'workspace.hosts.workRoot', 'workspace.hosts.workRootHint'],
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
  /* A check said what the host was; an edit may make it another. */
  const save = (): void => {
    const id = editing
    void ask(() => commands.hostSave(draftOf(id, fields))).then((answer) => {
      if (answer.data && id) setChecks(({ [id]: _was, ...rest }) => rest)
      landed(answer, true)
    })
  }
  const remove = (host: Host): void => void ask(() => commands.hostRemove(host.id)).then((answer) => landed(answer, host.id === editing))
  const check = (host: Host): void => {
    setChecks((was) => ({ ...was, [host.id]: 'checking' }))
    void ask(() => commands.hostCheck(host.id)).then((answer) =>
      setChecks((was) => ({ ...was, [host.id]: answer.data ?? { rows: [{ what: 'check', ok: false, said: answer.error ?? t('workspace.hosts.checkFailed') }] } })),
    )
  }
  const edit = (host: Host): void => {
    setEditing(host.id)
    setFields(fieldsOf(host))
    setSaid(null)
  }

  return (
    <>
      <h1 className="prefs__h">{t('workspace.hosts.title')}</h1>
      <p className="acc__note">
        {t('workspace.hosts.note')}
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
              <button className="btn" aria-label={t('workspace.hosts.checkHost', { name: host.name })} disabled={checks[host.id] === 'checking'} onClick={() => check(host)}>
                {checks[host.id] === 'checking' ? t('workspace.hosts.checking') : t('workspace.hosts.check')}
              </button>
              <button className="btn" aria-label={t('workspace.hosts.editHost', { name: host.name })} onClick={() => edit(host)}>{t('workspace.hosts.edit')}</button>
              <button className="btn" aria-label={t('workspace.hosts.removeHost', { name: host.name })} onClick={() => remove(host)}>{t('workspace.hosts.remove')}</button>
            </span>
            <CheckRows check={checks[host.id]} />
          </div>
        </div>
      ))}

      <div className="pref pref--stack">
        <div className="pref__body">
          <span className="pref__t">{editing ? t('workspace.hosts.editTitle') : t('workspace.hosts.addTitle')}</span>
          {LABELS.map(([key, labelKey, hintKey]) => (
            <label className="voice__row" key={key}>
              <span className="voice__l">{t(labelKey)}</span>
              <input className="voice__in" aria-label={t(labelKey)} placeholder={t(hintKey)} spellCheck={false} value={fields[key]} onChange={(event) => setFields({ ...fields, [key]: event.target.value })} />
            </label>
          ))}
          <label className="voice__row">
            <span className="voice__l">{t('workspace.hosts.environment')}</span>
            <select className="use__pick" aria-label={t('workspace.hosts.environment')} value={fields.environment} onChange={(event) => setFields({ ...fields, environment: event.target.value as HostEnvironment })}>
              {ENVIRONMENTS.map((one) => <option key={one} value={one}>{one}</option>)}
            </select>
          </label>
          <span className="voice__row">
            <button className="btn" disabled={!fields.name.trim() || !fields.target.trim() || !fields.workRoot.trim()} onClick={save}>
              {editing ? t('workspace.hosts.save') : t('workspace.hosts.add')}
            </button>
            {editing && <button className="btn" onClick={() => { setEditing(null); setFields(EMPTY); setSaid(null) }}>{t('workspace.hosts.cancel')}</button>}
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
