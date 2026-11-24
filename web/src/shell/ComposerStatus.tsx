import { useEffect, useMemo, useRef, useState } from 'react'

import type { Message, Part } from '../gen/bindings'
import { elapsed, running, taskLabel, tasksOf, wakeupOf } from './backgroundTasks'
import { checklistOf, done } from './checklist'
import { useMessagesPick, type ChatMessages } from './chatMessages'
import { ask, commands } from './live'
import { t } from '../i18n'

/*
 * What the turn has going on, above the composer where the hands are: work it
 * left running in the background behind one chip, and the checklist it is
 * working through.
 *
 * Both read the newest answer only. A checklist from three turns ago is done
 * or abandoned, and either way it is not what the agent is doing now.
 */

const NONE: readonly Part[] = []

/* The newest answer's parts other than its text, item by item the same while only its text grows. */
const workOf = (all: readonly Message[]): readonly Part[] =>
  all.findLast((one) => one.role === 'assistant')?.parts.filter((part) => part.kind !== 'text') ?? NONE

export function ComposerStatus({
  conversationId,
  messages,
}: {
  conversationId: string
  messages: ChatMessages
}): React.JSX.Element | null {
  /* Read from the end, and only when the work changes: the clock below ticks
     every second and must not walk the thread, nor a streamed batch draw it. */
  const work = useMessagesPick(messages, workOf)
  const { tasks, wakeup, list } = useMemo(
    () => ({ tasks: tasksOf(work).filter(running), wakeup: wakeupOf(work), list: checklistOf(work) }),
    [work],
  )
  const [open, setOpen] = useState(false)
  const count = tasks.length + (wakeup ? 1 : 0)

  /* When each task was first seen here. The CLI sends no start time, and
     "running for" is the question a person glancing at the strip is asking. */
  const since = useRef(new Map<string, number>())
  const [now, setNow] = useState(() => Date.now())
  for (const task of tasks) if (!since.current.has(task.id)) since.current.set(task.id, Date.now())
  useEffect(() => {
    if (tasks.length === 0) return
    /* A window nobody is looking at does not need its clock ticking. */
    const timer = window.setInterval(() => {
      if (!document.hidden) setNow(Date.now())
    }, 1000)
    return () => window.clearInterval(timer)
  }, [tasks.length])

  if (count === 0 && list.items.length === 0) return null

  return (
    <div className="bgt">
      {count > 0 && (
        <button className="chip bgt__chip" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
          {tasks.length > 0 && <span className="arow__live" aria-hidden="true" />}
          {t('workspace.composerStatus.inBackground', { count })}
        </button>
      )}
      {open && count > 0 && (
        <div className="bgp" role="region" aria-label={t('workspace.composerStatus.region')}>
          {tasks.map((task) => (
            <div className="bgt__row" key={task.id}>
              <span className="arow__live" aria-label={t('workspace.composerStatus.running')} />
              <span className="bgt__k">{taskLabel(task)}</span>
              <span className="bgt__d" title={task.description}>
                {task.description}
              </span>
              <span className="bgt__t">{elapsed(since.current.get(task.id) ?? now, now)}</span>
              <button
                className="chip"
                aria-label={t('workspace.composerStatus.stopLabel', { task: task.description })}
                onClick={() => void ask(() => commands.chatStopTask(conversationId, task.id))}
              >
                {t('workspace.composerStatus.stop')}
              </button>
            </div>
          ))}
          {wakeup && (
            <div className="bgt__row">
              <span className="bgt__k">{t('workspace.composerStatus.wakeup')}</span>
              <span className="bgt__d" title={wakeup.reason}>
                {wakeup.reason}
              </span>
              {wakeup.when && <span className="bgt__t">{wakeup.when}</span>}
            </div>
          )}
        </div>
      )}

      {list.items.length > 0 && (
        <details className="ckl">
          <summary className="ckl__h">
            {t('workspace.composerStatus.done', { done: list.items.filter(done).length, total: list.items.length })}
          </summary>
          {list.items.map((item) => (
            <div className="ckl__i" key={item.id} data-done={done(item) || undefined} data-changed={list.changed.includes(item.id) || undefined}>
              <span className="ckl__box" aria-hidden="true">{done(item) ? '✓' : ''}</span>
              {item.subject}
            </div>
          ))}
        </details>
      )}
    </div>
  )
}
