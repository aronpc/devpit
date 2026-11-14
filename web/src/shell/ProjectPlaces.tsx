import { useEffect, useState } from 'react'

import { ask, commands } from './live'
import type { PaneName } from './paneList'
import { useShell } from './useShell'

/*
 * The project's own places in the sidebar: its board, and its Devflow beside
 * it — set up, switched on or not, every project has one to look at. An
 * orchestrator has neither: its folder is notes, and the projects' boards
 * are in its right panel. It has its Schedules instead. A project also has
 * its agent to ask and its agent's memory, unless Labs has the project agent
 * off.
 */
export function ProjectPlaces({ open, showing }: { open: (kind: PaneName) => void; showing: (kind: PaneName) => boolean }): React.JSX.Element | null {
  const shell = useShell()
  const [agent, setAgent] = useState(true)
  /* Settings is the one place Labs flips, so it is read again as Settings closes. */
  const settled = !shell.prefs
  useEffect(() => {
    if (settled) void ask(() => commands.maintenanceRead()).then((answer) => setAgent(answer.data?.projectAgent ?? true))
  }, [settled])
  if (shell.project?.orchestrator) {
    return (
      <button className="act" onClick={() => open('schedules')} aria-pressed={showing('schedules')}>
        <span className="act__ico"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="18" rx="2" /><path d="M16 2v4M8 2v4M3 10h18M12 14v3l2 1" /></svg></span>
        <span className="act__label">Schedules</span>
      </button>
    )
  }
  return (
    <>
      <button className="act" onClick={() => open('board')} aria-pressed={showing('board')}>
        <span className="act__ico"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M9 3v18M15 3v18" /></svg></span>
        <span className="act__label">Board</span>
      </button>
      <button className="act" onClick={() => open('devflow')} aria-pressed={showing('devflow')}>
        <span className="act__ico"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><circle cx="5" cy="6" r="2" /><circle cx="12" cy="18" r="2" /><circle cx="19" cy="6" r="2" /><path d="M5 8v2a4 4 0 0 0 4 4h1M19 8v2a4 4 0 0 1-4 4h-1" /></svg></span>
        <span className="act__label">Devflow</span>
      </button>
      {agent && (
      <button className="act" onClick={() => open('ask')} aria-pressed={showing('ask')}>
        <span className="act__ico"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9" /><path d="M9.5 9.2a2.6 2.6 0 0 1 5 .8c0 1.7-2.5 2.3-2.5 3.8M12 17h.01" /></svg></span>
        <span className="act__label">Ask the project</span>
      </button>
      )}
      {agent && (
      <button className="act" onClick={() => open('memory')} aria-pressed={showing('memory')}>
        <span className="act__ico"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M12 4a3 3 0 0 0-3 3 3 3 0 0 0-3 3c0 1 .4 1.8 1 2.4A3 3 0 0 0 9 18a3 3 0 0 0 3 2 3 3 0 0 0 3-2 3 3 0 0 0 2-5.6c.6-.6 1-1.4 1-2.4a3 3 0 0 0-3-3 3 3 0 0 0-3-3Z" /><path d="M12 4v16" /></svg></span>
        <span className="act__label">Project agent</span>
      </button>
      )}
    </>
  )
}
