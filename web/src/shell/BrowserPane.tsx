import { useCallback, useEffect, useRef, useState } from 'react'

import { BrowserFind } from './BrowserFind'
import { aim, boxOf, covered, inset, moved, shown, type Viewport, VIEWPORTS, type Where } from './browsing'
import type { Did, Drove, Showing } from '../gen/bindings'
import { ask, commands } from './live'
import type { Tab } from './strip'
import { useShell } from './useShell'
import { listen } from './useShortcut'
import { onCarried } from './window'
import { t } from '../i18n'

/*
 * A page, in a webview that is not ours.
 *
 * What this component draws is a **hole**. The page itself is a second native
 * webview inside the same window, placed over this element in window
 * coordinates — it is not an iframe and it is not inside this document, which
 * is the whole point: the page cannot reach what devpit draws, and devpit's
 * commands are scoped to the `main` webview so the page cannot call them
 * either.
 *
 * Two consequences the rest of the file is about:
 *
 * 1. **Nothing moves it but us.** A native webview knows nothing about the
 *    layout that decided where it goes, so every resize and every scroll has
 *    to be measured here and sent over.
 * 2. **Nothing hides it but us.** It floats above the window. A pane that
 *    scrolls out of view, or a tab that stops being the visible one, leaves a
 *    page sitting over whatever is there now unless this closes it.
 */

export function BrowserPane({ tab }: { tab: Tab }): React.JSX.Element {
  const { rename, managing, prefs, palette } = useShell()
  const over = covered({ managing, prefs, palette })
  const box = useRef<HTMLDivElement | null>(null)
  const placed = useRef<Where | null>(null)
  const open = useRef(false)
  /* The control the menu window hangs from. Rust needs its rectangle to put
     the window under it, and only this document can measure it. */
  const dots = useRef<HTMLButtonElement | null>(null)

  const [typed, setTyped] = useState('')
  const [at, setAt] = useState('')
  const [refused, setRefused] = useState<string | null>(null)
  const [finding, setFinding] = useState(false)
  /* A width to hold the page at, for looking at a layout that is not this
     window's. Null is the pane itself, which is what a browser pane is. */
  const [viewport, setViewport] = useState<Viewport | null>(null)
  /* Which session this pane is in, visible rather than buried in settings:
     the failure it prevents is somebody signed into the wrong account and
     having no way to see why. */
  const [session, setSession] = useState('default')
  /* Off, and only a person turns it on. An agent that asks about a pane it
     was not given is told no, so the refusal reaches its transcript. */
  const [granted, setGranted] = useState(false)
  const [drove, setDrove] = useState<string | null>(null)

  /* Measured, not derived: see the note above.
   *
   * And it does one more thing than it looks like. A pane that is not the
   * active tab is hidden with `display: none` (`Panes.tsx`, `data-show`), so
   * this component **never unmounts** and the cleanup below never runs. The
   * page would go on living as a 1×1 webview at the window's corner — still
   * loaded, still running script, still drivable by an agent. A hidden pane
   * measures 0×0, and that is the signal to close it; coming back measures
   * real again, and the url is reopened. */
  const place = useCallback((): void => {
    const node = box.current
    if (!node) return
    const hole = boxOf(node.getBoundingClientRect())
    const now = inset(hole, viewport)
    const shown = hole.width > 0 && hole.height > 0 && !over

    if (!shown) {
      if (open.current) {
        open.current = false
        placed.current = null
        void ask(() => commands.browserClose(tab.id))
      }
      return
    }
    if (!open.current) {
      /* Back on screen: the page comes back where it was. */
      if (!at) return
      placed.current = now
      void ask(() => commands.browserOpen(tab.id, at, now, session)).then((answer) => {
        if (answer.error === null) open.current = true
      })
      return
    }
    if (!moved(placed.current, now)) return
    placed.current = now
    void ask(() => commands.browserPlace(tab.id, now))
  }, [at, over, session, tab.id, viewport])

  const go = useCallback(
    (where: string, into: string = session): void => {
      const aimed = aim(where)
      if ('refused' in aimed) {
        setRefused(aimed.refused)
        return
      }
      const node = box.current
      if (!node) return
      setRefused(null)
      const now = inset(boxOf(node.getBoundingClientRect()), viewport)
      placed.current = now
      void ask(() => commands.browserOpen(tab.id, aimed.at, now, into)).then((answer) => {
        if (answer.error !== null) {
          /* A page that will not load says so here rather than leaving a blank
             rectangle, which is what a person reads as the app being broken. */
          setRefused(answer.error)
          return
        }
        open.current = true
        setAt(aimed.at)
        setTyped(shown(aimed.at))
      })
    },
    [session, tab.id, viewport],
  )

  /* Where the page actually is. A link, a redirect or a form leaves the last
     url devpit asked for saying where the page was *sent*, not where it *is* —
     so the bar follows the webview rather than the request. */
  useEffect(
    () =>
      onCarried<Showing>('browser:showing', (said) => {
        if (said.pane !== tab.id) return
        setAt(said.url)
        setTyped(shown(said.url))
        setRefused(null)
        /* The page names its own tab once it has a name. A page that calls
           itself nothing leaves the tab on the address, which is what a
           browser has always done. */
        rename(tab.id, said.title.trim() || shown(said.url))
      }),
    [rename, tab.id],
  )

  /* What was done in the menu window, on its way back.
   *
   * The menu is another document and cannot reach this component's state, so
   * everything it changes about the *pane* arrives here. The commands that
   * need no pane — listing sessions, reading a store, granting — it called
   * itself, which is why this handles four cases and not ten. */
  useEffect(
    () =>
      onCarried<[string, Did]>('browser:menu-did', ([whose, what]) => {
        if (whose !== tab.id) return
        switch (what.did) {
          case 'session':
            setSession(what.session)
            /* A webview's data directory is fixed when it is built, so the
               page is opened again in the session that was chosen. Without
               this the menu would say one thing and the cookies would be
               another's — the wrong-account failure sessions exist to stop. */
            open.current = false
            if (at) go(at, what.session)
            return
          case 'viewport':
            setViewport(VIEWPORTS.find((one) => one.id === what.viewport) ?? null)
            return
          case 'grant':
            setGranted(what.granted)
            void ask(() => commands.browserGrant(tab.id, what.granted))
            if (!what.granted) setDrove(null)
            return
          case 'said':
            setRefused(what.said)
        }
      }),
    [at, go, tab.id],
  )

  /* What an agent is doing to this page, drawn as it happens rather than
     reported once the page has already changed. */
  useEffect(
    () =>
      onCarried<Drove>('browser:driven', (said) => {
        if (said.pane !== tab.id) return
        setDrove(said.said)
      }),
    [tab.id],
  )

  /* The latest `place`, reachable from listeners that are registered once.
   *
   * Without this the effect below depended on `place`, which depends on `at`
   * — so **every page load tore the webview down and built a new one**. The
   * cleanup closes the webview unconditionally, the observers then reopened
   * it at the same address, and the round trip threw away the history: back
   * and forward had nothing to go back to after the first link. The pane
   * looked like it worked, because the page it landed on was the right one.
   *
   * The listeners belong to the pane and should be registered for as long as
   * the pane exists, which is what `[tab.id]` now says. */
  const latest = useRef(place)
  useEffect(() => {
    latest.current = place
    /* And a change in what `place` would do — a new page width, a new
       address — is applied at once rather than waiting for the next resize
       or scroll to notice. */
    place()
  }, [place])

  /* The webview follows the pane, and goes away with it. */
  useEffect(() => {
    const node = box.current
    if (!node) return undefined
    const put = (): void => latest.current()

    const watchers: Array<{ disconnect: () => void }> = []
    if (typeof ResizeObserver !== 'undefined') {
      const size = new ResizeObserver(put)
      size.observe(node)
      watchers.push(size)
    }
    /* `display: none` is not a resize on every engine, so the attribute that
       causes it is watched directly. Without this a tab switch leaves the
       page running behind whatever is on screen. */
    if (typeof MutationObserver !== 'undefined' && node.parentElement) {
      const shown = new MutationObserver(put)
      shown.observe(node.parentElement, { attributes: true, attributeFilter: ['data-show'] })
      watchers.push(shown)
    }
    /* Scrolling moves the hole without resizing it, and a window resize moves
       every hole at once. Neither reaches a ResizeObserver on this element. */
    window.addEventListener('resize', put)
    window.addEventListener('scroll', put, true)

    /* Ctrl-F, while devpit's own chrome has the keyboard. It cannot work while
       the page has focus — the key goes to the other webview and this document
       never sees it — which is why the bar has a button for the same thing and
       does not rely on the shortcut. */
    const unfind = listen('find', () => {
      if (node.getBoundingClientRect().width === 0) return false
      setFinding(true)
    })

    return () => {
      window.removeEventListener('resize', put)
      window.removeEventListener('scroll', put, true)
      unfind()
      for (const watcher of watchers) watcher.disconnect()
      /* Unconditionally: a webview left behind floats over whatever the window
         shows next, and nothing else in the app knows it is there. */
      open.current = false
      placed.current = null
      void ask(() => commands.browserClose(tab.id))
    }
  }, [tab.id])

  return (
    <div className="browser">
      <div className="browser__bar">
        <button
          type="button"
          className="browser__act"
          aria-label={t('workspace.browserPane.back')}
          title={t('workspace.browserPane.back')}
          disabled={!at}
          onClick={() => void ask(() => commands.browserBack(tab.id))}
        >
          ‹
        </button>
        <button
          type="button"
          className="browser__act"
          aria-label={t('workspace.browserPane.forward')}
          title={t('workspace.browserPane.forward')}
          disabled={!at}
          onClick={() => void ask(() => commands.browserForward(tab.id))}
        >
          ›
        </button>
        <button
          type="button"
          className="browser__act"
          aria-label={t('workspace.browserPane.reload')}
          title={t('workspace.browserPane.reload')}
          disabled={!at}
          onClick={() => at && go(at)}
        >
          ⟳
        </button>
        <button
          type="button"
          className="browser__act"
          aria-label={t('workspace.browserPane.stop')}
          title={t('workspace.browserPane.stop')}
          disabled={!at}
          onClick={() => void ask(() => commands.browserStop(tab.id))}
        >
          ✕
        </button>
        {/* Find takes the address bar's place rather than adding a row.
            A row of its own changed the pane's height, which moves the hole,
            which resizes the native webview — and a page being re-laid out
            reads as the page reloading. The bar is the same height either
            way, so nothing under it moves. */}
        {finding ? (
          <BrowserFind pane={tab.id} onDone={() => setFinding(false)} />
        ) : (
          <form
            className="browser__where"
            onSubmit={(event) => {
              event.preventDefault()
              go(typed)
            }}
          >
            <input
              className="browser__address"
              aria-label={t('workspace.browserPane.address')}
              placeholder={t('workspace.browserPane.addressExample')}
              spellCheck={false}
              value={typed}
              onChange={(event) => setTyped(event.target.value)}
            />
          </form>
        )}
        <button
          type="button"
          className="browser__act"
          aria-label={t('workspace.browserPane.find')}
          title={t('workspace.browserPane.find')}
          aria-pressed={finding}
          disabled={!at}
          onClick={() => setFinding((was) => !was)}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-4.3-4.3" />
          </svg>
        </button>
        {/* The menu is a window, not a panel in this document. A pane's page
            is a second native webview and two native webviews have no z-order
            between them, so a panel drawn here came out behind the site. This
            button sends its own rectangle and Rust puts a small window under
            it. `browser_menu.rs` has the whole of why. */}
        <button
          type="button"
          className="browser__act"
          aria-label={t('workspace.browserPane.menu')}
          title={t('workspace.browserPane.menu')}
          aria-haspopup="menu"
          ref={dots}
          onClick={() => {
            /* `rect` and not `at`: `at` is this pane's url, and shadowing it
               here is how the menu was told the page was a DOMRect. */
            const rect = dots.current?.getBoundingClientRect()
            if (!rect) return
            void ask(() =>
              commands.browserMenuShow(tab.id, boxOf(rect), {
                pane: tab.id,
                at,
                session,
                viewport: viewport?.id ?? null,
                granted,
              }),
            )
          }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="19" cy="12" r="1.8" /></svg>
        </button>
      </div>
      {/* The hole. The page is drawn over this by the window, not by React. */}
      <div className="browser__page" ref={box} data-pane-id={tab.id} />
      {granted && drove && (
        <div className="browser__drove" role="status">
          {t('workspace.browserPane.agentDid', { what: drove })}
        </div>
      )}
      {refused && (
        <div className="browser__said" role="status">
          {refused}
        </div>
      )}
    </div>
  )
}
