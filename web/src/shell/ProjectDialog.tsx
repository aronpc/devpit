import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'

import type { Project, WorktreeSetup } from '../gen/bindings'
import { t } from '../i18n'
import { GroupPicker } from './GroupPicker'
import { ask, commands } from './live'
import { ProjectHosts } from './ProjectHosts'
import { COLOURS, ICONS, iconName, ProjectMark } from './ProjectMark'
import { abandoned, committed } from './typing'
import { SecretFields } from './SecretFields'
import { useShell } from './useShell'
import { WorktreeSetupFields } from './WorktreeSetupFields'

/*
 * A project's name, group and mark, set in one place.
 *
 * Opened from the rail's right-click, and on its own when a project has just
 * been added — the moment somebody is looking at a new icon is the moment they
 * want to change it. The preview is the rail's own mark, so what you choose is
 * what you get.
 */

export function ProjectDialog({ project, onClose }: { project: Project; onClose: () => void }): React.JSX.Element {
  const { projects, reloadProjects } = useShell()
  const [name, setName] = useState(project.name)
  const [group, setGroup] = useState(project.group ?? '')
  const [icon, setIcon] = useState<string | null>(project.icon)
  const [color, setColor] = useState<string | null>(project.color)
  const [emoji, setEmoji] = useState(project.icon && !iconName(project.icon) ? project.icon : '')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  /* A worktree is a git project's; an orchestrator's folder never gets one. */
  const [setup, setSetup] = useState<WorktreeSetup | null>(null)
  const [setupChanged, setSetupChanged] = useState(false)
  useEffect(() => {
    if (project.orchestrator) return
    void ask(() => commands.projectWorktreeSetup(project.id)).then((answer) => setSetup(answer.data))
  }, [project])

  const groups = [...new Set(projects.map((one) => one.group).filter((one): one is string => Boolean(one)))].sort()
  const preview = { id: project.id, name: name || project.name, icon, color }

  const save = (): void => {
    setBusy(true)
    void ask(() => commands.projectEdit(project.id, name, group || null, icon, color))
      .then(async (answer) => {
        if (answer.error) return setError(answer.error)
        if (setup && setupChanged) {
          const kept = await ask(() => commands.projectWorktreeSetupSet(project.id, setup))
          if (kept.error) return setError(kept.error)
        }
        reloadProjects()
        onClose()
      })
      .finally(() => setBusy(false))
  }

  /* On the app's frame, where the other dialogs are: opened from the rail,
     whose panel clips, it was drawn cut off at the rail's edge. */
  return createPortal(
    <div className="ask" data-open="true" onClick={(event) => event.target === event.currentTarget && onClose()} onKeyDown={(event) => abandoned(event) && onClose()}>
      <div className="addpj__box pdlg" role="dialog" aria-modal="true" aria-labelledby="pdlgT">
        <div className="pdlg__head">
          <ProjectMark project={preview} size={44} />
          <div>
            <h2 className="addpj__t" id="pdlgT">{t('workspace.projectDialog.title')}</h2>
            <p className="addpj__d" title={project.rootPath}>{project.rootPath}</p>
          </div>
        </div>

        <label className="pdlg__f">
          <span>{t('workspace.projectDialog.name')}</span>
          <input autoFocus value={name} onChange={(event) => setName(event.target.value)} onKeyDown={(event) => committed(event) && save()} />
        </label>

        {/* An orchestrator sits in the Orchestrators group, always. */}
        {!project.orchestrator && (
          <div className="pdlg__f">
            <span>{t('workspace.projectDialog.group')}</span>
            <GroupPicker value={group} groups={groups} onChange={setGroup} />
          </div>
        )}

        <div className="pdlg__f">
          <span>{t('workspace.projectDialog.icon')}</span>
          <div className="pdlg__icons">
            <button className="pdlg__icon" aria-pressed={icon === null} onClick={() => setIcon(null)} title={t('workspace.projectDialog.initials')}>
              {t('workspace.projectDialog.initialsMark')}
            </button>
            {Object.entries(ICONS).map(([key, paths]) => (
              <button key={key} className="pdlg__icon" aria-pressed={icon === `icon:${key}`} onClick={() => setIcon(`icon:${key}`)} title={key}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  {paths.map((d) => (
                    <path key={d} d={d} />
                  ))}
                </svg>
              </button>
            ))}
          </div>
          <input
            className="pdlg__emoji"
            value={emoji}
            placeholder={t('workspace.projectDialog.emoji')}
            maxLength={8}
            onChange={(event) => {
              const typed = event.target.value.trim()
              setEmoji(typed)
              if (typed) setIcon(typed)
            }}
          />
        </div>

        <div className="pdlg__f">
          <span>{t('workspace.projectDialog.colour')}</span>
          <div className="pdlg__colours">
            <button className="pdlg__sw pdlg__sw--auto" aria-pressed={color === null} onClick={() => setColor(null)} title={t('workspace.projectDialog.automatic')} />
            {COLOURS.map((one) => (
              <button key={one} className="pdlg__sw" style={{ background: one }} aria-pressed={color === one} onClick={() => setColor(one)} title={one} />
            ))}
            <input type="color" className="pdlg__pick" value={color ?? '#5b9be2'} onChange={(event) => setColor(event.target.value)} title={t('workspace.projectDialog.anyColour')} />
          </div>
        </div>

        {setup && (
          <WorktreeSetupFields
            setup={setup}
            onChange={(next) => {
              setSetup(next)
              setSetupChanged(true)
            }}
          />
        )}

        {!project.orchestrator && <SecretFields projectId={project.id} />}
        {!project.orchestrator && <ProjectHosts projectId={project.id} />}

        {error && <p className="acc__note">{error}</p>}
        <div className="pdlg__acts">
          <button className="btn" onClick={onClose}>
            {t('workspace.projectDialog.cancel')}
          </button>
          <button className="btn btn--go" disabled={busy || !name.trim()} onClick={save}>
            {t('workspace.projectDialog.save')}
          </button>
        </div>
      </div>
    </div>,
    document.querySelector('.app') ?? document.body,
  )
}
