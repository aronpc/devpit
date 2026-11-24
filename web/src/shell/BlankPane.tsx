import mark from '../assets/brand/mark.png'
import { t } from '../i18n'
import { AgentMark } from './AgentMark'
import type { Shell } from './shape'
import { useKeys } from './useShortcut'
import { offered, useKnownAgents } from './useKnownAgents'

/* A project with nothing open: the ways to start, the agents this machine
   has among them — one click from a terminal running them, which is what
   most people open a project to do. */
export function BlankPane({ name, show }: { name: string; show: Shell['show'] }): React.JSX.Element {
  const agents = offered(useKnownAgents()).filter((agent) => agent.installed)
  const keys = useKeys()
  return (
    <div className="blank">
      <span className="blank__mark"><img
          className="mark"
          alt="" // ui-text: ok an empty literal — the walk misreads it as the rest of the line
          src={mark}
        /></span>
      <div className="blank__name">{name}</div>
      <div className="blank__sub">{t('workspace.blank.sub')}</div>
      <div className="blank__keys">
        <button className="blank__k" onClick={() => show('chat')}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9.9 9.9 0 0 1-4.2-.9L3 20.5l1.6-4.4A8.4 8.4 0 0 1 3.6 11.5 8.4 8.4 0 0 1 12 3.1a8.4 8.4 0 0 1 9 8.4Z" /></svg>
          <b>{t('workspace.blank.newChat')}</b><span>{keys('chat')}</span>
        </button>
        <button className="blank__k" onClick={() => show('term')}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="m4 17 6-6-6-6M12 19h8" /></svg>
          <b>{t('workspace.blank.newTerminal')}</b><span>{keys('terminal')}</span>
        </button>
        {agents.map((agent) => (
          <button key={agent.id} className="blank__k" onClick={() => show('term', { title: agent.label, launch: agent.id })}>
            <AgentMark agent={agent.id} />
            <b>{agent.label}</b><span>{t('workspace.blank.inTerminal')}</span>
          </button>
        ))}
        <button className="blank__k" onClick={() => show('board')}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M9 3v18M15 3v18" /></svg>
          <b>{t('workspace.blank.board')}</b>
        </button>
      </div>
    </div>
  )
}
