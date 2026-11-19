import { useState } from 'react'

import { open as pickFolder } from '@tauri-apps/plugin-dialog'

import { t } from '../i18n'
import { ask, commands } from './live'
import { useShell } from './useShell'
import { committed } from './typing'

/* Two ways to add a project, one decision — so it is one sheet with a choice
   inside, not two commands side by side. */

export function AddProject({ onClose }: { onClose: () => void }): React.JSX.Element {
  const { reloadProjects, setProject } = useShell()
  const [cloning, setCloning] = useState(false)
  const [url, setUrl] = useState('')
  const [into, setInto] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function clone(): Promise<void> {
    if (!url.trim()) return
    setBusy(true)
    const added = await ask(() => commands.projectClone(url.trim(), into))
    setBusy(false)
    setError(added.error)
    if (!added.data) return
    onClose()
    reloadProjects()
    setProject(added.data.id)
  }

  async function pickInto(): Promise<void> {
    const picked = await pickFolder({ directory: true, multiple: false })
    if (typeof picked === 'string') setInto(picked)
  }

  async function openFolder(): Promise<void> {
    const picked = await pickFolder({ directory: true, multiple: false })
    if (typeof picked !== 'string') return
    const added = await ask(() => commands.projectAdd(picked))
    onClose()
    if (added.data) {
      reloadProjects()
      setProject(added.data.id)
    }
  }

  return (
    <div className="ask" data-open="true" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="addpj__box" role="dialog" aria-modal="true" aria-labelledby="addpjT">
        <button className="auth__x" aria-label={t('workspace.addProject.close')} title={t('workspace.addProject.close')} onClick={onClose}><svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M18 6 6 18M6 6l12 12" /></svg></button>
        <h2 className="addpj__t" id="addpjT">{cloning ? t('workspace.addProject.cloneTitle') : t('workspace.addProject.addTitle')}</h2>
        <p className="addpj__d">
          {cloning ? t('workspace.addProject.cloneSaid') : t('workspace.addProject.addSaid')}
        </p>

        {!cloning && (
          <>
            <button className="addpj__o" onClick={() => void openFolder()}>
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
                <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
              </svg>
              <span>
                <span className="addpj__ot">{t('workspace.addProject.openFolder')}</span>
                <span className="addpj__od">
                  {t('workspace.addProject.openFolderSaid')}
                </span>
              </span>
            </button>
            <button className="addpj__o" onClick={() => setCloning(true)}>
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 3v12" />
                <path d="m8 11 4 4 4-4" />
                <path d="M4 19h16" />
              </svg>
              <span>
                <span className="addpj__ot">{t('workspace.addProject.clone')}</span>
                <span className="addpj__od">{t('workspace.addProject.cloneFirst')}</span>
              </span>
            </button>
          </>
        )}

        {cloning && (
          <>
            <label className="fld">
              <span className="fld__l">{t('workspace.addProject.repository')}</span>
              <input
                className="fld__b"
                autoFocus
                placeholder={t('workspace.addProject.urlHint')}
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                onKeyDown={(event) => committed(event) && void clone()}
              />
            </label>

            {/* Where it lands is shown before it runs. Someone deciding
                whether to clone should not be stopped to answer where. */}
            <label className="fld" style={{ marginTop: 10 }}>
              <span className="fld__l">{t('workspace.addProject.into')}</span>
              <button className="fld__b" onClick={() => void pickInto()}>
                {into ?? '~/.devpit/repos'}
              </button>
            </label>

            {error && <p className="addpj__d" style={{ marginTop: 12 }}>{error}</p>}

            <div className="ask__row">
              <button className="btn" onClick={() => setCloning(false)}>{t('workspace.addProject.back')}</button>
              <button className="btn btn--go" disabled={busy || !url.trim()} onClick={() => void clone()}>
                {busy ? t('workspace.addProject.cloning') : t('workspace.addProject.cloneGo')}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  )
}
