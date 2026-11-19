import type { Usage } from '../gen/bindings'
import { t } from '../i18n'
import { AgentGlyph } from './AgentGlyph'
import { counted, cpu, notable, size } from './watching'

/*
 * What each terminal is costing.
 *
 * By pane rather than by process, because a pane is the thing you can act on:
 * `node` twice over says nothing, and "Claude Code, three processes, 1.06 GB"
 * says what to close.
 *
 * The whole tree under each pane is counted. An agent's cost is mostly its
 * children — the language server, the MCP servers it spawned — and a number
 * that leaves them out is a number saying an agent is free.
 */

export function Monitor({
  usage,
  onClose,
  onShow,
}: {
  usage: Usage
  onClose: () => void
  onShow: (paneId: string) => void
}): React.JSX.Element {
  const rows = notable(usage.panes)

  return (
    <div className="hud" role="dialog" aria-modal="false" aria-label={t('workspace.monitor.label')}>
      <div className="hud__top">
        <span className="hud__t">{t('workspace.monitor.terminals')}</span>
        <span className="hud__sum" title={counted(usage)}>
          {cpu(usage.cpuTenths)} · {size(usage.memoryKb)}
          {/* The total says how it was counted rather than hoping nobody
              asks: resident memory overstates a process tree by half. */}
          {!usage.proportional && <span className="hud__warn">≈</span>}
        </span>
        <button className="sq26" onClick={onClose} aria-label={t('workspace.monitor.close')} title={t('workspace.monitor.close')}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
        </button>
      </div>

      {rows.length === 0 && (
        <p className="acc__note">{t('workspace.monitor.nothing')}</p>
      )}

      {rows.map((pane) => (
        <button className="hudrow" key={pane.paneId} onClick={() => onShow(pane.paneId)}>
          <span className="hudrow__ico">
            <AgentGlyph agent={pane.agent ?? ''} />
          </span>
          <span className="hudrow__body">
            <span className="hudrow__n">{pane.label}</span>
            {/* How many processes, because an agent with fifteen of them is
                worth knowing about even when the memory looks ordinary. */}
            <span className="hudrow__d">
              {t('workspace.monitor.processes', { count: pane.processes })}
            </span>
          </span>
          <span className="hudrow__cpu">{cpu(pane.cpuTenths)}</span>
          <span className="hudrow__mem">{size(pane.memoryKb)}</span>
        </button>
      ))}
    </div>
  )
}
