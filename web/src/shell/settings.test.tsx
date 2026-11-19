import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { setLocale } from '../i18n'
import { Settings } from './Settings'

/* Settings is the whole window: the panes it draws itself — account,
   projects, storage, appearance, providers — plus the column that lists
   every pane. The panes it merely mounts keep their own tests; the
   commands they ask answer null and they draw their empty faces. */

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('./window', () => ({ onEvent: () => () => undefined, onCarried: () => () => undefined }))

vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: new Proxy({}, { get: () => async () => null }),
}))

/* What Settings and its panes read off the shell: the fields that carry
   words or state here, and a stand-in for whatever else a pane reaches for. */
const shell = new Proxy(
  {
    account: { initials: 'AP', name: 'Aron Peyroteo', email: 'aron@example.com' },
    theme: 'system',
    setTheme: vi.fn(),
    signOut: vi.fn(),
    closePrefs: vi.fn(),
    openPrefs: vi.fn(),
    projects: [] as unknown[],
    project: null,
    reload: vi.fn(),
  },
  { get: (known, field) => (field in known ? known[field as keyof typeof known] : vi.fn()) },
)
vi.mock('./useShell', () => ({ useShell: () => shell }))

afterEach(() => {
  cleanup()
  setLocale('en')
})

const draw = (): void => {
  render(<Settings pane="projects" row={null} onAddProject={vi.fn()} onRemove={vi.fn()} />)
}

describe('the settings window', () => {
  /* The whole window mounts at once — every pane, hidden but drawn — which
     takes longer than the default five seconds on a loaded machine. */
  const SLOW = 30000

  it('speaks English until the person says otherwise', () => {
    draw()
    expect(screen.getByRole('heading', { name: 'Projects' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Add project' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Back' })).toBeTruthy()
  }, SLOW)

  it('follows the person into Portuguese: headings, buttons, column', () => {
    setLocale('pt-BR')
    draw()
    expect(screen.getByRole('heading', { name: 'Projetos' })).toBeTruthy()
    /* Every pane is mounted, hidden until its turn; `hidden` reads them. */
    expect(screen.getByRole('heading', { name: 'Armazenamento', hidden: true })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Adicionar projeto' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Voltar' })).toBeTruthy()
    expect(screen.getByPlaceholderText('Buscar configurações')).toBeTruthy()
    expect(screen.getByText('Recursos de IA')).toBeTruthy()
    expect(screen.getByText(/Remover um projeto o tira do devpit/)).toBeTruthy()
    const theme = screen.getByRole('radiogroup', { name: 'Tema', hidden: true })
    expect(within(theme).getByRole('radio', { name: 'Sistema', hidden: true })).toBeTruthy()
    expect(screen.getByText(/As transcrições são sempre gravadas/)).toBeTruthy()
  }, SLOW)
})
