import { useRef } from 'react'

import type { Notice } from '../gen/bindings'
import { t } from '../i18n'
import { useAway } from './away'
import { since } from './projects'
import { nextThatNeedsYou } from './headsDown'
import { useShortcut } from './useShortcut'
import type { Bell } from './useNotices'
import { Icon } from './Icon'

/*
 * The bell.
 *
 * Nothing here invents an event. Every line is something the app already knew
 * and had nowhere to say: a run ended, an agent stopped and is waiting for a
 * person, a card went past its date, a step with no undo was set off. The bell
 * is the place they were missing.
 *
 * Read, never cleared. A list that empties itself is a list where the thing
 * you meant to come back to is gone — so opening the panel marks nothing, and
 * a line is read when you act on it or when you say so.
 */

const GLYPHS: Readonly<Record<string, React.JSX.Element>> = {
  run: <path d="m9 11 3 3L22 4M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />,
  agent: <path d="M12 8v4l3 2M12 3a9 9 0 1 0 9 9 9 9 0 0 0-9-9Z" />,
  due: <path d="M8 2v4M16 2v4M3 10h18M5 4h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z" />,
  irreversible: <path d="M12 9v4M12 17v.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />,
}

/* A kind this build does not know draws as a plain dot rather than failing:
   the set grows with whatever learns to notice something. */
const glyphFor = (kind: string): React.JSX.Element =>
  GLYPHS[kind] ?? <circle cx="12" cy="12" r="4" />

export function Notices({
  bell,
  open,
  setOpen,
  onOpenCard,
  onOpenPane,
}: {
  bell: Bell
  open: boolean
  setOpen: React.Dispatch<React.SetStateAction<boolean>>
  onOpenCard?: (cardId: string) => void
  /** Goes to the terminal a notice happened in, in its project. */
  onOpenPane?: (projectId: string | null, paneId: string) => void
}): React.JSX.Element {
  const box = useRef<HTMLDivElement>(null)
  useAway(box, () => setOpen(false), open)

  /* The terminal it happened in first: an agent waiting is answered there,
     and the card only says which piece of work it was. */
  const act = (one: Notice): void => {
    bell.markRead(one.id)
    if (one.paneId && onOpenPane) {
      setOpen(false)
      onOpenPane(one.projectId, one.paneId)
    } else if (one.cardId && onOpenCard) {
      setOpen(false)
      onOpenCard(one.cardId)
    }
  }

  /* One key to the thing that has been waiting longest for you, in the order
     somebody between five agents wants: an agent stopped for an answer, then a
     step with no undo, then a run that ended. It walks what is ringing — what
     a focus is holding is not something to be sent to. */
  useShortcut('next', () => {
    const next = nextThatNeedsYou(bell.notices)
    if (!next) return false
    act(next)
  })

  return (
    <div className="bell" ref={box}>
      <button
        className="bell__b"
        aria-label={bell.unread > 0 ? t('workspace.notices.unread', { count: bell.unread }) : t('workspace.notices.title')}
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
      >
        <Icon size="m">
          <path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9M13.7 21a2 2 0 0 1-3.4 0" />
        </Icon>
        {/* The count, capped: past a certain number the exact figure stops
            being information and starts being a wall of digits. */}
        {bell.unread > 0 && <span className="bell__n">{bell.unread > 99 ? '99+' : bell.unread}</span>}
      </button>

      {open && (
        <div className="bell__pop" role="menu">
          <div className="bell__top">
            <span className="bell__t">{t('workspace.notices.title')}</span>
            {bell.unread > 0 && (
              <button className="conv__act" onClick={bell.markAllRead}>
                {t('workspace.notices.markAll')}
              </button>
            )}
          </div>

          {bell.notices.length === 0 && bell.waiting.length === 0 && (
            <p className="bell__none">{t('workspace.notices.none')}</p>
          )}

          <div className="bell__list">
            {bell.notices.map((one) => (
              <button
                className="bell__one"
                key={one.id}
                role="menuitem"
                data-unread={one.readAt === null}
                data-kind={one.kind}
                onClick={() => act(one)}
              >
                <Icon size="s" className="bell__ico">
                  {glyphFor(one.kind)}
                </Icon>
                <span className="bell__body">
                  <span className="bell__line">{one.title}</span>
                  {one.detail && <span className="bell__d">{one.detail}</span>}
                </span>
                <span className="bell__when">{since(one.createdAt)}</span>
              </button>
            ))}
          </div>

          {/* What the focus is holding, where the rest of the notices are:
              looking at it is not leaving the focus, and nothing here is
              marked read by being seen. */}
          {bell.waiting.length > 0 && (
            <div className="bell__held">
              <span className="bell__t">{t('workspace.notices.held')}</span>
              {bell.waiting.map((one) => (
                <span className="bell__one" key={one.id} data-kind={one.kind}>
                  <Icon size="s" className="bell__ico">
                    {glyphFor(one.kind)}
                  </Icon>
                  <span className="bell__body">
                    <span className="bell__line">{one.title}</span>
                  </span>
                  <span className="bell__when">{since(one.createdAt)}</span>
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
