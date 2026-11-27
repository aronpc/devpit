import { useDetails, useDetailsMenu } from './chatDetails'
import { useShell } from './useShell'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * Where the work happens: the project, the machine and the branch.
 *
 * Quieter than the composer's own chips, because these describe the
 * conversation rather than steer the turn.
 */

export function ChatWhere(): React.JSX.Element {
  const { project } = useShell()
  const shows = useDetails()
  const details = useDetailsMenu()
  return (
    <div className="composer__where" onContextMenu={details.open}>
                {details.menu}
                {shows('project') && <span className="wschip">
                  <Icon size="s"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></Icon>
                  {project?.name ?? t('sessions.chat.noProject')}
                </span>}
                {shows('machine') && <span className="wschip">
                  <Icon size="s"><rect x="2" y="3" width="20" height="14" rx="2" /><path d="M8 21h8M12 17v4" /></Icon>
                  {t('sessions.chat.local')}
                </span>}
                {shows('branch') && project?.worktrees[0] && (
                  <span className="wschip">
                    <Icon size="s"><line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" /></Icon>
                    {project.worktrees[0].branch}
                  </span>
                )}
              </div>
  )
}
