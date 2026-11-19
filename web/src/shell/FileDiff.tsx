import { useCallback, useEffect, useRef, useState } from 'react'

import { ChevronDown, ChevronUp, Columns, Fold, Rows, Wrap } from './GitIcons'
import { ofPath } from './languages'
import { ask, commands } from './live'
import { mergeInto, toChange, type Merged, type Sides } from './mergeEditor'
import { reason } from './reason'
import { matches } from './shortcuts'
import { viewPrefs } from './viewPrefs'
import { t } from '../i18n'

/*
 * One file's change against HEAD, the whole file lined up.
 *
 * The side on disk is an editor: reading a diff is when a stray line gets
 * noticed, and fixing it should not mean finding the file somewhere else.
 * Saved with the same stale-write check as the file view, so a save never
 * overwrites a change it did not see.
 */

export function FileDiff({
  projectId,
  path,
  bar,
}: {
  projectId: string
  path: string
  /** Draws the pane's bar around this diff's own controls. */
  bar: (controls: React.ReactNode) => React.JSX.Element
}): React.JSX.Element {
  const [sides, setSides] = useState<Sides | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [editable, setEditable] = useState(false)
  const [split, setSplit] = useState(() => viewPrefs().diffLayout !== 'stacked')
  const [folded, setFolded] = useState(true)
  const [wrap, setWrap] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [saving, setSaving] = useState(false)

  const host = useRef<HTMLDivElement>(null)
  const merged = useRef<Merged | null>(null)
  /* The text as edited, kept out of state: a keystroke must not rebuild the
     editor it was typed into. Read back when a toggle does rebuild it. */
  const text = useRef<string | null>(null)
  const readAt = useRef(0)
  /* What is on disk as far as this pane knows: "unsaved" is measured from
     here, and a save moves it without rebuilding the editor. */
  const saved = useRef('')

  useEffect(() => {
    let dropped = false
    setSides(null)
    setError(null)
    setDirty(false)
    text.current = null
    void Promise.all([
      ask(() => commands.fileAtHead(projectId, null, path)),
      ask(() => commands.fileRead(projectId, null, path)),
    ]).then(([head, disk]) => {
      if (dropped) return
      if (head.error) return setError(head.error)
      /* No file on disk is a deletion: the right side is empty and there is
         nothing to edit. A file that is there but not text says why. */
      if (disk.data && disk.data.text === null) {
        return setError(disk.data.notShown ?? t('workspace.fileDiff.notText'))
      }
      readAt.current = disk.data?.readAt ?? 0
      saved.current = disk.data?.text ?? ''
      setEditable(disk.data !== null)
      setSides({ original: head.data ?? '', modified: disk.data?.text ?? '' })
    })
    return () => {
      dropped = true
    }
  }, [projectId, path])

  useEffect(() => {
    const parent = host.current
    if (!parent || !sides) return
    const shown = { ...sides, modified: text.current ?? sides.modified }
    merged.current = mergeInto(parent, shown, ofPath(path), { split, folded, wrap, editable }, (now) => {
      text.current = now
      setDirty(now !== saved.current)
    })
    return () => {
      merged.current?.destroy()
      merged.current = null
    }
  }, [sides, path, split, folded, wrap, editable])

  const save = useCallback(() => {
    const now = text.current
    if (now === null || saving) return
    setSaving(true)
    void ask(() => commands.fileWrite(projectId, null, path, now, readAt.current))
      .then((answer) => {
        if (!answer.data) return setError(answer.error ?? 'the file could not be saved')
        readAt.current = answer.data.readAt ?? readAt.current
        setError(null)
        saved.current = now
        setDirty(text.current !== now)
      })
      .catch((thrown: unknown) => setError(reason(thrown, 'the file could not be saved')))
      .finally(() => setSaving(false))
  }, [projectId, path, saving])

  const go = (forward: boolean) => () => {
    if (merged.current) toChange(merged.current.editor, forward)
  }

  return (
    <>
      {bar(
        <>
          <button className="dbtn" onClick={go(false)} title={t('workspace.fileDiff.previous')} aria-label={t('workspace.fileDiff.previous')}>
            <ChevronUp />
          </button>
          <button className="dbtn" onClick={go(true)} title={t('workspace.fileDiff.next')} aria-label={t('workspace.fileDiff.next')}>
            <ChevronDown />
          </button>
          <span className="dbar__sep" />
          <button className="dbtn" aria-pressed={split} onClick={() => setSplit((was) => !was)} title={split ? t('workspace.fileDiff.inline') : t('workspace.fileDiff.sideBySide')}>
            {split ? <Rows /> : <Columns />}
          </button>
          <button className="dbtn" aria-pressed={!folded} onClick={() => setFolded((was) => !was)} title={folded ? t('workspace.fileDiff.showWhole') : t('workspace.fileDiff.fold')}>
            <Fold />
          </button>
          <button className="dbtn" aria-pressed={wrap} onClick={() => setWrap((was) => !was)} title={t('workspace.fileDiff.wrap')}>
            <Wrap />
          </button>
          {dirty && (
            <button className="dbtn dbtn--go" disabled={saving} onClick={save}>
              {saving ? t('workspace.fileDiff.saving') : t('workspace.fileDiff.save')}
            </button>
          )}
        </>,
      )}
      {error && <div className="exempty__t">{error}</div>}
      {/* ⌘S / Ctrl+S saves, as it does everywhere else a file is edited. */}
      <div
        className="mdiff"
        ref={host}
        onKeyDown={(event) => {
          if (matches('save', event)) {
            event.preventDefault()
            save()
          }
        }}
      />
    </>
  )
}
