import { useEffect, useState } from 'react'

import type { CalendarFeed } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'

/*
 * Card dates and reminders as a calendar on devpit.app, which Google Calendar
 * can subscribe to. Events say only "devpit reminder" unless titles are on:
 * the address is a secret, but whoever holds it reads what is there.
 */
export function CalendarSettings(): React.JSX.Element {
  const [feed, setFeed] = useState<CalendarFeed | null>(null)
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    void ask(() => commands.calendarRead()).then((answer) => setFeed(answer.data))
  }, [])

  const done = (answer: { data: CalendarFeed | null; error: string | null }): void => {
    setBusy(false)
    setSaid(answer.error)
    if (answer.data) setFeed(answer.data)
  }
  const set = (on: boolean, titles: boolean): void => {
    setBusy(true)
    void ask(() => commands.calendarSet(on, titles)).then(done)
  }
  const reset = (): void => {
    setBusy(true)
    setCopied(false)
    void ask(() => commands.calendarReset()).then(done)
  }

  if (!feed) return <></>
  const link = feed.link
  const copy = (): void => {
    if (link) void navigator.clipboard?.writeText(link).then(() => setCopied(true))
  }
  return (
    <PrefSwitch
      on={Boolean(link)}
      disabled={busy || (!link && !feed.signedIn)}
      onFlip={() => set(!link, feed.titles)}
      title={t('workspace.calendar.title')}
      said={
        <>
          {t('workspace.calendar.said')}
          {!feed.signedIn && ` ${t('workspace.calendar.signIn')}`}
        </>
      }
    >
      {link && (
        <>
          <span className="voice__row">
            <code className="remote__addr">{link}</code>
            <button className="btn" onClick={copy}>
              {copied ? t('workspace.calendar.copied') : t('workspace.calendar.copy')}
            </button>
          </span>
          <label className="voice__row">
            <input type="checkbox" checked={feed.titles} disabled={busy} onChange={(event) => set(true, event.target.checked)} />
            {t('workspace.calendar.titles')}
          </label>
          <span className="pref__d">
            {tRich('workspace.calendar.anyone', {
              reset: (
                <button className="plusl" disabled={busy} onClick={reset}>
                  {t('workspace.calendar.reset')}
                </button>
              ),
            })}
          </span>
        </>
      )}
      {said && <span className="pref__d voice__bad">{said}</span>}
    </PrefSwitch>
  )
}
