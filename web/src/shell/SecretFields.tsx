import { useEffect, useState } from 'react'

import type { Secret, SecretList } from '../gen/bindings'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { ROTATION_DAYS, rotationChoice, rotationDay, rotationMoment, rotationSaid } from './secretRotation'

const EXAMPLE_NAME = 'JEV_API_KEY'

/*
 * A project's secrets: a key goes from this password field into the project's
 * `.env`, private to you and kept out of git, and never through a chat. Its
 * sessions read it by its name; devpit keeps only the name and the date.
 *
 * Each can carry a rotation — every so many days, or by a date — which goes
 * off as a reminder on the board. A new value kept here counts as rotated.
 */
export function SecretFields({ projectId }: { projectId: string }): React.JSX.Element {
  useLocale()
  const [list, setList] = useState<SecretList | null>(null)
  const [name, setName] = useState('')
  const [value, setValue] = useState('')
  const [said, setSaid] = useState<string | null>(null)
  const [dating, setDating] = useState<string | null>(null)

  useEffect(() => {
    void ask(() => commands.projectSecrets(projectId)).then((answer) => setList(answer.data))
  }, [projectId])

  const keep = (): void => {
    const named = name.trim().toUpperCase()
    void ask(() => commands.projectSecretSet(projectId, named, value)).then((answer) => {
      if (answer.error) return setSaid(answer.error)
      setList(answer.data)
      setName('')
      setValue('')
      setSaid(t('secrets.kept', { name: named }))
    })
  }

  const rotate = (secret: string, everyDays: number | null, at: number | null): void => {
    void ask(() => commands.projectSecretRotation(projectId, secret, everyDays, at)).then((answer) => {
      if (answer.error) return setSaid(answer.error)
      setList(answer.data)
      setDating(null)
      setSaid(t(everyDays || at ? 'secrets.reminds' : 'secrets.noMore', { secret }))
    })
  }

  const picked = (one: Secret, choice: string): void => {
    if (choice === 'date') return setDating(one.name)
    rotate(one.name, choice ? Number(choice) : null, null)
  }

  const now = Date.now() / 1000
  return (
    <details className="pdlg__f wtsetup" open={Boolean(list?.secrets.length)}>
      <summary>{t('secrets.title')}</summary>
      <p className="wtsetup__d">
        {t('secrets.said', { file: list?.file ?? t('secrets.dotEnv') })}
      </p>
      {list?.secrets.map((one) => {
        const rotation = rotationSaid(one, now)
        const byDate = dating === one.name || rotationChoice(one) === 'date'
        return (
          <div key={one.name} className="wtsetup__row">
            <span className="wtsetup__d">
              <code>{one.name}</code> — {t('secrets.setOn', { date: formatDate((one.setAt ?? 0) * 1000) })}
              {rotation && ` · ${rotation}`}
            </span>
            <select aria-label={t('secrets.rotationOf', { name: one.name })} value={byDate ? 'date' : rotationChoice(one)} onChange={(event) => picked(one, event.target.value)}>
              <option value="">{t('secrets.noReminder')}</option>
              {ROTATION_DAYS.map((days) => (
                <option key={days} value={days}>
                  {t('secrets.every', { days })}
                </option>
              ))}
              <option value="date">{t('secrets.byDate')}</option>
            </select>
            {byDate && (
              <input
                type="date"
                aria-label={t('secrets.rotateByAria', { name: one.name })}
                defaultValue={one.rotateAt == null ? '' : rotationDay(one.rotateAt)}
                onChange={(event) => {
                  const at = rotationMoment(event.target.value)
                  if (at) rotate(one.name, null, at)
                }}
              />
            )}
          </div>
        )
      })}
      <label className="wtsetup__f">
        <span>{t('secrets.name')}</span>
        <input value={name} placeholder={EXAMPLE_NAME} onChange={(event) => setName(event.target.value)} />
      </label>
      <label className="wtsetup__f">
        <span>{t('secrets.value')}</span>
        <input type="password" autoComplete="off" value={value} onChange={(event) => setValue(event.target.value)} />
      </label>
      <button className="btn" disabled={!name.trim() || !value} onClick={keep}>
        {t('secrets.keepIt')}
      </button>
      {said && <p className="wtsetup__d">{said}</p>}
    </details>
  )
}
