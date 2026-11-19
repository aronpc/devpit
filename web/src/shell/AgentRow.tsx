import { AgentGlyph } from './AgentGlyph'
import type { Entry } from './catalogue'
import { IdleCompact } from './IdleCompact'
import { ProfileEditor } from './ProfileEditor'
import { ProfileModels } from './ProfileModels'
import type { Draft } from './profiles'
import type { KnownAgent } from '../gen/bindings'
import { ask, commands } from './live'
import { t } from '../i18n'

/*
 * One agent in the catalogue.
 *
 * Three controls and a command, which is what the row is for: whether it is
 * offered, whether it is the one a new terminal opens, and what it actually
 * runs. The command is shown rather than described — `claude --permission-mode
 * bypassPermissions` is the whole answer to "what will this do", and a row
 * that hides it behind a menu makes you open the menu to find out.
 */

export function AgentRow({
  entry,
  agents,
  open,
  draft,
  busy,
  onOpen,
  onDraft,
  onSave,
  onRemove,
  onDefault,
  onEnabled,
}: {
  entry: Entry
  agents: readonly KnownAgent[]
  open: boolean
  draft: Draft | null
  busy: boolean
  onOpen: () => void
  onDraft: (draft: Draft) => void
  onSave: () => void
  onRemove: () => void
  onDefault: () => void
  onEnabled: (on: boolean) => void
}): React.JSX.Element {
  return (
    <>
      <div className={`prov${entry.installed ? '' : ' prov--off'}`}>
        <span className="prov__ico">
          <AgentGlyph agent={entry.id} base={entry.base} />
          <span
            className="prov__dot"
            style={{ background: entry.installed ? 'var(--success)' : 'var(--ghost)' }}
          ></span>
        </span>
        <span className="prov__body">
          <span className="prov__top">
            <span className="prov__n">{entry.label}</span>
          </span>
          {/* The line, not a description of it. */}
          <span className="prov__sub">{entry.launch}</span>
          {entry.note && <span className="prov__sub prov__note">{entry.note}</span>}
        </span>

        {/* Offered or not. A person with twelve agents uses two. */}
        <span className="seg seg--tight">
          <button
            className="segb"
            aria-selected={entry.enabled}
            disabled={busy}
            onClick={() => onEnabled(true)}
          >
            {t('workspace.agentRow.enabled')}
          </button>
          <button
            className="segb"
            aria-selected={!entry.enabled}
            disabled={busy}
            onClick={() => onEnabled(false)}
          >
            {t('workspace.agentRow.disabled')}
          </button>
        </span>

        <button
          className="acc__act"
          disabled={busy || entry.isDefault || !entry.installed || !entry.enabled}
          aria-pressed={entry.isDefault}
          title={
            entry.installed
              ? t('workspace.agentRow.defaultTitle')
              : t('workspace.agentRow.notInstalled')
          }
          onClick={onDefault}
        >
          {entry.isDefault ? t('workspace.agentRow.isDefault') : t('workspace.agentRow.setDefault')}
        </button>

        {/* Its own documentation, rather than devpit explaining how to
            install something it does not ship. */}
        {entry.homepage && (
          <a
            className="acc__act"
            href={entry.homepage}
            /* Opened by the app, not by the window: nothing answers wry's
               new-window request, so this link did nothing when clicked. */
            onClick={(event) => {
              event.preventDefault()
              void ask(() => commands.urlOpen(entry.homepage as string))
            }}
            title={t('workspace.agentRow.docs', { name: entry.label })}
            aria-label={t('workspace.agentRow.docs', { name: entry.label })}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M14 4h6v6M20 4 10 14M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" /></svg>
          </a>
        )}

        <button
          className="acc__act"
          aria-expanded={open}
          aria-label={t(open ? 'workspace.agentRow.hide' : 'workspace.agentRow.show', { name: entry.label })}
          title={t(open ? 'workspace.agentRow.hide' : 'workspace.agentRow.show', { name: entry.label })}
          onClick={onOpen}
        >
          {open ? '⌃' : '⌄'}
        </button>
      </div>

      {open && draft && (
        <ProfileEditor
          draft={draft}
          agents={agents}
          onChange={onDraft}
          onSave={onSave}
          onCancel={onOpen}
          onRemove={entry.mine ? onRemove : undefined}
        />
      )}
      {open && <ProfileModels profileId={entry.id} claude={entry.base === 'claude'} draft={entry.mine ? draft : null} onDraft={onDraft} />}
      {open && entry.base === 'claude' && <IdleCompact profileId={entry.id} />}
    </>
  )
}
