import { cleanup, render, screen } from '@testing-library/react'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { t } from '../i18n'
import { Onboarding } from './Onboarding'

afterEach(cleanup)

vi.mock('./useShell', () => ({
  useShell: () => ({
    signedIn: false,
    signIn: vi.fn(),
    theme: 'dark',
    setTheme: vi.fn(),
    /* Nobody is halfway through signing in on a first run, which is the
       state this screen offers from. */
    membership: { signingIn: null, failed: null, origin: 'https://devpit.app', cancelSignIn: vi.fn() },
  }),
}))

/*
 * What the app tells somebody it does, pinned.
 *
 * The first screen said an account "saves the workspace around your work —
 * your board, the capabilities you turn on, your theme", before any of it
 * was built. Sync now carries preferences and project settings, and only
 * once it is turned on; the board stays on each computer. Copy is not a
 * detail here — somebody who believed it would have no backup of the board
 * they think is saved.
 */
describe('what the first run promises', () => {
  it('offers the account without promising a board that is saved', () => {
    render(<Onboarding onAddProject={vi.fn()} onDone={vi.fn()} />)
    const said = screen.getByText(/Free, and optional/).textContent ?? ''
    expect(said).toMatch(/off until you turn it on/)
    expect(said).toMatch(/only preferences and project settings/)
    expect(said).not.toMatch(/saves/)
    expect(said).not.toMatch(/board|capabilit|theme/i)
  })
})

describe('what the settings screen promises', () => {
  const settings = readFileSync(resolve(process.cwd(), 'src/shell/Settings.tsx'), 'utf8')

  it('says what stays on this computer, and what sync carries and keeps back', () => {
    /* The sentence moved to the catalog with the window's other words; the
       promise is still pinned, where it is said now. */
    const words = readFileSync(resolve(process.cwd(), 'src/i18n/en/settings.json'), 'utf8')
    expect(words).toMatch(/Projects, boards, conversations, terminal history and files stay on this computer/)
    const sync = t('workspace.sync.said')
    expect(sync).toMatch(/Preferences and project settings follow you/)
    expect(sync).toMatch(/a key devpit\.app never sees/)
    expect(sync).toMatch(/vault/)
  })

  /* The sidebar's sign-in button is read far more often than the account
     pane, and it said the account saves the workspace setup. */
  it('does not promise, on the sign-in button, a sync that is not turned on', () => {
    const sidebar = readFileSync(resolve(process.cwd(), 'src/shell/Sidebar.tsx'), 'utf8')
    expect(sidebar).not.toMatch(/Save your workspace/i)
    expect(t('workspace.sidebar.signInSaid')).toMatch(/nothing syncs unless you turn it on/)
  })

  /* The sign-in dialog is the last thing read before the account exists, and
     it said the account saves the workspace around the projects. */
  it('does not promise, in the sign-in dialog, a workspace that is saved', () => {
    const dialog = t('workspace.signIn.free')
    expect(dialog).not.toMatch(/saves the workspace/i)
    expect(dialog).toMatch(/boards stay where they are/)
  })

  /* A switch for something nothing reads is worse than no switch: it is a
     choice the person makes and the app ignores. */
  it('has no switch for anything that does not happen', () => {
    expect(settings).not.toMatch(/telemetry/i)
    expect(settings).not.toMatch(/keepTranscripts/)
    expect(settings).not.toMatch(/anonymous usage/i)
  })
})
