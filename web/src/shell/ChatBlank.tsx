import type { Project, Thread } from '../gen/bindings'
import { since } from './projects'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { Icon } from './Icon'

const PREFERENCES = 'context/preferences.md'

/* Catalog keys: what an orchestrator is good for, as things to ask it. Picking one puts it
   in the composer, unsent: a start, not a command. */
export const ORCHESTRATOR_TRIES = ['sessions.blank.orch1', 'sessions.blank.orch2', 'sessions.blank.orch3', 'sessions.blank.orch4'] as const

/* Where a project's new conversation might start, in the same way. */
export const PROJECT_TRIES = ['sessions.blank.proj1', 'sessions.blank.proj2', 'sessions.blank.proj3'] as const

/* An empty conversation: the question it is for. An orchestrator's is its own
   — it builds nothing itself; it keeps the rest moving. */
export function ChatBlank({
  project,
  account,
  onTry,
  recent = [],
  onOpen,
}: {
  project: Project | null
  account?: string
  onTry: (text: string) => void
  /** The project's last conversations, to go back to instead. */
  recent?: readonly Thread[]
  onOpen?: (thread: Thread) => void
}): React.JSX.Element {
  const orchestrating = Boolean(project?.orchestrator)
  return (
    <div className={orchestrating ? 'chat__blank chat__blank--orch' : 'chat__blank'}>
      <div className="chat__ask">
        {orchestrating ? (
          <Icon size="l" className="chat__mark"><circle cx="12" cy="12" r="3" /><circle cx="4.5" cy="5" r="2" /><circle cx="19.5" cy="5" r="2" /><circle cx="4.5" cy="19" r="2" /><circle cx="19.5" cy="19" r="2" /><path d="m6 6.4 3.8 3.6M18 6.4l-3.8 3.6M6 17.6l3.8-3.6M18 17.6l-3.8-3.6" /></Icon>
        ) : (
          <Icon size="l" className="chat__mark"><path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" /></Icon>
        )}
        {orchestrating ? (
          <>
            <h2 className="chat__q">{t('sessions.blank.orchQ')}</h2>
            <p className="chat__sub">{tRich('sessions.blank.orchSub', { account: <code>{account ?? project?.orchestrator}</code> })}</p>
            <p className="chat__sub">{tRich('sessions.blank.orchRules', { section: <i>{t('sessions.blank.rulesSection')}</i>, file: <code>{PREFERENCES}</code> })}</p>
            <div className="chat__tries">
              {ORCHESTRATOR_TRIES.map((one) => (
                <button key={one} className="chat__try" onClick={() => onTry(t(one))}>
                  {t(one)}
                </button>
              ))}
            </div>
          </>
        ) : (
          <>
            <h2 className="chat__q">
              {project ? tRich('sessions.blank.buildIn', { project: <span className="chat__where">{project.name}</span> }) : t('sessions.blank.build')}
            </h2>
            {project && (
              <div className="chat__tries">
                {PROJECT_TRIES.map((one) => (
                  <button key={one} className="chat__try chat__try--project" onClick={() => onTry(t(one))}>
                    {t(one)}
                  </button>
                ))}
              </div>
            )}
            {onOpen && recent.length > 0 && (
              <div className="chat__recent" aria-label={t('sessions.blank.recent')}>
                <div className="chat__recent-h">{t('sessions.blank.recent')}</div>
                {recent.map((one) => (
                  <button key={one.id} className="chat__again" onClick={() => onOpen(one)}>
                    <span className="chat__again-t">{one.title}</span>
                    {one.lastAt ? <span className="chat__again-w">{since(one.lastAt)}</span> : null}
                  </button>
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
