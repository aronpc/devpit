import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { keysLabel } from './shortcuts'
import { setViewPref, SIZES, useViewPrefs, type ViewPref, type ViewPrefs } from './viewPrefs'

/* Rows of Settings kept in `viewPrefs`: the chat's keys in Sessions, the
   sizes, contrast, maths and diffs in Appearance. */

function Pick<K extends ViewPref>({
  field,
  title,
  said,
  choices,
}: {
  field: K
  title: string
  said: React.ReactNode
  choices: readonly (readonly [ViewPrefs[K], string])[]
}): React.JSX.Element {
  const value = useViewPrefs()[field]
  return (
    <div className="pref">
      <span className="pref__body">
        <span className="pref__t">{title}</span>
        <span className="pref__d">{said}</span>
      </span>
      <div className="seg seg--tight" role="radiogroup" aria-label={title}>
        {choices.map(([one, label]) => (
          <button key={String(one)} className="segb" role="radio" aria-checked={value === one} onClick={() => setViewPref(field, one)}>
            {label}
          </button>
        ))}
      </div>
    </div>
  )
}

const SIZE_LABELS: Record<(typeof SIZES)[number], string> = { small: 'workspace.view.small', default: 'workspace.view.default', large: 'workspace.view.large', larger: 'workspace.view.larger' }
const sizeChoices = (): (readonly [(typeof SIZES)[number], string])[] => SIZES.map((one) => [one, t(SIZE_LABELS[one])] as const)

export function ChatKeySettings(): React.JSX.Element {
  const other = keysLabel('steerNow')
  const { sendKey } = useViewPrefs()
  return (
    <>
      <Pick
        field="sendKey"
        title={t('workspace.view.sendWith')}
        said={t('workspace.view.sendWithSaid', { other })}
        choices={[
          ['enter', t('workspace.view.enter')],
          ['modEnter', other],
        ]}
      />
      <Pick
        field="followUp"
        title={t('workspace.view.whileTurn')}
        said={t('workspace.view.whileTurnSaid') + (sendKey === 'enter' ? ` ${t('workspace.view.whileTurnOther', { other })}` : '')}
        choices={[
          ['queue', t('workspace.view.queue')],
          ['steer', t('workspace.view.steer')],
        ]}
      />
    </>
  )
}

export function LookSettings(): React.JSX.Element {
  return (
    <>
      <div className="acc__sub">{t('workspace.view.text')}</div>
      <Pick field="textSize" title={t('workspace.view.textSize')} said={t('workspace.view.textSizeSaid')} choices={sizeChoices()} />
      <Pick field="codeSize" title={t('workspace.view.codeSize')} said={t('workspace.view.codeSizeSaid')} choices={sizeChoices()} />
      <Pick
        field="contrast"
        title={t('workspace.view.contrast')}
        said={t('workspace.view.contrastSaid')}
        choices={[
          ['standard', t('workspace.view.standard')],
          ['more', t('workspace.view.more')],
        ]}
      />
      <Pick
        field="latex"
        title={t('workspace.view.latex')}
        said={tRich('workspace.view.latexSaid', { inline: <code>$…$</code>, block: <code>$$…$$</code> })}
        choices={[
          [false, t('workspace.view.off')],
          [true, t('workspace.view.on')],
        ]}
      />

      <div className="acc__sub">{t('workspace.view.diffs')}</div>
      <Pick
        field="diffLayout"
        title={t('workspace.view.diffLayout')}
        said={t('workspace.view.diffLayoutSaid')}
        choices={[
          ['each', t('workspace.view.default')],
          ['split', t('workspace.view.split')],
          ['stacked', t('workspace.view.stacked')],
        ]}
      />
      <Pick
        field="diffWhitespace"
        title={t('workspace.view.whitespace')}
        said={t('workspace.view.whitespaceSaid')}
        choices={[
          ['show', t('workspace.view.show')],
          ['ignore', t('workspace.view.hide')],
        ]}
      />
      <Pick
        field="diffColours"
        title={t('workspace.view.colours')}
        said={t('workspace.view.coloursSaid')}
        choices={[
          ['standard', t('workspace.view.greenRed')],
          ['safe', t('workspace.view.blueOrange')],
        ]}
      />
      <Pick
        field="diffCollapsed"
        title={t('workspace.view.files')}
        said={t('workspace.view.filesSaid')}
        choices={[
          [true, t('workspace.view.folded')],
          [false, t('workspace.view.open')],
        ]}
      />
    </>
  )
}
