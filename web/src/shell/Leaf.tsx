import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { useCallback, useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { attachPane, retryIn, type Pane } from './attach'
import { UNAVAILABLE } from './paneSocket'
import { writeBatcher, type Batcher } from './writeBatcher'
import type { Screen } from './floodCut'
import { gpuWhileShown, measureWideCharacters } from './terminalAddons'
import { reason } from './reason'
import { ask, commands } from './live'
import { contrastFor, darkNow, options, palette } from './terminal'
import { clipboardKey, copySelection, pasteClipboard } from './terminalClipboard'
import { TerminalMenu } from './TerminalMenu'
import { guardComposition } from './terminalIme'
import { picturesAsPaths } from './terminalPaste'
import { followAfterClear } from './terminalFollow'
import { wheelFor } from './terminalWheel'
import { useMarks } from './useMarks'
import { menuPoint } from './menuRules'
import { useShellPick } from './shellStore'
import { linkPaths } from './terminalPathLinks'
import { shellWord, useFileDrop } from './fileDrop'

/*
 * One terminal, attached to one pane.
 *
 * A leaf of the tab's tree. It knows nothing about the tree: which pane it is
 * and which project it belongs to is all it takes, so a split rearranging its
 * siblings never tears it down and loses its attachment.
 */

/** Said when a view that took a pane from its tab lets it go. */
export const PANE_FREED = 'devpit:pane-freed'

/* Off screen, a pane lets go of its stream at once and the bytes wait in the
   pty's ring. Off this long, it gives back its rows too, but keeps the
   terminal and, under the ceiling, its GPU context: building six again took
   over a second, and six contexts another 200 ms. */
export const HIDDEN_RELEASE_MS = 20_000

/* What an attach wrote, kept to rebuild the terminal around an older page.
   Past this, paging ends rather than holding a flood in memory. */
const HELD_MOST = 2 * 1024 * 1024

/* A replay longer than this draws only its end, enough for the screen; the
   rest is drawn when scrolled to, like an older page. */
export const REPLAY_FIRST = 64 * 1024

const longReplay = (bytes: Uint8Array): boolean => bytes.length > REPLAY_FIRST

/** Where the end of a long replay starts drawing: at an ESC or after a
 *  newline, the first past the cut, as the daemon cuts its own history. */
export function drawFrom(bytes: Uint8Array): number {
  const from = Math.max(0, bytes.length - REPLAY_FIRST)
  for (let at = from; at < bytes.length; at += 1) {
    if (bytes[at] === 0x1b) return at
    if (bytes[at] === 0x0a) return at + 1
  }
  let at = from
  while (at < bytes.length && (bytes[at]! & 0xc0) === 0x80) at += 1
  return at
}

export function Leaf({
  paneId,
  projectId,
  onSplit,
  onClosePane,
  onTerminal,
  onBlocks,
  onSeparate,
  cwd = null,
  target,
}: {
  paneId: string
  projectId: string
  /** What the right-click menu offers for the pane, from the tab that owns it. */
  onSplit?: (direction: 'horizontal' | 'vertical') => void
  onClosePane?: () => void
  /** The live terminal, to whoever draws around it (`BlockTerm`). */
  onTerminal?: (terminal: Terminal | null) => void
  /** Offered on the right-click when the pane is drawn plain: back to blocks. */
  onBlocks?: () => void
  onSeparate?: () => void
  /** The shell's folder, when it says so: where a relative path it prints is. */
  cwd?: string | null
  /** The pane's target in the layout, which says whether tmux draws it. */
  target?: string
}): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null)
  const [error, setError] = useState<string | null>(null)
  /* Beside the terminal, not instead of it: a paste that failed leaves the
     terminal as usable as it was. */
  const [pasteFailed, setPasteFailed] = useState<string | null>(null)
  const [term, setTerm] = useState<Terminal | null>(null)
  const [menuAt, setMenuAt] = useState<{ x: number; y: number } | null>(null)
  /* Another view held this pane a while — a session opened beside an
     orchestrator's chat — and gave it back: attached again, from scratch. */
  const [round, setRound] = useState(0)
  useEffect(() => {
    const back = (event: Event): void => {
      if ((event as CustomEvent<string>).detail === paneId) setRound((was) => was + 1)
    }
    window.addEventListener(PANE_FREED, back)
    return () => window.removeEventListener(PANE_FREED, back)
  }, [paneId])
  const onTerminalRef = useRef(onTerminal)
  onTerminalRef.current = onTerminal
  /* Where a path clicked in the terminal opens: read at the click, so the
     terminal is not rebuilt when the project or the tabs change. */
  const opening = useShellPick((shell) => ({ show: shell.show, root: shell.project?.rootPath ?? null }))
  const openingRef = useRef({ ...opening, cwd })
  openingRef.current = { ...opening, cwd }

  useEffect(() => {
    const box = host.current
    if (!projectId || !box) return

    const terminal = new Terminal(options(darkNow()))
    /* The person's contrast, when they chose one. Read after the terminal is
       up rather than before: a settings read must not hold the pane empty. */
    let contrast: number | null = null
    void ask(() => commands.settingsRead()).then((answer) => {
      contrast = answer.data?.terminalContrast ?? null
      terminal.options.minimumContrastRatio = contrastFor(darkNow(), contrast)
    })
    const fit = new FitAddon()
    terminal.loadAddon(fit)
    terminal.open(box)
    const linked = linkPaths(terminal, openingRef)
    const follows = followAfterClear(terminal, () => rewriting, () => (wiped = true))
    // After `open`, which is when there is a canvas to take a context from.
    measureWideCharacters(terminal)
    /* tmux sends only what changes, so a screen that was emptied under it —
       a rebuilt terminal, a lost GPU context, a pane coming back — stays blank
       but for the cursor until the program next prints. Asked for whole. */
    const redraw = (tries = 5): void =>
      void ask(() => commands.sessionRedraw(projectId, paneId)).then((drawn) => {
        /* A client still connecting is not listed yet: asked again shortly. */
        if (drawn.data === false && tries > 1 && !dropped) setTimeout(() => redraw(tries - 1), 200)
      })
    const gpu = gpuWhileShown(terminal, () => redraw())
    /* Where there is no observer to say the pane is on screen, it is assumed
       to be; otherwise the observer's first answer turns the GPU on. */
    if (typeof IntersectionObserver === 'undefined') gpu.show()
    const pasted = picturesAsPaths(projectId, (path) => terminal.paste(path), setPasteFailed)
    box.addEventListener('paste', pasted, true)
    /* Copy and paste keys, taken before xterm's textarea sees them: on
       WebKitGTK neither reaches a clipboard from there (`terminalClipboard`). */
    const keys = (event: KeyboardEvent): void => {
      const act = clipboardKey(event)
      if (!act) return
      event.preventDefault()
      event.stopPropagation()
      if (act === 'copy') void copySelection(terminal)
      else void pasteClipboard(terminal, projectId).then(setPasteFailed)
    }
    box.addEventListener('keydown', keys, true)
    /* The terminal's own right-click, kept from the window's menu, which
       leaves text fields — and xterm's input is one — to the webview. */
    const menu = (event: MouseEvent): void => {
      event.preventDefault()
      event.stopPropagation()
      setMenuAt(menuPoint(event))
    }
    box.addEventListener('contextmenu', menu, true)
    const unguard = guardComposition(box, (data) => terminal.input(data, true))
    const wheel = wheelFor(target, terminal, box, projectId, paneId)

    /* The pane is `display: none` until its tab is active and animates in on a
       transform, so a fit in this tick measures nothing and xterm ends up with
       zero columns. Fit only once the box has a size. */
    let live: Pane | null = null
    let dropped = false
    /* What the pty was last told, not what the grid was a moment ago: a fit
       that ran while nothing was attached yet — or on coming back into view,
       where the box did not change size — moved the grid and told nobody, and
       every later comparison against the grid found nothing to send. The pty
       stayed at 80×24 under a terminal twice as wide. */
    let told: { rows: number; cols: number } | null = null
    /* A resize sent while the attach is still being claimed is refused; it is
       asked again shortly, a few times, rather than lost. */
    let retries = 0
    /* The grid follows the box at once; the pty hears about it once the box
       has stopped moving. A split animating open is a dozen sizes in a few
       frames, and each one is a SIGWINCH the agent answers by redrawing —
       Claude Code clears its screen to do that, and a burst of them left it
       blank or drawn at a size already gone. */
    let settling: ReturnType<typeof setTimeout> | undefined
    let releasing: ReturnType<typeof setTimeout> | undefined
    let again: ReturnType<typeof setTimeout> | undefined
    /* At most once a frame: the observer fires for every pixel of a drag or
       of the side panel sliding, and each fit measures the box. */
    let framed = false
    const refit = (): void => {
      if (framed) return
      framed = true
      requestAnimationFrame(() => {
        framed = false
        if (dropped) return
        if (box.clientWidth >= 2 && box.clientHeight >= 2) fit.fit()
        clearTimeout(settling)
        wheel.dispose()
        settling = setTimeout(tell, 60)
      })
    }
    const tell = (): void => {
      const { rows, cols } = terminal
      if (dropped || !live || (told?.rows === rows && told.cols === cols)) return
      told = { rows, cols }
      void live.resize(rows, cols).then((applied) => {
        /* The pty clamps; the grid follows what it actually got. */
        if (applied && (applied.rows !== terminal.rows || applied.cols !== terminal.cols)) {
          told = { rows: applied.rows, cols: applied.cols }
          terminal.resize(applied.cols, applied.rows)
        }
        retries = 0
      }, () => {
        told = null
        if (!dropped && retries++ < 10) setTimeout(refit, 200)
      })
    }
    requestAnimationFrame(refit)

    /* Live output at most once a frame; parsed bytes acknowledged, which is
       what lets the pty read on. One per attach, so no count crosses over. */
    let batch: Batcher | null = null
    let held: Uint8Array[] | null = []
    let heldBytes = 0
    /* The start of a long replay, held but not drawn until scrolled to. */
    let unseen = false
    const hold = (bytes: Uint8Array): void => {
      if (!held) return
      heldBytes += bytes.length
      if (heldBytes > HELD_MOST) held = null
      else held.push(bytes)
    }
    let paging = false
    let more = true
    /* Held bytes being written again, and whether a clear of the scrollback
       was among them. Not the wait for a page: a clear then is the program's. */
    let rewriting = false
    let wiped = false
    /* xterm cannot prepend, so it is reset in order — after whatever is still
       waiting — and written again whole around the older page. */
    const rewrite = (paged = false): void => {
      if (!held) return
      const lines = terminal.buffer.active.length
      batch?.flush()
      /* The reset scrolls to the top too: no page is asked for until the
         rewrite is in. */
      paging = true
      rewriting = true
      wiped = false
      terminal.write('\x1bc')
      for (const one of held) terminal.write(one)
      terminal.write(new Uint8Array(), () => {
        const gained = terminal.buffer.active.length - lines
        /* Nothing above because a later clear wiped it: older pages would be
           wiped too, and each would reset the screen and pull the view up.
           A page redrawn in place draws nothing either, with rows before it. */
        if (gained <= 0 && wiped) more = false
        terminal.scrollToLine(Math.max(0, gained))
        rewriting = false
        paging = false
        /* The view is still at the top, where no scroll asks again. */
        if (paged && gained <= 0 && more) void page()
      })
      if (heldBytes > HELD_MOST) held = null
    }
    /* Scrolled to the top of a plain screen: the page before. */
    const page = async (): Promise<void> => {
      if (!live || !held || paging || terminal.buffer.active.type !== 'normal') return
      if (unseen) {
        unseen = false
        return rewrite()
      }
      if (!more) return
      paging = true
      const got = await live.older()
      paging = false
      if (dropped || !held) return
      if (!got || got.bytes.length === 0) {
        more = false
        return
      }
      more = got.left > 0
      held.unshift(got.bytes)
      heldBytes += got.bytes.length
      rewrite(true)
    }
    const scrolled = terminal.onScroll((top) => {
      if (top === 0) void page()
    })
    let drawn = false
    /* The screen still shows an earlier attach: reset before the next one draws. */
    let stale = false
    const draw = (bytes: Uint8Array, replay: boolean): void => {
      if (stale) {
        stale = false
        /* The reset scrolls to the top, which is not the person asking for a page. */
        paging = true
        terminal.write('\x1bc')
        terminal.write(new Uint8Array(), () => {
          paging = false
        })
      }
      hold(bytes)
      /* The replay comes before any live byte and is not acknowledged. */
      if (replay) batch?.direct(longReplay(bytes) ? bytes.subarray(drawFrom(bytes)) : bytes)
      else batch?.push(bytes)
      if (replay && longReplay(bytes)) unseen = true
      if (!drawn) {
        drawn = true
        tries = 0
        box.dataset.drawn = 'true'
        setError(null)
      }
    }

    /* A flood is cut only with the view at the bottom: scrolled up, the
       person is reading rows a cut would move. The pane can grow before the
       cut is parsed, at most to the rows the display holds at this font. */
    const floodScreen = (): Screen => {
      const { active } = terminal.buffer
      if (active.viewportY !== active.baseY) return null
      const row = (terminal.options.fontSize ?? 14) * (terminal.options.lineHeight ?? 1)
      const tallest = Math.ceil(window.screen.height / row)
      return { rows: terminal.rows, scrollback: terminal.options.scrollback ?? 0, tallest }
    }

    /* On screen, the pane streams; off it, it lets go at once. */
    let attempt = 0
    let tries = 0
    let shownOnce = false
    let visible = false
    const connect = (): void => {
      if (dropped || live || batch) return
      const mine = ++attempt
      let pane: Pane | null = null
      batch = writeBatcher(terminal, (bytes) => pane?.ack(bytes), floodScreen)
      held = []
      heldBytes = 0
      unseen = false
      more = true
      void attachPane(
        projectId,
        paneId,
        { rows: terminal.rows, cols: terminal.cols },
        {
          output: (bytes, replay) => {
            if (!dropped && mine === attempt) draw(bytes, replay)
          },
          /* The pane ended. A failure here is the one the person needs to see:
             it is why the terminal is empty. A connection that closed under
             it is attached again in a moment. */
          end: (failed) => {
            if (dropped || mine !== attempt) return
            const wait = retryIn(failed, tries)
            if (wait === null) return void (failed && setError(failed))
            disconnect()
            tries += 1
            if (failed === UNAVAILABLE && wait > 0) setError(t('workspace.leaf.trying'))
            again = setTimeout(() => visible && connect(), wait)
          },
        },
      ).then(
        (attached) => {
          /* A mount dropped while that was on its way must not keep the pane:
             an attach arriving after the live mount's is the newer claim, wins
             the pane, and is let go at once — leaving the terminal on screen
             with nobody attached and "a newer client already owns that pane".
             React mounts twice in development, so there it was every restart. */
          if (dropped || mine !== attempt) return attached?.detach()
          pane = attached
          live = attached
          told = { rows: terminal.rows, cols: terminal.cols }
          /* The grid may have been fitted while the attach was on its way. */
          refit()
          setTimeout(() => !dropped && redraw(), 150)
          if (shownOnce) return
          shownOnce = true
          setTerm(terminal)
          onTerminalRef.current?.(terminal)
        },
        /* Every step above talks to the backend, and a rejection here used to
           vanish: the pane mounted, drew a cursor, and never attached. */
        (thrown: unknown) => {
          if (!dropped && mine === attempt) setError(reason(thrown, t('workspace.leaf.cannotOpen')))
        },
      )
    }
    /* Whatever the attach on its way says from here is not heard; the bytes
       wait in the pty's ring, and coming back draws them over a reset screen. */
    const disconnect = (): void => {
      attempt += 1
      clearTimeout(again)
      batch?.dispose()
      batch = null
      live?.detach()
      live = null
      stale ||= drawn
      drawn = false
      delete box.dataset.drawn
    }
    terminal.onData((data) => wheel.typed(data, (typed) => live?.write(typed)))

    /* The theme changes under a live terminal. Repainting the palette is
       enough — rebuilding the pane would drop the scrollback and the process
       along with it. */
    const themed = new MutationObserver(() => {
      terminal.options.theme = palette(darkNow())
      terminal.options.minimumContrastRatio = contrastFor(darkNow(), contrast)
    })
    themed.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })

    /* The observer fires for every pixel of a drag, per visible pane. The pty
       only cares about the grid, so it is told only when rows or columns
       actually changed. */
    const watch = new ResizeObserver(refit)
    watch.observe(box)

    /* A pane is `display: none` while another tab is in front. The rows
       survive that; the pixels do not, and coming back the grid is the same
       size it was — so the `ResizeObserver` above says nothing and the
       terminal stays blank until a click happens to force a render.
       Coming back into view is the force. */
    const shown =
      typeof IntersectionObserver === 'undefined'
        ? null
        : new IntersectionObserver((entries) => {
            visible = entries.some((entry) => entry.isIntersecting)
            /* The GPU context stays until a pane on screen needs it; the rows
               go after a while, and the replay draws them on the way back. */
            if (!visible) {
              disconnect()
              gpu.away()
              clearTimeout(releasing)
              releasing = setTimeout(() => {
                terminal.reset()
                held = []
                heldBytes = 0
                unseen = false
              }, HIDDEN_RELEASE_MS)
              return
            }
            clearTimeout(releasing)
            gpu.show()
            connect()
            refit()
            terminal.refresh(0, terminal.rows - 1)
            if (live) redraw(1)
          })
    shown?.observe(box)
    /* Where nothing can say whether the pane is on screen, it is. */
    if (!shown) {
      visible = true
      connect()
    }

    return () => {
      dropped = true
      clearTimeout(settling)
      clearTimeout(releasing)
      clearTimeout(again)
      setTerm(null)
      onTerminalRef.current?.(null)
      themed.disconnect()
      watch.disconnect()
      shown?.disconnect()
      box.removeEventListener('paste', pasted, true)
      box.removeEventListener('keydown', keys, true)
      box.removeEventListener('contextmenu', menu, true)
      unguard()
      linked.dispose()
      follows.dispose()
      live?.detach()
      batch?.dispose()
      scrolled.dispose()
      delete box.dataset.drawn
      gpu.hide()
      terminal.dispose()
    }
  }, [projectId, paneId, round, target])

  useMarks(term, paneId)
  const closeMenu = useCallback(() => setMenuAt(null), [])
  /* Files dropped on a terminal are typed at its prompt as their paths —
     quoted, and left for the person to run. */
  const dropped = useCallback((paths: readonly string[]) => term?.paste(`${paths.map(shellWord).join(' ')} `), [term])
  const hovering = useFileDrop(host, term ? dropped : null)

  return (
    <>
      {error && <div className="exempty__t">{error}</div>}
      {pasteFailed && <div className="exempty__t">{pasteFailed}</div>}
      <div className="termhost" ref={host} data-drop={hovering ? 'true' : undefined} />
      {menuAt && term && (
        <TerminalMenu at={menuAt} terminal={term} projectId={projectId} paneId={paneId} onClose={closeMenu} onFailed={setPasteFailed} onSplit={onSplit} onClosePane={onClosePane} onBlocks={onBlocks} onSeparate={onSeparate} />
      )}
    </>
  )
}
