import { useEffect, useState } from 'react'

import type { ClaudePluginInstallation } from '../gen/bindings'
import { dismiss, dismissed, offerFor } from './claudePlugin'
import { ask, commands } from './live'
import { t } from '../i18n'

/*
 * "Install devpit plugin", in the strip under a terminal running Claude Code.
 *
 * The plugin is what gives a session started by the person's own command —
 * `claude2`, a function, an alias — devpit's hooks and board tools, which the
 * flags devpit puts on its own launch lines never reach. Installing is theirs
 * to choose, so it is offered here and done on a click, the way Warp offers
 * its own.
 */
type Step = { kind: 'offered' } | { kind: 'working' } | { kind: 'done' } | { kind: 'failed'; why: string }

export function PluginChip(): React.JSX.Element | null {
  const [installations, setInstallations] = useState<ClaudePluginInstallation[] | null>(null)
  const [step, setStep] = useState<Step>({ kind: 'offered' })
  const [hidden, setHidden] = useState(false)

  useEffect(() => {
    let live = true
    void ask(() => commands.claudePluginState()).then((answer) => live && setInstallations(answer.data ?? null))
    return () => {
      live = false
    }
  }, [])

  if (!installations || hidden) return null
  const offer = offerFor(installations)
  if (step.kind === 'offered' && (offer === null || dismissed(offer, installations))) return null

  const install = (): void => {
    setStep({ kind: 'working' })
    void ask(() => commands.claudePluginInstall()).then((answer) => {
      if (answer.data) {
        setInstallations(answer.data)
        setStep({ kind: 'done' })
      } else setStep({ kind: 'failed', why: answer.error ?? t('workspace.pluginChip.whyDefault') })
    })
  }
  const close = (): void => {
    if (step.kind === 'offered') dismiss(installations)
    setHidden(true)
  }

  const update = offer === 'update'
  return (
    <span className="pchip" data-step={step.kind} title={step.kind === 'failed' ? step.why : undefined}>
      {step.kind === 'offered' && (
        <button className="pchip__go" onClick={install} title={t('workspace.pluginChip.offerTitle')}>
          {t(update ? 'workspace.pluginChip.update' : 'workspace.pluginChip.install')}
        </button>
      )}
      {step.kind === 'working' && <span className="pchip__say">{t(update ? 'workspace.pluginChip.updating' : 'workspace.pluginChip.installing')}</span>}
      {step.kind === 'done' && <span className="pchip__say">{t('workspace.pluginChip.done')}</span>}
      {step.kind === 'failed' && <span className="pchip__say">{t('workspace.pluginChip.failed')}</span>}
      {step.kind !== 'working' && (
        <button className="pchip__x" aria-label={t('workspace.pluginChip.dismiss')} title={t('workspace.pluginChip.dismiss')} onClick={close}>
          ×
        </button>
      )}
    </span>
  )
}
