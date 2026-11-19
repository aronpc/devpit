import { useState } from 'react'

import type { QueuedMessage } from '../gen/bindings'
import { abandoned, committed } from './typing'
import type { Queue } from './useQueue'
import { t } from '../i18n'

/* The messages waiting for the turn to end, above where you type: each one
   editable, movable and removable before it goes, or sent into the turn now.
   Held, the queue says so and offers to go on or to give it all back. */
export function Queued({ queue, sending, onBack }: { queue: Queue; sending: boolean; onBack: () => void }): React.JSX.Element | null {
  const [editing, setEditing] = useState<string | null>(null)
  if (queue.waiting.length === 0) return null
  return (
    <div className="queued" aria-label={t('sessions.queued.label')}>
      {(queue.paused || !sending) && (
        <div className="queued__head">
          <span className="queued__k">{queue.paused ? t('sessions.queued.paused') : t('sessions.queued.queued')}</span>
          <span className="queued__t">{queue.paused ? t('sessions.queued.pausedNote') : t('sessions.queued.waitingNote')}</span>
          <button className="queued__go" onClick={queue.resume} title={sending ? t('sessions.queued.resumeSending') : t('sessions.queued.resumeIdle')}>{t('sessions.queued.resume')}</button>
          <button className="queued__go" onClick={onBack} title={t('sessions.queued.backTitle')}>{t('sessions.queued.back')}</button>
        </div>
      )}
      {queue.waiting.map((item, index) => (
        <div className="queued__o" key={item.id}>
          {editing === item.id ? (
            <QueuedEdit item={item} onDone={(prompt) => (prompt.trim() && queue.edit(item.id, prompt), setEditing(null))} />
          ) : (
            <span className="queued__t" title={item.prompt}>{item.prompt}</span>
          )}
          {item.files.length > 0 && (
            <span className="queued__f" title={item.files.map((file) => file.name).join(', ')}>
              {t('sessions.queued.files', { count: item.files.length })}
            </span>
          )}
          {index > 0 && <button className="queued__go" onClick={() => queue.raise(item.id)} aria-label={t('sessions.queued.up')} title={t('sessions.queued.up')}>↑</button>}
          {editing !== item.id && <button className="queued__go" onClick={() => setEditing(item.id)}>{t('sessions.queued.edit')}</button>}
          {sending && <button className="queued__go" onClick={() => queue.now(item.id)} title={t('sessions.queued.nowTitle')}>{t('sessions.queued.now')}</button>}
          <button className="queued__x" onClick={() => queue.drop(item.id)} aria-label={t('sessions.queued.removeLabel')} title={t('sessions.queued.remove')}>✕</button>
        </div>
      ))}
    </div>
  )
}

/* Enter keeps the edit, Escape leaves it as it was. */
function QueuedEdit({ item, onDone }: { item: QueuedMessage; onDone: (prompt: string) => void }): React.JSX.Element {
  const [text, setText] = useState(item.prompt)
  return (
    <textarea
      className="queued__e"
      aria-label={t('sessions.queued.editLabel')}
      autoFocus
      rows={Math.min(6, text.split('\n').length)}
      value={text}
      onChange={(event) => setText(event.target.value)}
      onBlur={() => onDone(text)}
      onKeyDown={(event) => {
        if (abandoned(event)) return (event.preventDefault(), onDone(item.prompt))
        if (committed(event) && !event.shiftKey) return (event.preventDefault(), onDone(text))
      }}
    />
  )
}
