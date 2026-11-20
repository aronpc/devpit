import { useEffect, useState } from 'react'

import { bytes } from './disk'
import { Code } from './Code'
import { ofPath } from './languages'
import { ask, commands } from './live'
import { Markdown } from './MarkdownView'
import { PdfView } from './PdfView'
import { Picture } from './Picture'
import { went } from './problems'
import { Streamed } from './Streamed'
import type { Tab } from './strip'
import { useFile } from './useFile'
import { useShell } from './useShell'
import { useShortcut } from './useShortcut'
import { useWantedLine } from './revealLine'
import { t } from '../i18n'

/*
 * One file, one tab.
 *
 * What is drawn comes from the backend's reading of the file, not from its
 * extension here: the first bytes decide, so a PNG named `notes.md` is a
 * picture rather than a pane full of mojibake.
 */

export function FilePane({ tab }: { tab: Tab }): React.JSX.Element {
  const { close, active, markUnsaved, project } = useShell()
  const path = tab.path ?? null
  const edit = useFile(path)
  const line = useWantedLine(path, edit.file !== null)
  const [preview, setPreview] = useState(true)

  const mine = active?.id === tab.id
  const kind = edit.file?.kind

  /* The strip draws the cross and this pane holds the edit, so the answer to
     "would closing lose anything" has to travel between them. Cleared on the
     way out: a tab that closed is not a tab with unsaved work. */
  useEffect(() => {
    markUnsaved(tab.id, edit.dirty)
    return () => markUnsaved(tab.id, false)
  }, [tab.id, edit.dirty, markUnsaved])

  /* ⌘S saves the tab in front, and only that one. */
  useShortcut('save', () => {
    if (edit.dirty) edit.save()
  }, mine)

  const name = path?.split('/').pop() ?? t('workspace.filePane.noFile')
  const editing = kind === 'text' || (kind === 'markdown' && !preview)

  return (
    <>
      <div className="pane__bar">
        <span className="pane__t">
          <b>{name}</b>
          {path ? ` · ${path}` : ''}
          {edit.dirty ? ' ·' : ''}
        </span>
        <span className="drag" />
        {kind === 'markdown' && (
          <button className="chip" onClick={() => setPreview((was) => !was)}>
            {preview ? t('workspace.workspaceFile.source') : t('workspace.workspaceFile.preview')}
          </button>
        )}
        {edit.dirty && (
          <button className="chip" onClick={edit.save} disabled={edit.saving}>
            {edit.saving ? t('workspace.filePane.saving') : t('workspace.filePane.save')}
          </button>
        )}
        <button className="sq26" onClick={() => close(tab.id)} aria-label={t('workspace.filePane.close')} title={t('workspace.filePane.close')}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
        </button>
      </div>

      {edit.clash && (
        <div className="clash">
          <span className="clash__t">{edit.clash}</span>
          <button className="btn" onClick={edit.reload}>
            {t('workspace.filePane.reload')}
          </button>
          <button className="btn btn--danger" onClick={edit.overwrite}>
            {t('workspace.filePane.overwrite')}
          </button>
        </div>
      )}

      <div className={editing ? 'code code--edit' : 'code'}>
        {!path && (
          <div className="exempty">
            <span className="exempty__t">{t('workspace.filePane.noFile')}</span>
            <span className="exempty__d">{t('workspace.filePane.pickOne')}</span>
          </div>
        )}
        {edit.error && <div className="exempty__t">{edit.error}</div>}

        {!edit.error && edit.file?.notShown && (
          <div className="exempty">
            <span className="exempty__t">{edit.file.notShown}</span>
            <span className="exempty__d">{bytes(edit.file.bytes) ?? t('workspace.filePane.empty')}</span>
            <span className="voice__row">
              <button className="btn" onClick={() => void ask(() => commands.pathOpen(edit.file!.fullPath)).then(went)}>
                {t('workspace.filePane.openApp')}
              </button>
              <button
                className="btn"
                onClick={() => void ask(() => commands.pathReveal(edit.file!.fullPath))}
              >
                {t('workspace.filePane.reveal')}
              </button>
            </span>
          </div>
        )}

        {kind === 'image' && edit.file?.dataUrl && (
          <Picture key={`${path}:${edit.file.readAt}`} src={edit.file.dataUrl} name={name} size={edit.file.bytes} />
        )}

        {path && edit.file && !edit.file.notShown && ((kind === 'image' && !edit.file.dataUrl) || kind === 'video' || kind === 'audio') && (
          <Streamed key={`${path}:${edit.file.readAt}`} kind={kind} projectId={project?.id ?? null} path={path} name={name} size={edit.file.bytes} />
        )}

        {kind === 'pdf' && edit.file?.dataUrl && <PdfView dataUrl={edit.file.dataUrl} fullPath={edit.file.fullPath} name={name} />}

        {kind === 'markdown' && preview && <Markdown source={edit.text} />}

        {editing && (
          <Code
            text={edit.text}
            language={path ? ofPath(path) : null}
            /* A file outside the project is shown, not edited: saving goes
               through the project it belongs to. */
            onChange={path?.startsWith('/') ? undefined : edit.change}
            readOnly={path?.startsWith('/')}
            line={line}
          />
        )}
      </div>
    </>
  )
}
