import { wantLine } from './revealLine'
import type { SearchFile } from '../gen/bindings'
import { looksLikeDotfile, type FindFlags, type FindHit } from './find'
import { formatNumber } from '../i18n/format'
import { t } from '../i18n'

/* A one-character query against a 40,000-file index can match nearly all of
   it — enough DOM nodes, built synchronously on a keystroke, to stall a
   300px-wide panel. Capped the same way `partial` is: the true count is
   always shown, just not always rendered. */
const MOST_SHOWN = 300

/* One row per hit from the flat index. Small on purpose — ranking, previews
   and the rest of what a real search panel does belong to a later story. */
export function SearchResults({
  query,
  hits,
  reason,
  partial,
  onOpen,
}: {
  query: string
  hits: readonly FindHit[]
  reason: string | null
  partial: boolean
  onOpen: (path: string) => void
}): React.JSX.Element {
  if (reason) {
    return (
      <div className="exempty">
        <span className="exempty__t">{reason}</span>
      </div>
    )
  }

  if (hits.length === 0) {
    return (
      <div className="exempty">
        <span className="exempty__t">{t('workspace.search.nothing')}</span>
        {/* The index drops every dotfile but `.github`; the tree does not,
            so this is where that gap would otherwise read as "not there". */}
        {looksLikeDotfile(query) && (
          <span className="exempty__d">{t('workspace.search.dotfiles')}</span>
        )}
        {partial && <span className="exempty__d">{t('workspace.search.tooLarge')}</span>}
      </div>
    )
  }

  const shown = hits.slice(0, MOST_SHOWN)
  const hidden = hits.length - shown.length

  return (
    <div className="results">
      {shown.map((hit) => (
        <button key={hit.path} className="result" onClick={() => onOpen(hit.path)}>
          <span className="result__n">
            {hit.at > 0 ? hit.path.slice(0, hit.at) : null}
            <mark className="result__hit">{hit.path.slice(hit.at, hit.at + hit.length)}</mark>
            {hit.path.slice(hit.at + hit.length)}
          </span>
        </button>
      ))}
      {hidden > 0 && (
        <div className="results__partial">
          {t('workspace.search.showing', { shown: formatNumber(shown.length), total: formatNumber(hits.length) })}
        </div>
      )}
      {partial && <div className="results__partial">{t('workspace.search.tooLarge')}</div>}
    </div>
  )
}

/* `find.ts` builds the same pattern for path highlighting, but it is owned by
   another story this round and works over paths, not arbitrary line text —
   duplicated here rather than reached into. */
const escape = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/* The query as one pattern, built once for the whole list rather than for
   every line drawn. Null when there is nothing to mark, or it does not parse. */
function patternOf(query: string, flags: FindFlags): RegExp | null {
  const needle = query.trim()
  if (!needle) return null
  const body = flags.regex ? needle : escape(needle)
  const withWord = flags.word ? `\\b(?:${body})\\b` : body
  try {
    return new RegExp(withWord, flags.case ? '' : 'i')
  } catch {
    return null
  }
}

function mark(text: string, pattern: RegExp | null): React.ReactNode {
  if (!pattern) return text
  const found = pattern.exec(text)
  if (!found) return text
  const at = found.index
  const length = found[0].length
  return (
    <>
      {text.slice(0, at)}
      <mark className="result__hit">{text.slice(at, at + length)}</mark>
      {text.slice(at + length)}
    </>
  )
}

/* Content search: matches grouped by file, one row per line under each —
   the shape `project.search` already grouped them in, so this does not
   re-sort a flat list. Same cap and the same "showing N of M" wording as
   `SearchResults`, because a content match runs into the thousands just as
   easily as a path does. */
export function ContentResults({
  query,
  flags,
  files,
  matched,
  truncated,
  reason,
  onOpen,
}: {
  query: string
  flags: FindFlags
  files: readonly SearchFile[]
  matched: number
  truncated: boolean
  reason: string | null
  onOpen: (path: string) => void
}): React.JSX.Element {
  if (reason) {
    return (
      <div className="exempty">
        <span className="exempty__t">{reason}</span>
      </div>
    )
  }

  if (files.length === 0) {
    return (
      <div className="exempty">
        <span className="exempty__t">{t('workspace.search.noMatches')}</span>
        {/* `git grep --untracked` never reaches a gitignored file, and the
            tree draws those regardless — a permanent property of this
            command, not something that varies call to call, so it is stated
            here rather than carried as a field that is always true. */}
        <span className="exempty__d">{t('workspace.search.gitignored')}</span>
      </div>
    )
  }

  const included = files.reduce((total, file) => total + file.lines.length, 0)
  const pattern = patternOf(query, flags)

  return (
    <div className="results">
      {files.map((file) => (
        <div key={file.path}>
          <button className="cresult__file" onClick={() => onOpen(file.path)}>
            {file.path}
          </button>
          {file.lines.map((hit) => (
            <button
              key={hit.line}
              className="cresult__line"
              onClick={() => {
                onOpen(file.path)
                wantLine(file.path, hit.line)
              }}
            >
              <span className="cresult__n">{hit.line}</span>
              <span className="cresult__t">{mark(hit.text, pattern)}</span>
            </button>
          ))}
        </div>
      ))}
      {truncated && (
        <div className="results__partial">
          {t('workspace.search.showing', { shown: formatNumber(included), total: formatNumber(matched) })}
        </div>
      )}
    </div>
  )
}
