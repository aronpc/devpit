import { useEffect, useRef, useState } from 'react'

import { Markdown } from './MarkdownView'
import { keepWidth, keptWidth, PEEK_DEFAULT, peekWidth } from './peekWidth'
import { useFile } from './useFile'
import { useShellPick } from './shellStore'
import { t } from '../i18n'

/*
 * A file an answer points at, read beside the chat rather than instead of it:
 * the conversation stays in view while the document is read. Expanding opens
 * it as a tab of its own, editable, where it can take the whole width.
 */
export function DocPeek({ path, onOpen, onClose }: { path: string; onOpen: (path: string) => void; onClose: () => void }): React.JSX.Element {
  const show = useShellPick((shell) => shell.show)
  const file = useFile(path)
  const name = path.split('/').pop() ?? path
  const text = file.file?.text ?? null
  const markdown = /\.(md|markdown|mdx)$/i.test(path)
  const side = useRef<HTMLElement>(null)
  const [width, setWidth] = useState(keptWidth)

  /* The pane makes room for it: the chat narrows beside it instead of
     running on underneath, and its corner moves with it. */
  useEffect(() => {
    const pane = side.current?.parentElement
    if (!pane) return
    const fitted = peekWidth(width, pane.clientWidth)
    pane.style.setProperty('--peek-w', `${fitted}px`)
    return () => {
      pane.style.removeProperty('--peek-w')
    }
  }, [width])

  const drag = (event: React.PointerEvent<HTMLDivElement>): void => {
    const pane = side.current?.parentElement
    if (!pane) return
    event.currentTarget.setPointerCapture(event.pointerId)
    const right = pane.getBoundingClientRect().right
    const move = (to: PointerEvent): void => setWidth(peekWidth(right - to.clientX, pane.clientWidth))
    const up = (): void => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      setWidth((was) => (keepWidth(was), was))
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }

  return (
    <aside className="peek" ref={side} aria-label={t('workspace.peek.preview', { name })}>
      <div
        className="peek__grip"
        role="separator"
        aria-orientation="vertical"
        aria-label={t('workspace.peek.resize')}
        title={t('workspace.peek.grip')}
        onPointerDown={drag}
        onDoubleClick={() => (setWidth(PEEK_DEFAULT), keepWidth(PEEK_DEFAULT))}
      />
      <header className="peek__bar">
        <span className="peek__name" title={path}>
          {name}
        </span>
        <button
          className="sq26"
          title={t('workspace.peek.openTab')}
          aria-label={t('workspace.peek.openTab')}
          onClick={() => (show('file', { id: `file:${path}`, path, title: name }), onClose())}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" /></svg>
        </button>
        <button className="sq26" title={t('workspace.peek.closeTitle')} aria-label={t('workspace.peek.close')} onClick={onClose}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
        </button>
      </header>
      <div className="peek__body">
        {file.error ? (
          <p className="peek__note">{file.error}</p>
        ) : text === null ? (
          <p className="peek__note">{file.file?.notShown ?? t('workspace.peek.reading')}</p>
        ) : markdown ? (
          <Markdown source={text} path={path} opens={onOpen} />
        ) : (
          <pre className="peek__text">{text}</pre>
        )}
      </div>
    </aside>
  )
}
