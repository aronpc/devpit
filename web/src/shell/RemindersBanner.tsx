import { useState } from 'react'

import type { Reminder } from '../gen/bindings'
import { t } from '../i18n'
import { dueLabel } from './due'
import { useShell } from './useShell'
import { laterOptions, useReminders } from './useReminders'

/*
 * The reminders that went off, under the top bar, until each is dealt with.
 *
 * Not another bell. A reminder is something the person asked to be
 * interrupted with at a time, so it interrupts: it has no close, only Later —
 * fifteen minutes, an hour, tomorrow morning — and Done. Several stack into
 * one strip that opens to show them all. A focus does not hold it back: the
 * time was the person's own choice. The bell keeps it too, as a record.
 *
 * A secret's rotation is Rotated, which sets the next date, or Forget,
 * rather than Done: done without a next date would just stop reminding.
 */

function Row({
  one,
  now,
  onLater,
  onDone,
  onRotated,
  onForget,
  onOpen,
}: {
  one: Reminder
  now: number
  onLater: (until: number) => void
  onDone: () => void
  onRotated: () => void
  onForget: () => void
  onOpen: () => void
}): React.JSX.Element {
  return (
    <div className="remind__one">
      <button className="remind__what" onClick={onOpen} title={t('workspace.reminders.open')}>
        <b>{one.secret ? t('workspace.reminders.rotate', { secret: one.secret }) : one.title}</b>
        <span className="remind__when">
          {dueLabel(one.dueAt, now, one.timed)}
          {one.project && ` · ${one.project}`}
        </span>
      </button>
      <span className="remind__acts">
        {laterOptions(now).map((later) => (
          <button key={later.label} className="remind__btn" onClick={() => onLater(later.until)}>
            {later.label}
          </button>
        ))}
        {one.secret ? (
          <>
            <button className="remind__btn remind__btn--done" onClick={onRotated} title={t('workspace.reminders.rotatedTitle')}>
              {t('workspace.reminders.rotated')}
            </button>
            <button className="remind__btn" onClick={onForget} title={t('workspace.reminders.forgetTitle')}>
              {t('workspace.reminders.forget')}
            </button>
          </>
        ) : (
          <button className="remind__btn remind__btn--done" onClick={onDone}>
            {t('workspace.reminders.done')}
          </button>
        )}
      </span>
    </div>
  )
}

export function RemindersBanner(): React.JSX.Element | null {
  const { reminders, snooze, done, rotated, forget } = useReminders()
  const { setProject, show, openCard, project } = useShell()
  const [all, setAll] = useState(false)
  if (reminders.length === 0) return null

  const now = Date.now() / 1000
  const shown = all ? reminders : reminders.slice(0, 1)
  const open = (one: Reminder): void => {
    if (one.projectId !== project?.id) setProject(one.projectId)
    show('board')
    openCard(one.cardId)
  }

  return (
    <section className="remind" role="alert" aria-label={t('workspace.reminders.label')}>
      <svg className="remind__icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden><circle cx="12" cy="13" r="8" /><path d="M12 9v4l2 2M5 3 2 6M22 6l-3-3" /></svg>
      <div className="remind__list">
        {shown.map((one) => (
          <Row
            key={one.cardId}
            one={one}
            now={now}
            onLater={(until) => snooze(one.cardId, until)}
            onDone={() => done(one.cardId)}
            onRotated={() => rotated(one)}
            onForget={() => forget(one)}
            onOpen={() => open(one)}
          />
        ))}
      </div>
      {reminders.length > 1 && (
        <button className="remind__more" aria-expanded={all} onClick={() => setAll((was) => !was)}>
          {all ? t('workspace.reminders.fewer') : t('workspace.reminders.more', { count: reminders.length - 1 })}
        </button>
      )}
    </section>
  )
}
