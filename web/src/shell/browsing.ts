import { t } from '../i18n'

/*
 * What a person typed into an address bar, and where it points.
 *
 * Apart from the pane because it is the part that is easy to get subtly wrong
 * and impossible to notice from reading: `localhost:3000` is a url whose
 * scheme is `localhost`, and handing that to a parser gives a scheme nobody
 * asked for rather than the page somebody wanted.
 *
 * **devpit does not search.** A word that is not an address stays refused
 * rather than being sent to somebody's search engine — the pane opens what the
 * project is running, and quietly shipping a query to a third party is not
 * something an address bar should decide on its own.
 */

/** What an address bar was pointed at, or why it was not. */
export type Aimed = { at: string } | { refused: string }

/* The schemes the Rust side will accept. Repeated here on purpose: the window
   should say no before a round trip, and `browser.rs` says no again because a
   check only the window makes is a check a caller can skip. */
const OPENS = ['http:', 'https:']

/** Whether a scheme is one the pane can show. */
const opens = (scheme: string): boolean => OPENS.includes(scheme)

/*
 * A bare `host`, `host:port` or `host/path`, which is what people type.
 *
 * Deliberately narrow: letters, digits, dots and dashes before an optional
 * port and an optional path. `localhost`, `127.0.0.1:3000` and `example.com`
 * pass; a sentence with a space does not, and neither does something with a
 * scheme in it, which took the branch above.
 *
 * An IPv6 literal in brackets is allowed too — `[::1]:8080`. Added because
 * `loopback()` below was already testing for one that this could never
 * produce, which is dead code wearing the costume of coverage.
 */
const HOSTISH =
  /^([a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*|\[[0-9a-fA-F:]+\])(:\d{1,5})?(\/\S*)?$/

/**
 * Where to point the pane, given what somebody typed.
 *
 * Three cases, in this order: something that already carries a scheme, which
 * is honoured or refused on that scheme; something host-shaped, which gets
 * `https://` because that is the safe half of the guess; and anything else,
 * which is refused by name.
 */
export function aim(typed: string): Aimed {
  const text = typed.trim()
  if (!text) return { refused: t('workspace.browserPane.typeAnAddress') }

  /* A scheme is `word:`, and only when what follows is not just digits —
     otherwise `localhost:3000` reads as the scheme `localhost`. */
  const scheme = /^([a-zA-Z][a-zA-Z0-9+.-]*):(?!\d+(?:\/|$))/.exec(text)
  if (scheme) {
    const named = `${scheme[1].toLowerCase()}:`
    if (!opens(named)) {
      return { refused: t('workspace.browserPane.schemeRefused', { scheme: scheme[1]! }) }
    }
    try {
      return { at: new URL(text).toString() }
    } catch {
      return { refused: t('workspace.browserPane.notAnAddress', { text }) }
    }
  }

  if (HOSTISH.test(text)) {
    /* `http` for a machine that is this one, `https` for everything else.
       The pane's own placeholder is `localhost:3000`, and typing it opened
       `https://localhost:3000/`, which essentially no local dev server
       answers — the pane failed at the one thing US-025 says it is for. A
       loopback address is not a place a downgrade attack can reach. */
    const scheme = loopback(text) ? 'http' : 'https'
    try {
      return { at: new URL(`${scheme}://${text}`).toString() }
    } catch {
      return { refused: t('workspace.browserPane.notAnAddress', { text }) }
    }
  }

  return { refused: t('workspace.browserPane.notSearch', { text }) }
}

/** Whether a host is this machine, which is the only place `http` is assumed. */
const loopback = (text: string): boolean => {
  const authority = text.split('/')[0].toLowerCase()
  /* Brackets first: an IPv6 literal is full of the colon that splits a port. */
  if (authority.startsWith('[')) {
    const inside = authority.slice(1, authority.indexOf(']'))
    return inside === '::1' || inside === '0:0:0:0:0:0:0:1'
  }
  const host = authority.split(':')[0]
  return host === 'localhost' || host === '127.0.0.1' || host.endsWith('.localhost')
}

/** What to show in the bar for a url, which is the url and not a decoration. */
export function shown(url: string): string {
  try {
    const parsed = new URL(url)
    /* The trailing slash a parser adds to a bare host is noise nobody typed. */
    return parsed.pathname === '/' && !parsed.search && !parsed.hash
      ? `${parsed.protocol}//${parsed.host}`
      : parsed.toString()
  } catch {
    return url
  }
}

/** Where a pane sits in the window, which is what the native webview needs. */
export type Where = { x: number; y: number; width: number; height: number }

/**
 * The box a pane occupies, in window coordinates.
 *
 * A native webview is placed against the window and knows nothing about the
 * layout that decided where it goes, so the number has to be measured rather
 * than derived. A pane that has not been laid out yet measures zero, and the
 * Rust side clamps that to something a window can hold — a webview of zero
 * height is one nobody can see and nobody can close.
 */
export function boxOf(rect: DOMRect): Where {
  return {
    x: Math.round(rect.left),
    y: Math.round(rect.top),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
  }
}

/** What can open over every pane at once. */
export interface Over {
  readonly managing: boolean
  readonly prefs: unknown
  readonly palette: boolean
}

/**
 * Whether a screen is open over the panes.
 *
 * A native page floats above the whole window, so a screen drawn by this
 * document cannot cover it: the page has to leave, as it does for a tab that
 * is not the visible one.
 */
export function covered(over: Over): boolean {
  return over.managing || over.prefs !== null || over.palette
}

/** Whether two boxes differ enough to be worth telling the window about. */
export function moved(before: Where | null, now: Where): boolean {
  if (!before) return true
  return (
    before.x !== now.x ||
    before.y !== now.y ||
    before.width !== now.width ||
    before.height !== now.height
  )
}

/**
 * The host of a url, for offering a domain somebody probably means.
 *
 * Offered and not assumed: it fills the field, and importing still needs the
 * field to be agreed to. A url that will not parse offers nothing rather than
 * offering a guess.
 */
export function hostOf(url: string): string {
  try {
    return new URL(url).hostname
  } catch {
    return ''
  }
}

/** A browser on this machine, with the profiles of it that can be read. */
export interface Family {
  readonly family: string
  readonly profiles: readonly { readonly path: string; readonly name: string; readonly warning: string }[]
}

/**
 * The stores on this machine, as a menu asks about them: browser first, then
 * which profile of it.
 *
 * One question at a time because that is how somebody looks for their login —
 * *the Chrome one*, then *my work Chrome*. A flat list of
 * `Google Chrome · Profile 1` makes them read the browser name five times to
 * find the profile, and it is the shape that hid the profile entirely on the
 * first version of this menu.
 *
 * Order is kept as it arrives; Rust already sorted it by browser and profile.
 */
export function grouped(stores: readonly { family: string; profile: string; path: string; warning: string }[]): Family[] {
  const families: Family[] = []
  for (const store of stores) {
    const one = { path: store.path, name: store.profile, warning: store.warning }
    const held = families.find((found) => found.family === store.family)
    if (held) (held.profiles as { path: string; name: string; warning: string }[]).push(one)
    else families.push({ family: store.family, profiles: [one] })
  }
  return families
}

/** A width to show a page at, for looking at a layout that is not this one. */
export interface Viewport {
  readonly id: string
  readonly width: number
  readonly height: number
}

/*
 * The sizes a page can be held at.
 *
 * **This is a size and not a device.** A real device toolbar also sends a
 * phone's user agent, reports touch, and sets a device pixel ratio, and all
 * three need an engine hook devpit's webview does not have. What this does is
 * make the page narrow, which is what moves a responsive layout — and the menu
 * says so rather than implying a phone.
 *
 * The numbers are Chrome DevTools', so a layout checked here and a layout
 * checked there break at the same place.
 */
export const VIEWPORTS: readonly Viewport[] = [
  { id: 'mobile-s', width: 320, height: 568 },
  { id: 'mobile-m', width: 375, height: 667 },
  { id: 'mobile-l', width: 425, height: 812 },
  { id: 'tablet', width: 768, height: 1024 },
  { id: 'laptop', width: 1024, height: 768 },
  { id: 'laptop-l', width: 1440, height: 900 },
  { id: 'desktop', width: 1920, height: 1080 },
]

/** What the menu calls a width. */
export const viewportLabel = (one: Viewport): string => t(`workspace.browserMenu.viewport.${one.id}`)

/**
 * The box to give the page inside the hole the pane measured.
 *
 * Without a preset it is the hole. With one, the page is that size, centred,
 * and **never larger than the hole** — a 1920-wide preset in a 700-wide pane
 * would otherwise put most of the page under the sidebar, which is the bug
 * this whole area of the app has been about.
 */
export function inset(hole: Where, viewport: Viewport | null): Where {
  if (!viewport) return hole
  const width = Math.min(viewport.width, hole.width)
  const height = Math.min(viewport.height, hole.height)
  return {
    x: hole.x + Math.round((hole.width - width) / 2),
    y: hole.y,
    width,
    height,
  }
}
