import { open as pickFiles } from '@tauri-apps/plugin-dialog'
import { useCallback, useEffect, useRef, useState } from 'react'

import type { Artifacts } from '../gen/bindings'
import { bytes } from './disk'
import { useFileDrop } from './fileDrop'
import { ask, commands } from './live'
import { PanelAct, PanelEmpty, PanelHead } from './PanelHead'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * The project's artifacts: files kept for it outside its repository, in its
 * own folder in the devpit workspace. Sessions keep and take them back with
 * devpit's tools; here they are added, opened, revealed and removed.
 */
export function ArtifactsView({ shown }: { shown: boolean }): React.JSX.Element {
  const { project, show } = useShell()
  const [found, setFound] = useState<Artifacts | null>(null)
  const [error, setError] = useState<string | null>(null)
  /* Removing asks once, in place: the window has no dialog for one file. */
  const [removing, setRemoving] = useState<string | null>(null)
  const box = useRef<HTMLDivElement>(null)

  const load = useCallback(() => {
    if (!project) return
    void ask(() => commands.artifactsList(project.id)).then((answer) => {
      setFound(answer.data ?? null)
      setError(answer.error)
    })
  }, [project])

  /* Sessions add to it while the panel is away; read again on the way in. */
  useEffect(() => {
    if (shown) load()
  }, [shown, load])

  const add = useCallback(
    (paths: readonly string[]) => {
      if (!project || paths.length === 0) return
      void ask(() => commands.artifactsAdd(project.id, [...paths])).then((answer) => {
        setError(answer.error)
        if (answer.data) setFound(answer.data)
        else load()
      })
    },
    [project, load],
  )
  const hovering = useFileDrop(box, shown ? add : null)
  const choose = (): void =>
    void pickFiles({ multiple: true, directory: false }).then((picked) => {
      if (picked) add(Array.isArray(picked) ? picked : [picked])
    })

  const remove = (name: string): void => {
    if (!project) return
    void ask(() => commands.artifactRemove(project.id, name)).then((answer) => {
      setError(answer.error)
      setRemoving(null)
      load()
    })
  }

  const items = found?.items ?? []
  return (
    <div className="arts" ref={box} data-drop={hovering ? 'true' : undefined}>
      <PanelHead title={t('workspace.artifacts.title')} meta={items.length > 0 ? items.length : null}>
        <PanelAct label={t('workspace.artifacts.add')} onClick={choose}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M12 5v14M5 12h14" /></svg>
        </PanelAct>
        {found && (
          <PanelAct label={t('workspace.artifacts.reveal')} onClick={() => void ask(() => commands.pathReveal(found.folder))}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></svg>
          </PanelAct>
        )}
      </PanelHead>
      {error && <p className="pempty__d">{error}</p>}
      {found && items.length === 0 && (
        <PanelEmpty
          title={t('workspace.artifacts.none')}
          hint={t('workspace.artifacts.hint')}
        >
          <button className="btn" onClick={choose}>
            {t('workspace.artifacts.add')}
          </button>
        </PanelEmpty>
      )}
      <ul className="arts__list">
        {items.map((one) => {
          const path = `${found!.folder}/${one.name}`
          return (
            <li className="arts__i" key={one.name}>
              <button className="arts__name" title={path} onClick={() => show('file', { id: `file:${path}`, path, title: one.name.split('/').pop() })}>
                {one.name}
              </button>
              <span className="arts__size">{bytes(one.bytes)}</span>
              {removing === one.name ? (
                <button className="arts__bad" onClick={() => remove(one.name)} onBlur={() => setRemoving(null)} autoFocus>
                  {t('workspace.artifacts.confirmRemove')}
                </button>
              ) : (
                <>
                  <button className="arts__x" aria-label={t('workspace.artifacts.openLabel', { name: one.name })} title={t('workspace.artifacts.openTitle')} onClick={() => void ask(() => commands.pathOpen(path))}>
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M7 17 17 7M8 7h9v9" /></svg>
                  </button>
                  <button className="arts__x" aria-label={t('workspace.artifacts.removeLabel', { name: one.name })} title={t('workspace.artifacts.remove')} onClick={() => setRemoving(one.name)}>
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
                  </button>
                </>
              )}
            </li>
          )
        })}
      </ul>
    </div>
  )
}
