import { t } from '../i18n'

import { ROWS } from './prefsRows'
import type { PrefsPane } from './shape'

/*
 * The settings sidebar, as a list rather than as markup.
 *
 * Nine buttons in a flat column meant the only way to find a setting was to
 * have already known where it was. Grouped, the column answers a question a
 * newcomer can actually ask — "this is about my projects" — before it asks
 * them to know the name of the pane.
 *
 * A list because two things read it: the column that draws it, and the search
 * field that filters it. Written as markup, the second one would have had to
 * re-state the first.
 *
 * A function, so the labels are said in the locale of whoever is reading:
 * called from a render, never at import time.
 */

export interface NavItem {
  readonly id: PrefsPane
  readonly label: string
  /** What else this pane is about, for someone searching the word they know
   *  rather than the word on the button. Its rows' titles are searched too.
   *  Kept in English: an index for the field, not words the screen says. */
  readonly about: string
  readonly icon: React.JSX.Element
}

export interface NavGroup {
  readonly id: string
  readonly title: string
  readonly items: readonly NavItem[]
}

const stroke = {
  width: 15,
  height: 15,
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.9,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
} as const

export function prefsNav(): readonly NavGroup[] {
  return [
    {
      id: 'application',
      title: t('prefs.group.application'),
      items: [
        {
          id: 'general',
          label: t('prefs.nav.general'),
          about: 'updates version privacy local automatic error reports login startup tray shortcut open apps display wayland x11',
          icon: (
            <svg {...stroke}><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-2.82 1.18V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 7.26 19.4l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 3.09 13H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 7.26l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 10 3.09V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 2.74 1.51l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 20.91 11H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z" /></svg>
          ),
        },
        {
          id: 'notifications',
          label: t('prefs.nav.notifications'),
          about: 'island pause quiet meeting reminders focus banner notify',
          icon: (
            <svg {...stroke}><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" /><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" /></svg>
          ),
        },
        {
          id: 'remote',
          label: t('prefs.nav.remote'),
          about: 'phone tablet tailscale relay pair devices mcp calendar ics google subscribe',
          icon: (
            <svg {...stroke}><rect x="7" y="2" width="10" height="20" rx="2" /><path d="M11 18h2" /></svg>
          ),
        },
        {
          id: 'appearance',
          label: t('prefs.nav.appearance'),
          about: 'theme dark light system colours terminal text font size code contrast latex maths math formula diffs side by side stacked whitespace colour blind folded collapsed',
          icon: (
            <svg {...stroke}><circle cx="12" cy="12" r="9" /><path d="M12 3v18" /></svg>
          ),
        },
        {
          id: 'keyboard',
          label: t('prefs.nav.keyboard'),
          about: 'shortcuts keys keybindings hotkeys record reset cheat sheet ctrl cmd',
          icon: (
            <svg {...stroke}><rect x="2" y="5" width="20" height="14" rx="2" /><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M18 13h.01M9 15h6" /></svg>
          ),
        },
        {
          id: 'maintenance',
          label: t('prefs.nav.maintenance'),
          about: 'power saver battery background probes logs artifacts delete clean disk labs beta project agent canvas',
          icon: (
            <svg {...stroke}><path d="M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.5 2.5-2.4-.6-.6-2.4z" /></svg>
          ),
        },
        {
          id: 'diagnostics',
          label: t('prefs.nav.diagnostics'),
          about: 'processes cpu memory load logs errors checks health doctor',
          icon: (
            <svg {...stroke}><path d="M3 12h4l3-8 4 16 3-8h4" /></svg>
          ),
        },
        {
          id: 'about',
          label: t('prefs.nav.about'),
          about: 'version privacy policy licence license open source notices',
          icon: (
            <svg {...stroke}><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 8h.01" /></svg>
          ),
        },
      ],
    },
    {
      id: 'workspace',
      title: t('prefs.group.workspace'),
      items: [
        {
          id: 'projects',
          label: t('prefs.nav.projects'),
          about: 'repositories folders clone remove rename remote origin',
          icon: (
            <svg {...stroke}><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></svg>
          ),
        },
        {
          id: 'worktrees',
          label: t('prefs.nav.worktrees'),
          about: 'checkouts branches disk cards',
          icon: (
            <svg {...stroke}><line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" /></svg>
          ),
        },
        {
          id: 'hosts',
          label: t('prefs.nav.hosts'),
          about: 'ssh remote machines servers staging production',
          icon: (
            <svg {...stroke}><rect x="3" y="4" width="18" height="7" rx="1" /><rect x="3" y="13" width="18" height="7" rx="1" /><path d="M7 7.5h.01M7 16.5h.01" /></svg>
          ),
        },
        {
          id: 'storage',
          label: t('prefs.nav.storage'),
          about: 'disk transcripts workspace directory size',
          icon: (
            <svg {...stroke}><ellipse cx="12" cy="6" rx="8" ry="3" /><path d="M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6" /><path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" /></svg>
          ),
        },
        {
          id: 'archive',
          label: t('prefs.nav.archive'),
          about: 'archived conversations chats restore put away hidden',
          icon: (
            <svg {...stroke}><rect x="3" y="4" width="18" height="4" rx="1" /><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8M10 12h4" /></svg>
          ),
        },
      ],
    },
    {
      id: 'capabilities',
      title: t('prefs.group.capabilities'),
      items: [
        {
          id: 'providers',
          label: t('prefs.nav.providers'),
          about: 'agents claude codex gemini models cli',
          icon: (
            <svg {...stroke}><rect x="3" y="3" width="18" height="18" rx="3" /><path d="m8 10 3 3-3 3M14 16h3" /></svg>
          ),
        },
        {
          id: 'sessions',
          label: t('prefs.nav.sessions'),
          about: 'terminal stop close relay next card epic handoff context chat activity zen',
          icon: (
            <svg {...stroke}><path d="M3 12a9 9 0 1 0 3-6.7L3 8" /><path d="M3 3v5h5" /><path d="M12 7v5l4 2" /></svg>
          ),
        },
        {
          id: 'skills',
          label: t('prefs.nav.skills'),
          about: 'capabilities prompts omc installed',
          icon: (
            <svg {...stroke}><path d="M12 3v3M5.6 5.6l2.1 2.1M3 12h3M18 12h3M16.3 7.7l2.1-2.1" /><rect x="7" y="12" width="10" height="9" rx="2" /></svg>
          ),
        },
        {
          id: 'channels',
          label: t('prefs.nav.channels'),
          about: 'telegram bot notify notification phone away quiet hours silent message push email escalate escalation next channel',
          icon: (
            <svg {...stroke}><path d="m22 2-7 20-4-9-9-4Z" /><path d="M22 2 11 13" /></svg>
          ),
        },
        {
          id: 'voice',
          label: t('prefs.nav.voice'),
          about: 'voice notes microphone whisper transcribe speech audio',
          icon: (
            <svg {...stroke}><rect x="9" y="2" width="6" height="12" rx="3" /><path d="M19 10v2a7 7 0 0 1-14 0v-2" /><path d="M12 19v3" /></svg>
          ),
        },
        {
          id: 'devflow',
          label: t('prefs.nav.devflow'),
          about: 'flow briefing plan review gate stages reviewers codex plugin setup preset recommended custom',
          icon: (
            <svg {...stroke}><circle cx="5" cy="6" r="2" /><circle cx="12" cy="18" r="2" /><circle cx="19" cy="6" r="2" /><path d="M5 8v2a4 4 0 0 0 4 4h1M19 8v2a4 4 0 0 1-4 4h-1" /></svg>
          ),
        },
        {
          id: 'orchestrator',
          label: t('prefs.nav.orchestrator'),
          about: 'guardrails confirm ask stop force draft send full access permission',
          icon: (
            <svg {...stroke}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></svg>
          ),
        },
        {
          id: 'assistant',
          label: t('prefs.nav.assistant'),
          about: 'assistant mode heartbeat schedules routines proactive check in ci blocked waiting briefing away',
          icon: (
            <svg {...stroke}><circle cx="12" cy="13" r="8" /><path d="M12 9v4l2 2M5 3 2 6M22 6l-3-3" /></svg>
          ),
        },
        {
          id: 'decisions',
          label: t('prefs.nav.decisions'),
          about: 'jev typesafe openrouter vercel gateway rubric gate judge classify verdict probability key cap',
          icon: (
            <svg {...stroke}><path d="M12 3v18M5 7h14" /><path d="m5 7-3 6a3 3 0 0 0 6 0Z" /><path d="m19 7-3 6a3 3 0 0 0 6 0Z" /></svg>
          ),
        },
      ],
    },
    {
      id: 'account',
      title: t('prefs.group.account'),
      items: [
        {
          id: 'account',
          label: t('prefs.nav.account'),
          about: 'sign in out sync email profile',
          icon: (
            <svg {...stroke}><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" /><circle cx="12" cy="7" r="4" /></svg>
          ),
        },
        {
          id: 'accounts',
          label: t('prefs.nav.accounts'),
          about: 'claude config directory quota limit switch five hour weekly remote control',
          icon: (
            <svg {...stroke}><circle cx="9" cy="8" r="3" /><path d="M3 20v-1a5 5 0 0 1 10 0v1" /><circle cx="17" cy="9" r="2.5" /><path d="M15 20v-1a4 4 0 0 1 6-3.5" /></svg>
          ),
        },
        {
          id: 'usage',
          label: t('prefs.nav.usage'),
          about: 'spend cost tokens budget limits',
          icon: (
            <svg {...stroke}><path d="M4 20V10M10 20V4M16 20v-7M22 20H2" /></svg>
          ),
        },
      ],
    },
  ]
}

/*
 * The groups that still have something in them, for a given query.
 *
 * Empty query is the whole list untouched rather than a filter that happens to
 * match everything: a heading that disappears while you delete the last letter
 * of your search is the column moving under the pointer.
 */
export function matching(query: string, nav: readonly NavGroup[] = prefsNav()): readonly NavGroup[] {
  const wanted = query.trim().toLowerCase()
  if (!wanted) return nav

  return nav
    .map((group) => ({
      ...group,
      items: group.items.filter(
        (item) =>
          item.label.toLowerCase().includes(wanted) ||
          item.about.includes(wanted) ||
          (ROWS[item.id] ?? []).some((title) => title.toLowerCase().includes(wanted)) ||
          group.title.toLowerCase().includes(wanted),
      ),
    }))
    .filter((group) => group.items.length > 0)
}
