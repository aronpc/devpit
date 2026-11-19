import type { PaneRunning, Project, Theme as StoredTheme } from '../gen/bindings'
import type { PaneName } from './paneList'
import type { Sweep, Tab } from './strip'
import type { Who } from './account'
import type { Membership } from './useAccount'
import type { PaneSessions } from './paneSessions'
import type { Subagents } from './subagents'
import type { Unread } from './unread'
import type { Doing } from './useAgents'
import type { Closing } from './useClosing'
import type { Panel, Widths } from './sizing'
import type { Renaming } from './useTabs'
import type { Talks } from './useConversations'

/*
 * What the window has, in one shape.
 *
 * Apart from the provider that fills it because the list is long and the
 * filling is short: reading what a surface can reach should not mean scrolling
 * past the wiring that gets it there.
 */

export type Theme = StoredTheme
export type PrefsPane =
  | 'account'
  | 'projects'
  | 'general'
  | 'notifications'
  | 'remote'
  | 'appearance'
  | 'keyboard'
  | 'maintenance'
  | 'diagnostics'
  | 'about'
  | 'archive'
  | 'providers'
  | 'sessions'
  | 'skills'
  | 'storage'
  | 'worktrees'
  | 'hosts'
  | 'usage'
  | 'accounts'
  | 'decisions'
  | 'devflow'
  | 'orchestrator'
  | 'assistant'
  | 'channels'
  | 'voice'

/*
 * Three facts about the panes, because they came apart the moment tabs
 * arrived:
 *
 *   open      the strip's order — first opened is leftmost, and a new one
 *             lands on the right. Clicking an existing tab must not move it,
 *             or the strip reshuffles under the pointer.
 *   active    the pane you are looking at.
 *   previous  what a split would pair with — recency, which is a different
 *             order from the strip and the reason these are two lists.
 */
export interface Shell extends Talks {
  readonly open: readonly Tab[]
  readonly active: Tab | null
  /** Opens a new one of a kind you can have several of; focuses the rest. */
  show: (kind: PaneName, tab?: Partial<Tab>) => void
  close: (id: string) => void
  /** Opens again the tab closed last; false when there is none. */
  reopen: () => boolean
  /** Moves a terminal tab's panes into another's split; answers a refusal or null. */
  join: (from: string, into: string) => Promise<string | null>
  /** Closing several at once, as the tab's menu names it. */
  sweep: (id: string, what: Sweep) => void
  focus: (id: string) => void
  move: (id: string, to: number) => void
  rename: (id: string, title: string) => void
  attach: (id: string, panes: readonly string[]) => void
  launched: (id: string) => void
  drafted: (id: string) => void
  /** Puts another tab in this one's place. */
  replace: (id: string, tab: Tab) => void
  /** What each of the project's panes has in front of it.

      Polled once for the window rather than once per surface: the strip, the
      sidebar and the close prompt all ask the same question, and three timers
      would be three answers that disagree for two seconds at a time. */
  readonly running: readonly PaneRunning[]
  /** What each pane's agent last said it was doing.

      Apart from `running` because the two are different questions with
      different answers: the process table says which agent is open, and only
      the agent says whether it is working or waiting for you. */
  readonly doing: Doing
  /** The panes whose agent finished, and the chats whose turn ended, while their tab was not in front. */
  readonly unread: Unread
  /** A chat's turn ended in this tab. */
  readonly finished: (tabId: string) => void
  /** The subagents each pane's agent started, from its own hooks. */
  readonly subagents: Subagents
  /** The CLI session each pane's agent is in, when its hooks said. */
  readonly agentSessions: PaneSessions
  /** Closes a tab without asking, for a caller that already knows it should. */
  closeNow: (id: string) => void
  /** A pane says whether it is holding an edit that is not on disk, so the
   *  cross in the strip can stop and ask before throwing it away. */
  markUnsaved: (tabId: string, dirty: boolean) => void
  /** How wide the two side panels are. */
  readonly widths: Widths
  /** While a divider is being dragged: paints, does not write. */
  setWidth: (panel: Panel, wide: number) => void
  /** When it is let go: paints and writes. */
  keepWidths: (panel: Panel, wide: number) => void
  readonly renaming: Renaming | null
  setRenaming: (renaming: Renaming | null) => void

  readonly side: boolean
  readonly files: boolean
  toggleSide: () => void
  toggleFiles: () => void

  readonly theme: Theme
  setTheme: (theme: Theme) => void

  readonly project: Project | null
  readonly projects: readonly Project[]
  readonly projectsError: string | null
  setProject: (id: string) => void
  forgetProject: (id: string, wipe?: boolean) => void
  renameProject: (id: string, name: string) => Promise<string | null>
  reloadProjects: () => void

  readonly signedIn: boolean
  readonly account: Who
  /** The whole sign-in machine, for the sheet that drives it. The two fields
   *  above are what every other pane needs, and they stay. */
  readonly membership: Membership
  signIn: () => void
  signOut: () => void

  readonly prefs: PrefsPane | null
  /** The row Settings scrolls to, when it was opened for one. */
  readonly prefsRow: string | null
  openPrefs: (pane?: PrefsPane, row?: string) => void
  /* The Manager shows every project's board, which puts it above any one of
     them — so it takes the window like Settings rather than sitting in a
     project's pane area. */
  readonly managing: boolean
  openManager: () => void
  closeManager: () => void
  closePrefs: () => void

  /** The card a notification asked for, and the board's job to clear.

      On the shell because two surfaces have to agree about it and neither
      contains the other: the bell is in the top bar, and the card opens over
      the board. Cleared by whoever opened it, so a second click on the same
      notification opens it again. */
  readonly wantedCard: string | null
  openCard: (cardId: string | null) => void
  /** Brings the tab holding this terminal pane to the front, once the
   *  project it belongs to has its tabs open. */
  openPane: (paneId: string | null) => void

  /** Whether the sidebar has asked for the runs, and the board's job to clear.

      Here for the same reason as `wantedCard`: the row is in the sidebar and
      the list opens over the board, and neither contains the other. The list
      itself stays where it was — this asks for it, it does not hold a second
      copy of it. */

  /** The one field over the window. Here rather than in the window's own
      state because three things open it — the sidebar's Search, ⌘K, and the
      plus at the end of the strip — and two of them are not the window. */
  readonly palette: boolean
  openPalette: () => void
  closePalette: () => void

  /** The close that stopped to ask, because something is running in it.

      Closing a terminal kills the window behind it, and an agent halfway
      through a task is not something to lose to a stray click on a cross. */
  readonly closing: Closing | null
  confirmClose: (dontAskAgain: boolean) => void
  cancelClose: () => void
}
