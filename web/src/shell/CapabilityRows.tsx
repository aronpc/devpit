import { CAPABILITIES_ICON, CapabilityGlyph, enabledCount, liveCapabilities } from './capabilities'
import { usePlugins } from './usePlugins'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * Capabilities in the sidebar: where they are chosen, then one item for each
 * that is on.
 *
 * Top level and not in Resources: a capability switched on is somewhere to
 * go, and a place kept behind a popover is a place that gets forgotten.
 */

export function CapabilityRows(): React.JSX.Element {
  const { show, active } = useShell()
  const { plugins } = usePlugins()
  const on = enabledCount(plugins)

  return (
    <>
      <button className="act" onClick={() => show('plugins')} aria-pressed={active?.kind === 'plugins'}>
        <span className="act__ico">{CAPABILITIES_ICON(15)}</span>
        <span className="act__label">{t('workspace.capRows.title')}</span>
        {on ? <span className="act__n">{t('workspace.capRows.on', { count: on })}</span> : null}
      </button>

      {liveCapabilities(plugins).map(({ manifest, opens }) => (
        <button
          className="act act--live"
          key={manifest.id}
          onClick={() => show(opens.kind, opens.tab)}
          aria-pressed={active?.kind === opens.kind}
        >
          <span className="act__ico">
            <CapabilityGlyph id={manifest.id} size={15} />
          </span>
          <span className="act__label">{manifest.name}</span>
        </button>
      ))}
    </>
  )
}
