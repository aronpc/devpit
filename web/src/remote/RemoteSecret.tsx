import { useState } from 'react'

import type { RemoteIn, RemoteProject } from '../gen/bindings'
import { t } from '../i18n'

/*
 * A secret kept from the phone: the value goes to the machine over this
 * device's own connection and into the project's .env, the same write as
 * Edit project → Secrets there. It is dropped from this page as it is sent,
 * and the machine says back the name alone.
 */
export function RemoteSecret({
  projects,
  project,
  typing,
  send,
}: {
  projects: readonly RemoteProject[]
  project: string | null
  typing: boolean
  send: (message: RemoteIn) => void
}): React.JSX.Element {
  const [into, setInto] = useState(project ?? projects[0]?.id ?? '')
  const [name, setName] = useState('')
  const [value, setValue] = useState('')

  if (!typing) return <p className="rm__said">{t('remote.secret.watchOnly')}</p>

  const named = name.trim().toUpperCase()
  const keep = (): void => {
    send({ t: 'secretSet', project: into, name: named, value })
    setValue('')
  }

  return (
    <div className="rm__vault">
      <label className="rm__field">
        <span>{t('remote.secret.project')}</span>
        <select value={into} onChange={(event) => setInto(event.target.value)}>
          {projects.map((one) => (
            <option key={one.id} value={one.id}>
              {one.name}
            </option>
          ))}
        </select>
      </label>
      <label className="rm__field">
        <span>{t('remote.secret.name')}</span>
        <input
          value={name}
          placeholder="JEV_API_KEY" // ui-text: ok an example variable name, not a sentence
          autoCapitalize="characters"
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      <label className="rm__field">
        <span>{t('remote.secret.value')}</span>
        <input type="password" autoComplete="off" value={value} onChange={(event) => setValue(event.target.value)} />
      </label>
      <p className="rm__dim">{t('remote.secret.kept')}</p>
      <button className="rm__btn rm__btn--go" disabled={!into || !named || !value} onClick={keep}>
        {t('remote.secret.keep')}
      </button>
    </div>
  )
}
