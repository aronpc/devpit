import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react'

import type { FolderGlance } from '../gen/bindings'
import { searched, suggestion } from './commandHistory'
import { common, finished, wordAt } from './completing'
import { shortPath } from './blockText'
import { Folder } from './GitIcons'
import { ask, commands } from './live'
import { keysLabel, matches } from './shortcuts'
import { abandoned, committed } from './typing'
import { t } from '../i18n'

/*
 * Where the next line is typed, below the blocks.
 *
 * An editor rather than the shell's own line: several lines are text, not
 * continuation prompts; the newest line in history that starts with what you
 * typed is shown ahead of the cursor, and Tab or → takes it; ↑ and ↓ walk the
 * history from the first and last line; Ctrl-R searches it. What it sends,
 * the shell runs exactly as if it had been typed there (`pane.submit`).
 *
 * The chips above it are where you are: the folder the shell last named, and
 * that folder's branch and how far it is from HEAD.
 */

export interface CommandInputHandle {
  focus: () => void
  /** Puts a line in the editor to be changed before it runs. */
  set: (line: string) => void
}

export const CommandInput = forwardRef<CommandInputHandle, {
  cwd: string | null
  home: string | null
  history: readonly string[]
  /** Something the glance should be taken again after: the last block's end. */
  settled: unknown
  onSubmit: (line: string) => void
  /** Ctrl+L: the finished blocks go. */
  onClear: () => void
  onClassic: () => void
}>(function CommandInput({ cwd, home, history, settled, onSubmit, onClear, onClassic }, handle) {
  const [text, setText] = useState('')
  const [walk, setWalk] = useState<number | null>(null)
  const [search, setSearch] = useState<string | null>(null)
  const [pick, setPick] = useState(0)
  const [glance, setGlance] = useState<FolderGlance | null>(null)
  /* Tab's choices when it could not choose alone, and which one is lit. */
  const [choices, setChoices] = useState<{ start: number; caret: number; found: readonly string[]; at: number } | null>(null)
  const field = useRef<HTMLTextAreaElement>(null)

  useImperativeHandle(handle, () => ({
    focus: () => field.current?.focus(),
    set: (line) => {
      setText(line)
      setWalk(null)
      requestAnimationFrame(() => {
        field.current?.focus()
        field.current?.setSelectionRange(line.length, line.length)
      })
    },
  }))

  useEffect(() => {
    if (!cwd) return setGlance(null)
    let live = true
    void ask(() => commands.folderGlance(cwd)).then((answer) => live && setGlance(answer.data ?? null))
    return () => {
      live = false
    }
  }, [cwd, settled])

  const ghost = walk === null ? suggestion(history, text) : null
  const found = search === null ? [] : searched(history, search)
  const rows = Math.min(8, text.split('\n').length)

  const submit = (line: string): void => {
    if (!line.trim()) return
    onSubmit(line)
    setText('')
    setWalk(null)
  }

  const step = (by: 1 | -1): void => {
    const next = walk === null ? (by === 1 ? 0 : null) : walk + by
    if (next === null || next < 0) {
      setWalk(null)
      setText('')
      return
    }
    if (next >= history.length) return
    setWalk(next)
    setText(history[next]!)
  }

  const put = (start: number, caret: number, word: string): void => {
    const next = finished(text, start, caret, word)
    setText(next.text)
    setWalk(null)
    requestAnimationFrame(() => field.current?.setSelectionRange(next.caret, next.caret))
  }

  /* Tab with nothing to accept: the word under the cursor, finished from the
     folder it names — alone when there is one answer, as far as they agree
     when there are several, and the rest offered to choose from. */
  const complete = (box: HTMLTextAreaElement): void => {
    if (!cwd) return
    const caret = box.selectionStart
    const { start, word } = wordAt(text, caret)
    void ask(() => commands.folderComplete(cwd, word)).then((answer) => {
      const found = answer.data ?? []
      if (found.length === 0) return
      if (found.length === 1) return put(start, caret, found[0]!)
      const shared = common(found)
      if (shared.length > word.length) put(start, caret, shared)
      setChoices({ start, caret: start + Math.max(shared.length, word.length), found, at: 0 })
    })
  }

  const onKey = (event: React.KeyboardEvent<HTMLTextAreaElement>): void => {
    const box = event.currentTarget
    if (choices) {
      if (abandoned(event)) {
        event.preventDefault()
        return setChoices(null)
      }
      if (event.key === 'Tab' || event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        const by = event.key === 'ArrowUp' || (event.key === 'Tab' && event.shiftKey) ? -1 : 1
        return setChoices({ ...choices, at: (choices.at + by + choices.found.length) % choices.found.length })
      }
      if (committed(event)) {
        event.preventDefault()
        put(choices.start, choices.caret, choices.found[choices.at]!)
        return setChoices(null)
      }
      setChoices(null)
    }
    const atStart = box.selectionStart === 0 || !text.slice(0, box.selectionStart).includes('\n')
    const atEnd = !text.slice(box.selectionEnd).includes('\n')
    if (matches('clearBlocks', event)) {
      event.preventDefault()
      onClear()
      return
    }
    if (matches('searchHistory', event)) {
      event.preventDefault()
      setSearch('')
      setPick(0)
      return
    }
    if (matches('clearLine', event) && !box.value.slice(box.selectionStart, box.selectionEnd)) {
      event.preventDefault()
      setText('')
      setWalk(null)
      return
    }
    if (committed(event) && !event.shiftKey) {
      event.preventDefault()
      return submit(text)
    }
    if (ghost && (event.key === 'Tab' || (event.key === 'ArrowRight' && box.selectionStart === text.length))) {
      event.preventDefault()
      setText(text + ghost)
      return
    }
    if (event.key === 'Tab' && !event.shiftKey) {
      event.preventDefault()
      complete(box)
      return
    }
    if (event.key === 'ArrowUp' && atStart && !event.shiftKey) {
      event.preventDefault()
      step(1)
      return
    }
    if (event.key === 'ArrowDown' && atEnd && !event.shiftKey && walk !== null) {
      event.preventDefault()
      step(-1)
    }
  }

  const folder = shortPath(cwd, home)
  return (
    <div className="cin">
      <div className="cin__chips">
        {folder && (
          <span className="cin__chip" title={cwd ?? undefined}>
            <Folder size={12} />
            {folder}
          </span>
        )}
        {glance && (
          <span className="cin__chip cin__chip--git" title={t('sessions.cmd.branchTitle')}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="6" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="8" r="3" /><path d="M6 9v6M18 11a6 6 0 0 1-6 6H9" /></svg>
            {glance.branch}
            {glance.files > 0 && (
              <span className="cin__diff">
                {glance.files} · <span className="cin__add">+{glance.added}</span> <span className="cin__del">−{glance.removed}</span>
              </span>
            )}
          </span>
        )}
        <button className="cin__classic" title={t('sessions.cmd.classicTitle')} onClick={onClassic}>
          {t('sessions.cmd.classic')}
        </button>
      </div>

      {search !== null && (
        <div className="cin__search" role="listbox" aria-label={t('sessions.cmd.history')}>
          <input autoFocus className="cin__sq" value={search} placeholder={t('sessions.cmd.searchHistory')} onChange={(event) => { setSearch(event.target.value); setPick(0) }} onKeyDown={(event) => {
            if (abandoned(event)) { setSearch(null); field.current?.focus() }
            else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); setPick((was) => Math.max(0, Math.min(found.length - 1, was + (event.key === 'ArrowDown' ? 1 : -1)))) }
            else if (committed(event)) { event.preventDefault(); const chosen = found[pick]; setSearch(null); if (chosen) { setText(chosen); setWalk(null) } field.current?.focus() }
          }} />
          {found.length === 0 && <div className="cin__none">{t('sessions.cmd.noHistory')}</div>}
          {found.map((one, at) => (
            <button key={one} className="cin__hit" role="option" aria-selected={at === pick} onMouseEnter={() => setPick(at)} onMouseDown={(event) => { event.preventDefault(); setSearch(null); setText(one); field.current?.focus() }}>
              {one}
            </button>
          ))}
        </div>
      )}

      {choices && (
        <div className="cin__search cin__choices" role="listbox" aria-label={t('sessions.cmd.completions')}>
          {choices.found.map((one, at) => (
            <button key={one} className="cin__hit" role="option" aria-selected={at === choices.at} onMouseDown={(event) => { event.preventDefault(); put(choices.start, choices.caret, one); setChoices(null) }}>
              {one}
            </button>
          ))}
        </div>
      )}

      <div className="cin__box">
        <div className="cin__edit">
          {ghost && (
            <div className="cin__ghost" aria-hidden="true">
              <span className="cin__typed">{text}</span>
              {ghost}
            </div>
          )}
          <textarea
            ref={field}
            className="cin__field"
            rows={rows}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            value={text}
            placeholder={t('sessions.cmd.placeholder', { keys: keysLabel('searchHistory') })}
            aria-label={t('sessions.cmd.label')}
            onChange={(event) => {
              setText(event.target.value)
              setWalk(null)
            }}
            onKeyDown={onKey}
          />
        </div>
      </div>
    </div>
  )
})
