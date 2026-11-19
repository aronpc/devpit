import { useState } from 'react'

import type { DevflowCli, DevflowDetected, DevflowFound, DevflowScreen } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { DevflowCustom } from './DevflowCustom'
import { ask, commands } from './live'

/*
 * A project with no devflow.yaml: what devpit read off the repository and
 * this machine — nothing run, nothing asked — and the two ways to start.
 * Recommended writes in one click what `devpit_devflow_setup` would; Custom
 * walks through the same choices first.
 */

/* The kinds of command found, by id; the catalog names them (`devflow.setup.kind.<id>`). */
const kinds = ['test', 'e2e', 'lint', 'typecheck']

/** A list longer than this shows its first few and a count. */
const SHOWN = 8

function Row({ label, children }: { label: string; children: React.ReactNode }): React.JSX.Element {
  return (
    <div className="dfrow">
      <dt className="dfrow__k">{label}</dt>
      <dd className="dfrow__v">{children}</dd>
    </div>
  )
}

function Commands({ found }: { found: readonly DevflowFound[] }): React.JSX.Element {
  return (
    <>
      {found.map((one) => (
        <span key={one.command} className="dfcmd" data-unavailable={one.unavailable ? '' : undefined} title={one.unavailable ?? undefined}>
          <code>{one.command}</code>
          <span className="dfcmd__at">{one.source}</span>
          {one.unavailable && <span className="dfcmd__off">{t('devflow.setup.unavailable')}</span>}
        </span>
      ))}
    </>
  )
}

function Cli({ cli }: { cli: DevflowCli }): React.JSX.Element {
  const state = cli.signedIn === false ? 'out' : cli.signedIn ? 'in' : 'unknown'
  const said = t({ in: 'devflow.setup.cliIn', out: 'devflow.setup.cliOut', unknown: 'devflow.setup.cliUnknown' }[state])
  return (
    <span className="dftag" data-state={state} title={t('devflow.setup.cliTitle', { name: cli.name, state: said })}>
      <span className="dftag__dot" aria-hidden />
      {cli.name}
      {state === 'out' && <span className="dftag__note">{t('devflow.setup.cliOut')}</span>}
    </span>
  )
}

function Tags({ items }: { items: readonly string[] }): React.JSX.Element {
  const [all, setAll] = useState(false)
  const shown = all ? items : items.slice(0, SHOWN)
  return (
    <>
      {shown.map((item) => (
        <span key={item} className="dftag">
          {item}
        </span>
      ))}
      {items.length > SHOWN && (
        <button className="dfmore" onClick={() => setAll(!all)}>
          {all ? t('devflow.setup.showFewer') : t('devflow.setup.more', { count: items.length - SHOWN })}
        </button>
      )}
    </>
  )
}

function Detected({ detected }: { detected: DevflowDetected }): React.JSX.Element {
  const clis = detected.clis.filter((cli) => cli.installed)
  const byKind = (kind: string): DevflowFound[] => detected.found.filter((one) => one.kind === kind)
  const others = detected.found.filter((one) => !kinds.includes(one.kind))
  return (
    <section className="dfcard" aria-label={t('devflow.setup.foundLabel')}>
      <header className="dfcard__head">
        <h2 className="dfcard__t">{t('devflow.setup.foundLabel')}</h2>
        <span className="dfcard__tag">{t('devflow.setup.readOnly')}</span>
      </header>
      <dl className="dfgrid">
        {detected.found.length === 0 && <Row label={t('devflow.setup.checks')}>{t('devflow.setup.noChecks')}</Row>}
        {kinds.map((kind) => {
          const found = byKind(kind)
          if (found.length === 0 && !(kind === 'e2e' && detected.e2eTools.length > 0)) return null
          return (
            <Row key={kind} label={t(`devflow.setup.kind.${kind}`)}>
              <Commands found={found} />
              {kind === 'e2e' && detected.e2eTools.map((tool) => <span key={tool} className="dftag">{tool}</span>)}
            </Row>
          )
        })}
        {others.length > 0 && (
          <Row label={t('devflow.setup.other')}>
            <Commands found={others} />
          </Row>
        )}
        <Row label={t('devflow.setup.agentClis')}>{clis.length === 0 ? <span className="dfnone">{t('devflow.setup.noneInstalled')}</span> : clis.map((cli) => <Cli key={cli.name} cli={cli} />)}</Row>
        <Row label={t('devflow.setup.decisions')}>
          {detected.decisionsKey ? (
            <span className="dfsaid">{t('devflow.setup.decisionsSet')}</span>
          ) : (
            <span className="dfsaid">
              {t('devflow.setup.decisionsNotSet')}
              <span className="dfsaid__why">{t('devflow.setup.decisionsNotSetWhy')}</span>
            </span>
          )}
        </Row>
        {detected.glossary && (
          <Row label={t('devflow.setup.glossary')}>
            <span className="dftag">{detected.glossary}</span>
          </Row>
        )}
        {detected.adrs.length > 0 && (
          <Row label={t('devflow.setup.adrs')}>
            <span className="dfsaid">{t('devflow.setup.adrsCount', { count: detected.adrs.length })}</span>
          </Row>
        )}
        {detected.hints.map((hint) => (
          <Row key={hint.label} label={hint.label}>
            <Tags items={hint.items} />
          </Row>
        ))}
      </dl>
      {detected.open.length > 0 && (
        <ul className="dfopen" aria-label={t('devflow.setup.leftToYou')}>
          {detected.open.map((one) => (
            <li key={one}>{one}</li>
          ))}
        </ul>
      )}
    </section>
  )
}

/** A project without Devflow: off, and one button away from today's setup. */
export function DevflowOff({ onActivate }: { onActivate: () => void }): React.JSX.Element {
  return (
    <div className="pref dfstate" data-tone="off">
      <span className="dfstate__dot" aria-hidden />
      <span className="pref__body">
        <span className="pref__t">{t('devflow.setup.offTitle')}</span>
        <span className="pref__d">{t('devflow.setup.offBody')}</span>
      </span>
      <button className="btn btn--go" onClick={onActivate}>
        {t('devflow.setup.activate')}
      </button>
    </div>
  )
}

export function DevflowSetup({ screen, onScreen }: { screen: DevflowScreen; onScreen: (next: DevflowScreen, note: string) => void }): React.JSX.Element {
  const [started, setStarted] = useState(false)
  const [custom, setCustom] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)
  const detected = screen.detected
  const created = t('devflow.setup.created')

  const recommended = (): void =>
    void ask(() => commands.devflowCreate(screen.projectId, 'recommended', '{}')).then((answer) => {
      if (answer.data) onScreen(answer.data, created)
      setProblem(answer.error)
    })

  if (!started) return <DevflowOff onActivate={() => setStarted(true)} />
  if (custom && detected) {
    return <DevflowCustom projectId={screen.projectId} start={detected.recommended} onCreated={(next) => onScreen(next, created)} onCancel={() => setCustom(false)} />
  }
  const customFirst = screen.preset === 'custom'
  return (
    <>
      <p className="dflede">
        {tRich('devflow.setup.lede', {
          file: <code>.devpit/devflow/devflow.yaml</code>, // ui-text: ok identifier
        })}
      </p>
      {detected && <Detected detected={detected} />}
      {problem && <p className="pref__d voice__bad">{problem}</p>}
      <div className="dfpick" role="group" aria-label={t('devflow.setup.howLabel')}>
        <div className="dfpick__opt" data-main={customFirst ? undefined : ''}>
          <span className="dfpick__t">{t('devflow.setup.recommendedTitle')}</span>
          <span className="dfpick__d">{t('devflow.setup.recommendedBody')}</span>
          <button className={customFirst ? 'btn' : 'btn btn--go'} onClick={recommended}>
            {t('devflow.setup.useRecommended')}
          </button>
        </div>
        <div className="dfpick__opt" data-main={customFirst ? '' : undefined}>
          <span className="dfpick__t">{t('devflow.setup.customTitle')}</span>
          <span className="dfpick__d">{t('devflow.setup.customBody')}</span>
          <button className={customFirst ? 'btn btn--go' : 'btn'} onClick={() => setCustom(true)} disabled={!detected}>
            {t('devflow.setup.customize')}
          </button>
        </div>
      </div>
      <p className="dffoot">{t('devflow.setup.foot')}</p>
    </>
  )
}
