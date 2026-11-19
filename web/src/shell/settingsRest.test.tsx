import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { setLocale } from '../i18n'
import { tRich } from '../i18n/rich'
import { ProfileEditor } from './ProfileEditor'
import { ProfilesWaiting } from './ProfilesWaiting'
import { ProviderChecks } from './ProviderChecks'
import { rotationSaid } from './secretRotation'
import { terminalsAfter, waitingFor } from './UpdateCard'
import { UpdateSettings } from './UpdateSettings'
import { ago, dollars, resetIn, tokens } from './usageFormat'
import { blank } from './profiles'

/* The rest of Settings — profiles, providers, updates, usage, secrets —
   speaks the person's language: the words from the catalog, the numbers and
   dates from the locale. The English wording is held by each screen's own
   test; this holds that Portuguese follows. */

vi.mock('./window', () => ({ onEvent: () => () => undefined, onCarried: () => () => undefined }))
vi.mock('./useShell', () => ({ useShell: () => ({ closePrefs: vi.fn() }) }))
vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: {
    appInfo: async () => ({ version: '0.1.30' }),
    updateStatus: async () => ({ type: 'idle' }),
    providersCheckRead: async () => ({ minutes: 5 }),
    agentProfilesWaiting: async () => ({
      profiles: [{ id: 'p1', label: 'GLM', arrived: false, needs: ['ANTHROPIC_AUTH_TOKEN'], changed: [] }],
    }),
  },
}))

afterEach(() => {
  cleanup()
  setLocale('en')
})

describe('the rest of Settings, in Portuguese', () => {
  it('puts a piece of the screen inside a sentence, wherever the language wants it', () => {
    setLocale('pt-BR')
    const out = tRich('waiting.changed', { label: 'GLM', value: 'x' }, { name: 'BASE' })
    expect(out.length).toBeGreaterThan(2)
  })

  it('says its helpers in Portuguese', () => {
    setLocale('pt-BR')
    expect(waitingFor(0, 125)).toBe('há 2 min')
    expect(terminalsAfter('nsis')).toMatch(/terminais/)
    const now = 1_000_000_000_000
    expect(resetIn(now / 1000 + 2 * 3600 + 14 * 60, now)).toBe('reinicia em 2h 14m')
    expect(ago(now / 1000 - 90 * 60, now)).toBe('há 1h')
    expect(tokens(12_400)).toBe('12,4K')
    expect(dollars(0.004)).toMatch(/^<.*0,01/)
    expect(rotationSaid({ name: 'A', setAt: 0, rotateDays: null, rotateAt: 200, reminderCard: null }, 100)).toMatch(/^trocar até /)
  })

  it('draws the profile editor, the waiting note, the check row and the update row', async () => {
    setLocale('pt-BR')
    render(<ProfileEditor draft={blank('claude')} agents={[]} onChange={vi.fn()} onSave={vi.fn()} onCancel={vi.fn()} />)
    expect(screen.getByLabelText('Nome')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Salvar' })).toBeTruthy()
    cleanup()
    render(<ProfilesWaiting />)
    expect(await screen.findByText(/precisa de ANTHROPIC_AUTH_TOKEN aqui/)).toBeTruthy()
    cleanup()
    render(<ProviderChecks />)
    expect(await screen.findByLabelText('Verificar logins e servidores MCP a cada')).toBeTruthy()
    expect(screen.getByRole('option', { name: 'A cada 5 minutos' })).toBeTruthy()
    cleanup()
    render(<UpdateSettings />)
    expect(await screen.findByText('O devpit procura uma versão mais nova enquanto está aberto.')).toBeTruthy()
    await act(async () => undefined)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Verificar agora' })).toBeTruthy())
  })
})
