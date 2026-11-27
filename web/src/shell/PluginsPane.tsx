import { useState } from 'react'

import type { PluginManifest } from '../gen/bindings'
import { CapabilityCard } from './CapabilityCard'
import { InstallCapability, uninstalledInWords, UninstallCapability } from './CapabilityDialogs'
import {
  addsInWords,
  CAPABILITIES_ICON,
  CAPABILITY_FILTERS,
  opensFor,
  shownCapabilities,
  summary,
  type CapabilityFilter,
} from './capabilities'
import { ask, commands } from './live'
import { usePlugins } from './usePlugins'
import { useShell } from './useShell'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * The capabilities devpit ships, and which this project installed and has on.
 *
 * Nothing to browse: every capability is compiled into the app. Installing
 * one is a choice per project, and both directions ask first.
 */

function Quiet({ title, detail }: { title: string; detail?: string | null }): React.JSX.Element {
  return (
    <div className="exempty capset__quiet">
      {CAPABILITIES_ICON(22)}
      <span className="exempty__t">{title}</span>
      {detail && <span className="exempty__d">{detail}</span>}
    </div>
  )
}

interface Uninstalling {
  readonly manifest: PluginManifest
  /** Null until its folder is read, and when it cannot be. */
  readonly files: number | null
}

export function PluginsPane(): React.JSX.Element {
  const { project, close, show } = useShell()
  const { plugins, error, setEnabled, install, uninstall } = usePlugins()
  const [installing, setInstalling] = useState<PluginManifest | null>(null)
  const [uninstalling, setUninstalling] = useState<Uninstalling | null>(null)
  /* Kept with its project, so a switch does not report another project's uninstall. */
  const [done, setDone] = useState<{ projectId: string; words: string } | null>(null)
  const said = summary(plugins)
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<CapabilityFilter>('all')
  const shown = plugins ? shownCapabilities(plugins, query, filter) : []

  const askUninstall = (manifest: PluginManifest): void => {
    if (!project) return
    setUninstalling({ manifest, files: null })
    void ask(() => commands.pluginDataList(project.id, manifest.id)).then((answer) => {
      const files = answer.data?.files.length
      if (files !== undefined) setUninstalling((was) => (was?.manifest.id === manifest.id ? { ...was, files } : was))
    })
  }

  return (
    <>
      <div className="pane__bar">
        <span className="pane__ico">{CAPABILITIES_ICON(12)}</span>
        <span className="pane__t">
          <b>{t('workspace.plugins.title')}</b>
          {said ? ` · ${said}` : ''}
        </span>
        <span className="drag"></span>
        <button className="sq26" onClick={() => close('plugins')} aria-label={t('workspace.plugins.closeLabel')} title={t('workspace.plugins.closeLabel')}>
          <Icon size="s"><path d="M18 6 6 18M6 6l12 12" /></Icon>
        </button>
      </div>

      <div className="list">
        <div className="list__in capset">
          <p className="list__note">
            {t('workspace.plugins.note')}
          </p>

          {!project ? (
            <Quiet title={t('workspace.plugins.noProject')} detail={t('workspace.plugins.noProjectDetail')} />
          ) : plugins === null && error === null ? (
            /* The shape of a card while the catalogue is read, so the grid does not jump when it lands. */
            <div className="capgrid" aria-busy="true" aria-label={t('workspace.plugins.reading')}>
              <div className="capcard capcard--ghost">
                <span className="capcard__ghost"></span>
                <span className="capcard__ghost"></span>
                <span className="capcard__ghost"></span>
              </div>
            </div>
          ) : plugins === null ? (
            <Quiet title={t('workspace.plugins.unread')} detail={error} />
          ) : plugins.length === 0 ? (
            <Quiet title={t('workspace.plugins.none')} />
          ) : (
            <>
              {error && (
                <p className="capset__problem" role="alert">
                  {error}
                </p>
              )}
              {done?.projectId === project.id && (
                <p className="capset__said" role="status">
                  {done.words}
                </p>
              )}
              <div className="list__tools">
                <input
                  className="list__search"
                  type="search"
                  aria-label={t('workspace.plugins.search')}
                  placeholder={t('workspace.plugins.search')}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                />
                <div className="list__seg" role="tablist" aria-label={t('workspace.plugins.show')}>
                  {CAPABILITY_FILTERS.map((one) => (
                    <button
                      key={one.id}
                      className="segb"
                      role="tab"
                      aria-selected={filter === one.id}
                      onClick={() => setFilter(one.id)}
                    >
                      {t(one.labelKey)}
                    </button>
                  ))}
                </div>
              </div>
              {shown.length === 0 && <Quiet title={t('workspace.plugins.noMatch')} detail={t('workspace.plugins.noMatchDetail')} />}
              <div className="capgrid">
                {shown.map((plugin) => {
                  const opens = opensFor(plugin.manifest.id)
                  return (
                    <CapabilityCard
                      key={plugin.manifest.id}
                      plugin={plugin}
                      onSwitch={() => setEnabled(plugin.manifest.id, !plugin.enabled)}
                      onOpen={opens ? () => show(opens.kind, opens.tab) : null}
                      onInstall={() => setInstalling(plugin.manifest)}
                      onUninstall={() => askUninstall(plugin.manifest)}
                    />
                  )
                })}
              </div>
            </>
          )}
        </div>
      </div>

      {installing && (
        <InstallCapability
          name={installing.name}
          adds={addsInWords(installing)}
          onClose={() => setInstalling(null)}
          onConfirm={() => {
            install(installing.id)
            setDone(null)
            setInstalling(null)
          }}
        />
      )}
      {uninstalling && project && (
        <UninstallCapability
          name={uninstalling.manifest.name}
          files={uninstalling.files}
          onClose={() => setUninstalling(null)}
          onConfirm={(deleteData) => {
            const { manifest } = uninstalling
            const projectId = project.id
            setUninstalling(null)
            void uninstall(manifest.id, deleteData).then((removed) => {
              if (removed !== null) setDone({ projectId, words: uninstalledInWords(manifest.name, deleteData, removed) })
            })
          }}
        />
      )}
    </>
  )
}
