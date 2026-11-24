import { useEffect, useRef, useState } from 'react'

import { canPaint, counter, occurrences, paint, rangesIn, stepped, unpaint, type Occurrence } from './chatSearch'
import { ask, commands } from './live'
import { discloseMessage } from './useDisclosed'
import { abandoned, committed } from './typing'
import { t } from '../i18n'

/*
 * Finding a word in the conversation, from a bar over it.
 *
 * The count is the backend's, over the whole transcript; going to a match
 * reads the pages it is in. Matches are painted with the Custom Highlight
 * API; where it is missing, the row holding the match is outlined instead.
 */
export function ChatFind({
  where,
  box,
  jump,
  onDone,
}: {
  where: { projectId: string; conversationId: string } | null
  box: React.RefObject<HTMLElement | null>
  jump: (id: string, then?: (row: HTMLElement) => void) => Promise<boolean>
  onDone: () => void
}): React.JSX.Element {
  const [what, setWhat] = useState('')
  const [found, setFound] = useState<readonly Occurrence[] | null>(null)
  const [at, setAt] = useState(0)
  const field = useRef<HTMLInputElement>(null)
  const now = useRef<{ needle: string; one: Occurrence | null }>({ needle: '', one: null })

  useEffect(() => field.current?.focus(), [])

  /* Painted again as rows come and go: a range in a row that left is gone with it. */
  const repaint = (): void => {
    const scroll = box.current
    const { needle, one } = now.current
    if (!scroll) return
    const row = one ? scroll.querySelector<HTMLElement>(`.trow[data-id="${CSS.escape(one.messageId)}"]`) : null
    const ranges = row ? rangesIn(row, needle) : []
    paint(scroll, needle, one ? (ranges[Math.min(one.nth, ranges.length - 1)] ?? null) : null)
  }

  const show = (one: Occurrence, needle: string): void => {
    now.current = { needle, one }
    /* A settled turn folds its earlier prose, and a long prompt its tail: the match may be there. */
    discloseMessage(one.messageId)
    void jump(one.messageId, (row) => {
      const scroll = box.current
      if (!scroll) return
      for (const was of Array.from(scroll.querySelectorAll('.trow[data-found]'))) was.removeAttribute('data-found')
      if (!canPaint()) row.setAttribute('data-found', '')
      repaint()
      const ranges = rangesIn(row, needle)
      const range = ranges[Math.min(one.nth, ranges.length - 1)]
      const place = range?.getBoundingClientRect()
      if (place && place.height > 0) scroll.scrollTop += place.top - scroll.getBoundingClientRect().top - scroll.clientHeight / 3
    })
  }

  /* The effects reach these through a ref: they follow the words, not every render. */
  const latest = useRef({ show, repaint })
  latest.current = { show, repaint }

  useEffect(() => {
    const needle = what.trim()
    if (!needle || !where) {
      setFound(null)
      now.current = { needle: '', one: null }
      unpaint()
      return
    }
    let gone = false
    const timer = window.setTimeout(() => {
      void ask(() => commands.chatFind(where.projectId, where.conversationId, needle)).then((answer) => {
        if (gone || !answer.data) return
        const all = occurrences(answer.data.hits)
        setFound(all)
        /* The newest first: it is nearest where the person is reading. */
        setAt(Math.max(0, all.length - 1))
        if (all.length > 0) latest.current.show(all.at(-1)!, needle)
        else {
          now.current = { needle, one: null }
          latest.current.repaint()
        }
      })
    }, 200)
    return () => {
      gone = true
      window.clearTimeout(timer)
    }
  }, [what, where, latest])

  useEffect(() => {
    const scroll = box.current
    if (!scroll) return
    let frame = 0
    const moved = (): void => {
      if (frame) return
      frame = requestAnimationFrame(() => {
        frame = 0
        if (now.current.needle) latest.current.repaint()
      })
    }
    scroll.addEventListener('scroll', moved, { passive: true })
    return () => {
      scroll.removeEventListener('scroll', moved)
      cancelAnimationFrame(frame)
      unpaint()
      for (const was of Array.from(scroll.querySelectorAll('.trow[data-found]'))) was.removeAttribute('data-found')
    }
  }, [box, latest])

  const step = (back: boolean): void => {
    if (!found || found.length === 0) return
    const next = stepped(at, found.length, back)
    setAt(next)
    show(found[next]!, what.trim())
  }

  return (
    <form
      className="cfind"
      role="search"
      onSubmit={(event) => {
        event.preventDefault()
        step(false)
      }}
    >
      <input
        className="cfind__in"
        ref={field}
        aria-label={t('sessions.find.conversation')}
        placeholder={t('sessions.find.conversation')}
        spellCheck={false}
        value={what}
        onChange={(event) => setWhat(event.target.value)}
        onKeyDown={(event) => {
          /* Taken here, or the chat's own Escape would stop the turn. */
          if (abandoned(event)) {
            event.preventDefault()
            onDone()
          }
          if (committed(event) && event.shiftKey) {
            event.preventDefault()
            step(true)
          }
        }}
      />
      {found && <span className="cfind__n" aria-live="polite">{counter(at, found.length)}</span>}
      <button className="browser__act" type="button" aria-label={t('sessions.find.prev')} title={t('sessions.find.prev')} onClick={() => step(true)}>
        ‹
      </button>
      <button className="browser__act" type="submit" aria-label={t('sessions.find.next')} title={t('sessions.find.next')}>
        ›
      </button>
      <button className="browser__act" type="button" aria-label={t('sessions.find.close')} title={t('sessions.find.close')} onClick={onDone}>
        ✕
      </button>
    </form>
  )
}
