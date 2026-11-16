import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import type { AgentHealth, Happening, Question } from '../gen/bindings'
import { inBrowser } from '../transport'

/*
 * The window, for a window that draws its own frame.
 *
 * `decorations: false` buys the titlebar back as ours to design, and hands
 * back the bill: moving, resizing, minimising, maximising and closing all stop
 * being the system's job. Every one of them is wired here.
 *
 * Outside Tauri — `vite preview`, the tests — these are no-ops rather than
 * throws, so the same shell renders in a plain browser.
 */
export const inTauri = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/* The desktop's own window: not there for a page in a browser, whose frame is
   the browser's. */
const inDesktop = (): boolean => inTauri() && !inBrowser()

const onWindow = async (act: (win: ReturnType<typeof getCurrentWindow>) => Promise<unknown>): Promise<void> => {
  if (!inDesktop()) return
  try {
    await act(getCurrentWindow())
  } catch {
    /* The window went away mid-gesture; there is nothing to report. */
  }
}

export const minimize = (): void => void onWindow((win) => win.minimize())
export const toggleMaximize = (): void => void onWindow((win) => win.toggleMaximize())
export const close = (): void => void onWindow((win) => win.close())

/** The eight directions a frameless window can be pulled by. */
export const EDGES = [
  'North',
  'South',
  'East',
  'West',
  'NorthEast',
  'NorthWest',
  'SouthEast',
  'SouthWest',
] as const

export type Edge = (typeof EDGES)[number]

/**
 * Hands the drag to the window manager.
 *
 * It has to start on pointerdown, not on click: the compositor takes over the
 * pointer for the rest of the gesture, and a click never arrives.
 */
export const startResize = (edge: Edge): void =>
  void onWindow((win) => win.startResizeDragging(edge))

/** True while the window fills the screen, so the corners can square off. */
export async function isMaximized(): Promise<boolean> {
  if (!inDesktop()) return false
  try {
    return await getCurrentWindow().isMaximized()
  } catch {
    return false
  }
}

/** Calls back whenever the window is resized, so the corners keep up. */
export function onResized(then: () => void): () => void {
  if (!inDesktop()) return () => {}
  /* Registering is asynchronous, so the caller can be gone before the
     listener exists. Dropping it then has to be remembered, or the listener
     arrives with nobody left to unsubscribe it. */
  let dropped = false
  let drop: (() => void) | undefined
  void getCurrentWindow()
    .onResized(() => then())
    .then((unlisten) => {
      if (dropped) unlisten()
      else drop = unlisten
    })
  return () => {
    dropped = true
    drop?.()
  }
}

/**
 * Calls back with the paths of files dropped on the window.
 *
 * The webview's own drop event carries a `File` with no path, which is no use
 * to an agent that has to open it. Tauri's event carries the real paths.
 */
/* Whether files are being dragged over the window, from the webview's own
   drag events. A function over the event type so the rule is testable: a
   drop and a leave both end the hover. */
export function draggingAfter(type: string): boolean {
  return type === 'enter' || type === 'over'
}



/**
 * Calls back when an agent asks to be allowed to do something.
 *
 * Every conversation hears every question; matching it to the session that
 * raised it is the caller's job, because only the caller knows which session
 * it is showing.
 */
export function onPermissionAsked(then: (question: Question) => void): () => void {
  if (!inTauri()) return () => {}
  let dropped = false
  let drop: (() => void) | undefined
  void listen<Question>('permission:asked', (event) => then(event.payload)).then((unlisten) => {
    if (dropped) unlisten()
    else drop = unlisten
  })
  return () => {
    dropped = true
    drop?.()
  }
}

/**
 * Calls back when a question stops waiting — answered in any window, or timed
 * out — with its id, so every window drawing it can put it away.
 */
export function onPermissionSettled(then: (id: string) => void): () => void {
  return shared<string>('permission:settled', then)
}

/**
 * Calls back whenever a pane says something about itself.
 *
 * One event for all of it — a prompt, a command starting, an exit code, a
 * title, a working directory — because they are all "something happened in
 * this pane", and five subscriptions to one question would leave the screen
 * ordering them itself. The payload names the pane, so a listener that cares
 * about one filters.
 *
 * A pane reports whether or not anyone is attached to it: see
 * `apps/desktop/src/tap.rs`.
 */
export function onHappening(then: (happening: Happening) => void): () => void {
  return shared<Happening>('terminal:happening', then)
}

/*
 * One Tauri listener per event name, however many parts of the window listen.
 *
 * Every `listen` is its own callback across the IPC boundary, and the payload
 * is parsed once for each. A pane used to open two or three of them on the
 * same event, so with ten panes a terminal's every prompt crossed thirty
 * times. Here it crosses once and is handed to each subscriber; the listener
 * goes away with the last of them.
 */
const listeners = new Map<
  string,
  { subscribers: Set<(payload: unknown) => void>; drop?: () => void; dropped?: boolean; heard: Promise<void> }
>()

function shared<T>(name: string, then: (payload: T) => void): () => void {
  if (!inTauri()) return () => {}
  let entry = listeners.get(name)
  if (!entry) {
    /* The promise says the listener is in place — or never will be: it
       resolves on a failed registration too, because a waiter must not hang
       on a listener that does not exist. */
    let settled: () => void
    const made: {
      subscribers: Set<(payload: unknown) => void>
      drop?: () => void
      dropped?: boolean
      heard: Promise<void>
    } = {
      subscribers: new Set(),
      heard: new Promise<void>((resolve) => {
        settled = resolve
      }),
    }
    entry = made
    listeners.set(name, made)
    void listen<unknown>(name, (event) => {
      /* Each on its own: one subscriber throwing does not keep the event from
         the rest, and one that left during the dispatch is not called. */
      for (const one of [...made.subscribers]) {
        if (!made.subscribers.has(one)) continue
        try {
          one(event.payload)
        } catch (thrown) {
          reportError(thrown)
        }
      }
    }).then(
      (unlisten) => {
        if (made.dropped) unlisten()
        else made.drop = unlisten
        settled()
      },
      /* Never heard: the next subscriber tries again. */
      () => {
        if (listeners.get(name) === made) listeners.delete(name)
        settled()
      },
    )
  }
  /* Its own function, so the same callback subscribed twice is two entries. */
  const mine = (payload: unknown): void => then(payload as T)
  entry.subscribers.add(mine)
  const held = entry
  return () => {
    held.subscribers.delete(mine)
    if (held.subscribers.size === 0 && listeners.get(name) === held) {
      listeners.delete(name)
      held.dropped = true
      held.drop?.()
    }
  }
}

/*
 * Any event the backend emits, with no payload to read.
 *
 * `onHappening` is the shape for one that carries something; this is the
 * shape for one that only says "look again". The bell is the first of those:
 * the count and the list come from a command, and the event exists only to
 * say when to ask for them.
 */
export function onEvent(name: string, then: () => void): () => void {
  if (!inTauri()) return () => {}
  let dropped = false
  let drop: (() => void) | undefined
  void listen(name, () => then()).then((unlisten) => {
    if (dropped) unlisten()
    else drop = unlisten
  })
  return () => {
    dropped = true
    drop?.()
  }
}

/*
 * An event that names the thing it happened to.
 *
 * `onEvent` is for one that only says "look again"; this is for one that says
 * which card, or which run. The payload is a plain value rather than a shape,
 * because these two carry exactly that — `run:changed` is a card id, and
 * `run:progress` is a pair.
 */
export function onCarried<T>(name: string, then: (payload: T) => void): () => void {
  return shared<T>(name, then)
}

/*
 * Resolves once the shared listener for `name` is in place — and when it never
 * will be: the registration failed, nobody subscribed, or there is no Tauri
 * to hear from. A caller that must not miss what the event carries waits for
 * this before asking for whatever came before it.
 */
export function whenHeard(name: string): Promise<void> {
  if (!inTauri()) return Promise.resolve()
  return listeners.get(name)?.heard ?? Promise.resolve()
}

/**
 * Calls back when a conversation whose process stays is woken by another
 * session and starts a turn nobody here asked for — with its id, so an open
 * chat can join it.
 */
/** A terminal tab opened or closed from outside the window — an orchestrator
 *  starting or stopping a session. */
export interface OutsideTab {
  readonly projectId: string
  readonly tabId: string
  readonly paneId?: string
}

export function onOutsideTabs(opened: (tab: OutsideTab) => void, closed: (tab: OutsideTab) => void): () => void {
  if (!inTauri()) return () => {}
  let dropped = false
  const drops: (() => void)[] = []
  for (const [name, then] of [
    ['session:tab-opened', opened],
    ['session:tab-closed', closed],
  ] as const) {
    void listen<OutsideTab>(name, (event) => then(event.payload)).then((unlisten) => {
      if (dropped) unlisten()
      else drops.push(unlisten)
    })
  }
  return () => {
    dropped = true
    for (const drop of drops) drop()
  }
}

/** Said when a conversation reachable by Remote Control is given its page. */
export function onRemoteConnected(then: (conversationId: string) => void): () => void {
  if (!inTauri()) return () => {}
  let dropped = false
  let drop: (() => void) | undefined
  void listen<string>('chat:remote', (event) => then(event.payload)).then((unlisten) => {
    if (dropped) unlisten()
    else drop = unlisten
  })
  return () => {
    dropped = true
    drop?.()
  }
}

export function onChatWoke(then: (conversationId: string) => void): () => void {
  if (!inTauri()) return () => {}
  let dropped = false
  let drop: (() => void) | undefined
  void listen<string>('chat:woke', (event) => then(event.payload)).then((unlisten) => {
    if (dropped) unlisten()
    else drop = unlisten
  })
  return () => {
    dropped = true
    drop?.()
  }
}

/** Calls back with every check of devpit's own MCP, and every restart of it. */
export function onAgentHealth(then: (health: AgentHealth) => void): () => void {
  if (!inTauri()) return () => {}
  let dropped = false
  let drop: (() => void) | undefined
  void listen<AgentHealth>('agent:health', (event) => then(event.payload)).then((unlisten) => {
    if (dropped) unlisten()
    else drop = unlisten
  })
  return () => {
    dropped = true
    drop?.()
  }
}
