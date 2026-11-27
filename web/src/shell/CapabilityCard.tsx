import { useId } from 'react'

import type { PluginState } from '../gen/bindings'
import { addsInWords, CapabilityGlyph, isOn } from './capabilities'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * One capability, as a card.
 *
 * Only the switch switches. A card that also toggled on a click would flip
 * under a person reaching for Open, or selecting the description.
 *
 * Not installed, it has Install and no switch: there is nothing to turn on
 * yet. Uninstall is a quiet word at the far end from Open rather than an
 * overflow menu — the two dialogs behind it are the guard, and a menu would
 * hide the only way out.
 */

export function CapabilityCard({
  plugin,
  onSwitch,
  onOpen,
  onInstall,
  onUninstall,
}: {
  plugin: PluginState
  onSwitch: () => void
  /** Null for a capability with nowhere to open. */
  onOpen: (() => void) | null
  onInstall: () => void
  onUninstall: () => void
}): React.JSX.Element {
  const { manifest, installed } = plugin
  const on = isOn(plugin)
  const described = useId()

  return (
    <article className="capcard" data-on={on} data-installed={installed} aria-label={manifest.name}>
      <div className="capcard__head">
        <span className="capcard__ico">
          <CapabilityGlyph id={manifest.id} size={17} />
        </span>
        <span className="capcard__id">
          <span className="capcard__name">{manifest.name}</span>
          <span className="capcard__ver">{manifest.version}</span>
        </span>
        {installed && (
          <button
            className="capcard__sw"
            role="switch"
            aria-checked={on}
            aria-label={manifest.name}
            aria-describedby={described}
            onClick={onSwitch}
          >
            <span className="sw"></span>
          </button>
        )}
      </div>

      <p className="capcard__what" id={described}>
        {manifest.description}
      </p>
      <ul className="capcard__adds" aria-label={t('workspace.capCard.adds')}>
        {addsInWords(manifest).map((words) => (
          <li className="capcard__add" key={words}>
            {words}
          </li>
        ))}
      </ul>

      <div className="capcard__foot">
        <span className="capcard__state">{!installed ? t('workspace.capCard.notInstalled') : on ? t('workspace.capCard.on') : t('workspace.capCard.off')}</span>
        {installed ? (
          <>
            <button className="capcard__uninstall" onClick={onUninstall} aria-label={t('workspace.capCard.uninstallLabel', { name: manifest.name })}>
              {t('workspace.capCard.uninstall')}
            </button>
            {on && onOpen && (
              <button className="capcard__open" onClick={onOpen} aria-label={t('workspace.capCard.openLabel', { name: manifest.name })}>
                {t('workspace.capCard.open')}
                <Icon size="s" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></Icon>
              </button>
            )}
          </>
        ) : (
          <button className="capcard__install" onClick={onInstall} aria-label={t('workspace.capCard.installLabel', { name: manifest.name })}>
            <Icon size="s" aria-hidden="true"><path d="M12 5v14M5 12h14" /></Icon>
            {t('workspace.capCard.install')}
          </button>
        )}
      </div>
    </article>
  )
}
