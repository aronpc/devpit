import type { Installations } from './useInstallations'
import { named } from './useInstallations'
import { t } from '../i18n'

/*
 * The installation a panel shows, as chips.
 *
 * Nothing at all with one installation: a picker with one choice is a label
 * pretending to be a control.
 */

export function InstallationPicker({ installations }: { installations: Installations }): React.JSX.Element | null {
  const { list, chosen, choose } = installations
  if (list.length < 2) return null
  const current = chosen ?? list.find((one) => one.default)?.directory

  return (
    <div className="agpick" role="radiogroup" aria-label={t('workspace.installation.label')}>
      {list.map((one) => (
        <button
          key={one.directory}
          className="agpick__o"
          role="radio"
          aria-checked={one.directory === current}
          title={one.directory}
          onClick={() => choose(one.directory)}
        >
          {named(one)}
          {one.default && <span className="agpick__on">{t('workspace.installation.default')}</span>}
        </button>
      ))}
    </div>
  )
}
