import { useCallback, useEffect, useState } from 'react'

import type { Reminder, Reminders } from '../gen/bindings'
import { t } from '../i18n'
import { onCameBack } from './cameBack'
import { ask, commands } from './live'
import { onCarried } from './window'

/**
 * The reminders that went off and nobody has dealt with yet.
 *
 * Asked once, and again whenever one goes off or is dealt with anywhere — on
 * the banner, from an agent, by a date moved on a card. The window coming
 * back to the front also rearms the app's clock: a machine that slept may
 * have passed a reminder while the thread waiting for it slept too.
 */
export function useReminders(): {
  reminders: readonly Reminder[]
  snooze: (cardId: string, until: number) => void
  done: (cardId: string) => void
  rotated: (one: Reminder) => void
  forget: (one: Reminder) => void
} {
  const [reminders, setReminders] = useState<readonly Reminder[]>([])

  const told = useCallback((answer: { data: Reminders | null }): void => {
    if (answer.data) setReminders(answer.data.reminders)
  }, [])
  const reload = useCallback(() => void ask(() => commands.remindersPending()).then(told), [told])

  useEffect(() => {
    reload()
    const fired = onCarried<Reminders>('reminder:fired', reload)
    const changed = onCarried<null>('reminder:changed', reload)
    const back = onCameBack(() => void commands.remindersRearm().catch(() => undefined))
    return () => {
      fired()
      changed()
      back()
    }
  }, [reload])

  return {
    reminders,
    snooze: (cardId, until) => void ask(() => commands.reminderSnooze(cardId, until)).then(told),
    done: (cardId) => void ask(() => commands.reminderDone(cardId)).then(told),
    // A rotation answers with the project's secrets, so the reminders are read again.
    rotated: (one) => {
      const name = one.secret
      if (name) void ask(() => commands.projectSecretRotated(one.projectId, name)).then(reload)
    },
    forget: (one) => {
      const name = one.secret
      if (name) void ask(() => commands.projectSecretRotation(one.projectId, name, null, null)).then(reload)
    },
  }
}

/** The moments a reminder can be put off to, from `now`, in seconds. */
export function laterOptions(now: number): { label: string; until: number }[] {
  const tomorrow = new Date(now * 1000)
  tomorrow.setDate(tomorrow.getDate() + 1)
  tomorrow.setHours(9, 0, 0, 0)
  return [
    { label: t('workspace.reminders.min15'), until: now + 15 * 60 },
    { label: t('workspace.reminders.hour1'), until: now + 3600 },
    { label: t('workspace.reminders.tomorrow'), until: tomorrow.getTime() / 1000 },
  ]
}
