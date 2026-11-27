import { useEffect, useMemo, useState } from 'react'

import type { Mode } from './explorer'
import { filter, type FindFlags } from './find'
import { ask, commands } from './live'
import { ContentResults, SearchResults } from './SearchResults'
import { Skeleton } from './Skeleton'
import { Tree } from './TreeView'
import type { UseFileIndex } from './useFileIndex'
import type { UseTree } from './useTree'
import type { Project, SearchHits } from '../gen/bindings'
import { t } from '../i18n'
import { Icon, iconSize } from './Icon'

const Line = ({ d, size = 15 }: { d: string; size?: number }): React.JSX.Element => (
  <Icon size={iconSize(size)}>
    <path d={d} />
  </Icon>
)

const COLLAPSE = 'M4 7h16M4 12h16M4 17h16'
const REFRESH = 'M21 12a9 9 0 0 1-15.5 6.2M3 12a9 9 0 0 1 15.5-6.2M3 20v-5h5M21 4v5h-5'
const SEARCH = 'm21 21-4.3-4.3'

/* The tree, name search and content search: three answers to "what files",
   picked by mode rather than shown at once. Split out of `RightPanel`,
   which owns the tab bar and the project, not this view's own search
   state. `view`, `mode` and `query` still live one level up — this one
   only reads and sets `mode`/`query`, the same way `tree` and `index` are
   handed down rather than fetched twice. */
export function Explorer({
  project,
  tree,
  index,
  mode,
  setMode,
  query,
  setQuery,
  current,
  onOpen,
}: {
  project: Project | null
  tree: UseTree
  index: UseFileIndex
  mode: Mode
  setMode: (mode: Mode) => void
  query: string
  setQuery: (query: string) => void
  current: string | null
  onOpen: (path: string) => void
}): React.JSX.Element {
  const [flags, setFlags] = useState<FindFlags>({ case: false, word: false, regex: false })
  const [collapsed, setCollapsed] = useState(0)

  /* Empty query: the tree, exactly as before. A query in Names mode: the
     flat index reaches paths the tree has never loaded. */
  const searching = mode === 'names' && query.trim().length > 0
  const found = useMemo(
    () => (index.paths ? filter(index.paths, query, flags) : { hits: [], reason: null }),
    [index.paths, query, flags],
  )

  /* Keyed on the state that needs the index, not the keystroke that usually
     precedes it — a project switch with a query already in the field needs
     this exact fetch too, and a keystroke-only trigger would never see it. */
  useEffect(() => {
    if (searching && index.paths === null) index.ensure()
  }, [searching, index.paths, index.ensure])

  /* Contents mode: no flat index to filter client-side, so every keystroke
     is a `git grep`. Debounced so a fast typist starts one process, not one
     per letter. */
  const contentSearching = mode === 'contents' && query.trim().length > 0
  const [contentHits, setContentHits] = useState<SearchHits | null>(null)
  const [contentReason, setContentReason] = useState<string | null>(null)
  const [contentLoading, setContentLoading] = useState(false)

  useEffect(() => {
    if (!contentSearching || !project) {
      setContentHits(null)
      setContentReason(null)
      return
    }
    let live = true
    setContentLoading(true)
    const timer = window.setTimeout(() => {
      void ask(() => commands.projectSearch(project.id, null, query, flags.case, flags.word, flags.regex)).then(
        (asked) => {
          if (!live) return
          setContentHits(asked.data)
          setContentReason(asked.error)
          setContentLoading(false)
        },
      )
    }, 200)
    return () => {
      live = false
      window.clearTimeout(timer)
    }
  }, [contentSearching, project, query, flags])

  // Loading with nothing shown yet is ambiguous; stale rows just stay put.
  const treeSkeleton = tree.loading && tree.nodes.length === 0

  return (
    <>
      <div className="ex__head">
        <span className="ex__proj">{project?.name ?? t('workspace.explorer.noProject')}</span>
        <button className="sq26 tip" data-tip={t('workspace.explorer.collapseAll')} aria-label={t('workspace.explorer.collapseAll')} onClick={() => setCollapsed((was) => was + 1)}>
          <Line d={COLLAPSE} />
        </button>
        <button className="sq26 tip" data-tip={t('workspace.explorer.refresh')} aria-label={t('workspace.explorer.refresh')} onClick={tree.reload}>
          <Line d={REFRESH} />
        </button>
      </div>

      <div className="ex__search">
        <Icon size="s">
          <circle cx="11" cy="11" r="8" />
          <path d={SEARCH} />
        </Icon>
        <input
          className="ex__q"
          placeholder={mode === 'contents' ? t('workspace.explorer.searchInFiles') : t('workspace.explorer.search')}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <span className="ex__flags">
          <button className="flag" aria-pressed={flags.case} title={t('workspace.explorer.matchCase')} onClick={() => setFlags((f) => ({ ...f, case: !f.case }))}>{t('workspace.explorer.matchCaseFlag')}</button>
          <button className="flag" aria-pressed={flags.word} title={t('workspace.explorer.wholeWord')} onClick={() => setFlags((f) => ({ ...f, word: !f.word }))}>{t('workspace.explorer.wholeWordFlag')}</button>
          <button className="flag" aria-pressed={flags.regex} title={t('workspace.explorer.regex')} onClick={() => setFlags((f) => ({ ...f, regex: !f.regex }))}>.*</button>
        </span>
      </div>

      <div className="ex__modes">
        <button className="exmode__tab" data-ex="names" aria-selected={mode === 'names'} onClick={() => setMode('names')}>{t('workspace.explorer.names')}</button>
        <button className="exmode__tab" data-ex="contents" aria-selected={mode === 'contents'} onClick={() => setMode('contents')}>{t('workspace.explorer.contents')}</button>
      </div>

      {tree.error && <div className="exempty"><span className="exempty__t">{tree.error}</span></div>}
      {!tree.error && mode === 'names' && !searching && treeSkeleton && <Skeleton />}
      {!tree.error && mode === 'names' && !searching && !treeSkeleton && (
        <Tree projectId={project?.id ?? ''} nodes={tree.nodes} version={tree.version} query={query} current={current} collapsed={collapsed} onOpen={onOpen} />
      )}
      {!tree.error && mode === 'names' && searching && index.loading && (
        <div className="exempty"><span className="exempty__t">{t('workspace.explorer.indexing')}</span></div>
      )}
      {!tree.error && mode === 'names' && searching && !index.loading && (
        <SearchResults query={query} hits={found.hits} reason={found.reason} partial={index.partial} onOpen={onOpen} />
      )}
      {!tree.error && mode === 'contents' && !contentSearching && (
        <div className="exempty">
          <span className="exempty__t">{t('workspace.explorer.typeToSearch')}</span>
        </div>
      )}
      {!tree.error && mode === 'contents' && contentSearching && contentLoading && (
        <div className="exempty"><span className="exempty__t">{t('workspace.explorer.searching')}</span></div>
      )}
      {!tree.error && mode === 'contents' && contentSearching && !contentLoading && (
        <ContentResults
          query={query}
          flags={flags}
          files={contentHits?.files ?? []}
          matched={contentHits?.matched ?? 0}
          truncated={contentHits?.truncated ?? false}
          reason={contentReason}
          onOpen={onOpen}
        />
      )}
    </>
  )
}
