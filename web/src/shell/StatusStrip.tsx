import { useState } from 'react'

import { t } from '../i18n'
import { AgentChip } from './AgentChip'
import { McpChip } from './McpChip'
import { Monitor } from './Monitor'
import { PauseControl } from './PauseControl'
import { RemoteBadge } from './RemoteBadge'
import { SignInChip } from './SignInChip'
import { Version } from './Version'
import { tabOfPane } from './strip'
import { useShell } from './useShell'
import { useUsage } from './useUsage'
import { counted, cpu, measured, size } from './watching'
import { Icon } from './Icon'

/*
 * A strip along the bottom, saying what is running.
 *
 * It exists to answer one question without being asked — "is something eating
 * this machine" — which people otherwise answer by opening another program. So
 * it is two numbers and a count, and everything else is behind a click.
 *
 * It stays put. The first version hid itself whenever nothing was measured,
 * which made "nothing is running" and "this is broken" the same picture:
 * somebody opened the window, looked for the strip, and found nothing to tell
 * them which. What it does instead is go quiet — with no terminals it says so
 * in words and shows no figures, because `0% · 0 MB` is a line the eye learns
 * to skip and then misses when it matters.
 */
export function StatusStrip(): React.JSX.Element | null {
  const { project, focus, open: tabs } = useShell()
  const [open, setOpen] = useState(false)
  const usage = useUsage(project?.id ?? null, true)

  // Nothing to say about a window with no project open.
  if (!project) return null
  const numbers = measured(usage)

  return (
    <div className="strip">
      <Version />
      <PauseControl />
      <RemoteBadge />
      <SignInChip projectId={project.orchestrator ? null : project.id} />
      <McpChip projectId={project.orchestrator ? null : project.id} />
      <AgentChip />
      {open && (
        <Monitor
          usage={usage}
          onClose={() => setOpen(false)}
          onShow={(paneId) => {
            const tab = tabOfPane(tabs, paneId)
            if (tab) focus(tab)
            setOpen(false)
          }}
        />
      )}
      <button
        className="strip__go"
        aria-expanded={open}
        title={counted(usage)}
        onClick={() => setOpen((was) => !was)}
      >
        <Icon size="s"><path d="M3 17l4-6 4 3 5-8 5 5" /></Icon>
        {numbers && (
          <>
            <span className="strip__n">{cpu(usage.cpuTenths)}</span>
            <span className="strip__n">{size(usage.memoryKb)}</span>
            {/* Said, not hidden: resident memory overstates a process tree by
                half, and a total that might be wrong has to arrive labelled. */}
            {!usage.proportional && <span className="strip__warn">≈</span>}
          </>
        )}
        <span className="strip__c">
          {usage.panes.length === 0
            ? t('workspace.status.noTerminals')
            : t('workspace.status.terminals', { count: usage.panes.length })}
        </span>
      </button>
    </div>
  )
}
