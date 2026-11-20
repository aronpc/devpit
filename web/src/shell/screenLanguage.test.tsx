import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Change } from '../gen/bindings'
import { setLocale } from '../i18n'
import { ChangeRows } from './ChangeRows'
import { CopyButton } from './CopyButton'
import { CopySession } from './CopySession'
import { FieldSelect } from './FieldSelect'
import { Grip } from './Grip'

afterEach(() => {
  cleanup()
  setLocale('en')
})

/* The screens the fixed text used to be hardcoded into, read in the language
   the person chose: the words come from the catalog, not from the component. */

describe('the shell in Portuguese', () => {
  it('says what the changes panel does to a file', () => {
    setLocale('pt-BR')
    const change: Change = { path: 'a/one.rs', status: 'modified', added: 1, removed: 0, staged: false }
    render(
      <ChangeRows
        changes={[change, { ...change, path: 'b/two.rs' }]}
        staged={false}
        busy={false}
        onOpen={vi.fn()}
        onStage={vi.fn()}
        onDiscard={vi.fn()}
      />,
    )
    expect(screen.getByText('Recolher tudo')).toBeTruthy()
    expect(screen.getByLabelText('Descartar a/one.rs')).toBeTruthy()
    expect(screen.getByLabelText('Preparar a/one.rs')).toBeTruthy()
    expect(screen.getAllByTitle('Colocar no commit')).toHaveLength(2)
    fireEvent.click(screen.getByText('Recolher tudo'))
    expect(screen.getByText('Expandir tudo')).toBeTruthy()
  })

  it('says what the copy buttons are for', () => {
    setLocale('pt-BR')
    render(
      <>
        <CopyButton text="x" className="c" />
        <CopySession id="abc" />
      </>,
    )
    expect(screen.getByLabelText('Copiar')).toBeTruthy()
    expect(screen.getByLabelText('Copiar id da sessão')).toBeTruthy()
  })

  it('says what a field with nothing chosen offers', () => {
    setLocale('pt-BR')
    render(
      <FieldSelect
        label="Conta"
        value={null}
        options={[{ id: 'one', label: 'Uma' }]}
        onPick={vi.fn()}
      />,
    )
    expect(screen.getByText('Escolha um')).toBeTruthy()
  })

  it('names the panel edges', () => {
    setLocale('pt-BR')
    const { container } = render(<Grip panel="sidebar" width={200} other={200} onSize={vi.fn()} onDone={vi.fn()} />)
    expect(container.querySelector('.seam')!.getAttribute('aria-label')).toBe('Redimensionar a barra lateral')
  })
})
