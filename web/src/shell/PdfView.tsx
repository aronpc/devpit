import { useEffect, useRef, useState } from 'react'

import { ask, commands } from './live'
import { t } from '../i18n'

/*
 * A PDF, drawn page by page.
 *
 * Not an `<object>`: the window's content policy refuses plugins, and the
 * Linux webview has no PDF viewer to embed anyway, so the pane stayed empty.
 * pdf.js draws each page into a canvas the same way in every webview, and is
 * loaded only when a PDF is opened. A file it cannot read says so, with the
 * way out: the system's own viewer.
 */

/** How many pages are drawn before "more" is asked for. */
export const PAGES_AT_ONCE = 10

type Loaded = { pages: number; draw: (page: number, into: HTMLCanvasElement, width: number) => Promise<void> }

/** The bytes a `data:` URL carries. */
export function bytesOf(dataUrl: string): Uint8Array {
  const base64 = dataUrl.slice(dataUrl.indexOf(',') + 1)
  const raw = atob(base64)
  const bytes = new Uint8Array(raw.length)
  for (let at = 0; at < raw.length; at++) bytes[at] = raw.charCodeAt(at)
  return bytes
}

async function load(dataUrl: string): Promise<Loaded> {
  const [pdfjs, worker] = await Promise.all([import('pdfjs-dist'), import('pdfjs-dist/build/pdf.worker.min.mjs?url')])
  pdfjs.GlobalWorkerOptions.workerSrc = worker.default
  const doc = await pdfjs.getDocument({ data: bytesOf(dataUrl) }).promise
  return {
    pages: doc.numPages,
    draw: async (number, into, width) => {
      const page = await doc.getPage(number)
      const natural = page.getViewport({ scale: 1 })
      const ratio = window.devicePixelRatio || 1
      const viewport = page.getViewport({ scale: (width / natural.width) * ratio })
      into.width = Math.floor(viewport.width)
      into.height = Math.floor(viewport.height)
      into.style.width = `${Math.floor(viewport.width / ratio)}px`
      await page.render({ canvas: into, viewport }).promise
    },
  }
}

export function PdfView({ dataUrl, fullPath, name }: { dataUrl: string; fullPath: string; name: string }): React.JSX.Element {
  const [loaded, setLoaded] = useState<Loaded | null>(null)
  const [failed, setFailed] = useState(false)
  const [shown, setShown] = useState(PAGES_AT_ONCE)

  useEffect(() => {
    let gone = false
    setLoaded(null)
    setFailed(false)
    load(dataUrl).then(
      (doc) => !gone && setLoaded(doc),
      () => !gone && setFailed(true),
    )
    return () => {
      gone = true
    }
  }, [dataUrl])

  const outside = (
    <button className="btn" onClick={() => void ask(() => commands.pathOpen(fullPath))}>
      {t('workspace.pdf.openOutside')}
    </button>
  )

  if (failed) {
    return (
      <div className="exempty">
        <span className="exempty__t">{t('workspace.pdf.cannotDraw', { name })}</span>
        {outside}
      </div>
    )
  }
  if (!loaded) return <div className="exempty__t">{t('workspace.pdf.reading')}</div>

  return (
    <div className="pdf">
      <div className="pdf__bar">
        <span>
          {t('workspace.pdf.pages', { count: loaded.pages })}
        </span>
        {outside}
      </div>
      {Array.from({ length: Math.min(shown, loaded.pages) }, (_, at) => (
        <Page key={at} number={at + 1} doc={loaded} />
      ))}
      {loaded.pages > shown && (
        <button className="btn pdf__more" onClick={() => setShown((was) => was + PAGES_AT_ONCE)}>
          {t('workspace.pdf.more', { count: Math.min(PAGES_AT_ONCE, loaded.pages - shown) })}
        </button>
      )}
    </div>
  )
}

function Page({ number, doc }: { number: number; doc: Loaded }): React.JSX.Element {
  const canvas = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    const into = canvas.current
    if (!into) return
    const width = Math.min(into.parentElement?.clientWidth ?? 800, 1100) - 32
    void doc.draw(number, into, Math.max(200, width)).catch(() => undefined)
  }, [number, doc])
  return <canvas ref={canvas} className="pdf__page" aria-label={t('workspace.pdf.page', { number })} />
}
