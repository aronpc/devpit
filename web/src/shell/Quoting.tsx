import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { t } from '../i18n'

/* A selection as a quote for the next message: each line marked, a blank line after. */
export const quoted = (text: string): string => `${text.split('\n').map((line) => `> ${line}`).join('\n')}\n\n`

/* Words selected in an answer, offered as a quote over where they were picked. */
export function Quoting({ box, onQuote }: { box: React.RefObject<HTMLDivElement | null>; onQuote: (text: string) => void }): React.JSX.Element | null {
  const [picked, setPicked] = useState<{ top: number; left: number; text: string } | null>(null)

  useEffect(() => {
    const view = box.current
    if (!view) return
    const read = (): void => {
      const chosen = window.getSelection()
      const text = chosen?.toString().trim() ?? ''
      const range = chosen && chosen.rangeCount > 0 ? chosen.getRangeAt(0) : null
      const node = range?.commonAncestorContainer
      const element = node instanceof Element ? node : node?.parentElement
      if (!text || !range || !element?.closest('.reply') || !view.contains(element)) return setPicked(null)
      const rect = range.getBoundingClientRect()
      setPicked({ top: rect.top, left: rect.left + rect.width / 2, text })
    }
    const gone = (): void => {
      if (window.getSelection()?.isCollapsed ?? true) setPicked(null)
    }
    const hide = (): void => setPicked(null)
    view.addEventListener('mouseup', read)
    view.addEventListener('keyup', read)
    view.addEventListener('scroll', hide)
    document.addEventListener('selectionchange', gone)
    return () => {
      view.removeEventListener('mouseup', read)
      view.removeEventListener('keyup', read)
      view.removeEventListener('scroll', hide)
      document.removeEventListener('selectionchange', gone)
    }
  }, [box])

  if (!picked) return null
  return createPortal(
    <button
      className="qbtn"
      style={{ top: Math.max(0, picked.top - 34), left: picked.left }}
      /* Pressed without taking the selection away first. */
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => {
        onQuote(quoted(picked.text))
        setPicked(null)
        window.getSelection()?.removeAllRanges()
      }}
    >
      {t('workspace.quoting.quote')}
    </button>,
    document.body,
  )
}
