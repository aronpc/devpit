import { useState } from 'react'

import { ignoringWhitespace, parse, sides, type Hunk, type Row } from './diff'
import { DiffText as Text } from './DiffText'
import { useViewPrefs } from './viewPrefs'
import { t } from '../i18n'

/*
 * A patch as git printed it, hunk by hunk.
 *
 * What a commit's diff is drawn with: it spans many files and has no single
 * file on disk to line up against, so the hunks are the whole of it.
 */

/* One hunk at a time past this many: a file with four thousand changed lines
   should not decide how long the window is frozen. */
const HUNKS_AT_ONCE = 40

export function PatchView({
  raw,
  split,
  spaces,
  empty,
}: {
  raw: string
  split: boolean
  spaces: boolean
  /** What to say when the patch has no hunks. */
  empty: string
}): React.JSX.Element {
  const [shown, setShown] = useState(HUNKS_AT_ONCE)
  const ignore = useViewPrefs().diffWhitespace === 'ignore'
  const files = parse(raw)
  const hunks = files.flatMap((file) => (ignore ? ignoringWhitespace(file.hunks) : file.hunks))
  const renamed = files.find((file) => file.from)
  const binary = files.some((file) => file.binary)

  return (
    <>
      {renamed && (
        <div className="diff__moved">
          {t('workspace.patch.renamed')} <code>{renamed.from}</code>
        </div>
      )}
      {binary && <div className="exempty__t">{t('workspace.patch.binary')}</div>}
      {!binary && hunks.length === 0 && <div className="exempty__t">{ignore && files.some((file) => file.hunks.length > 0) ? t('workspace.patch.whitespaceOnly') : empty}</div>}

      {hunks.slice(0, shown).map((hunk, at) =>
        split ? <Split key={at} hunk={hunk} spaces={spaces} /> : <Unified key={at} hunk={hunk} spaces={spaces} />,
      )}

      {hunks.length > shown && (
        <button className="btn" onClick={() => setShown((was) => was + HUNKS_AT_ONCE)}>
          {t('workspace.patch.more', { count: Math.min(HUNKS_AT_ONCE, hunks.length - shown), total: hunks.length })}
        </button>
      )}
    </>
  )
}

function Unified({ hunk, spaces }: { hunk: Hunk; spaces: boolean }): React.JSX.Element {
  return (
    <div className="diff">
      <div className="diff__at">{hunk.header}</div>
      {hunk.rows.map((row, at) => (
        <div className="diff__l" data-d={row.kind} key={at}>
          <Text text={row.text} spaces={spaces} />
        </div>
      ))}
    </div>
  )
}

function Split({ hunk, spaces }: { hunk: Hunk; spaces: boolean }): React.JSX.Element {
  const { left, right } = sides(hunk)
  return (
    <div className="diff">
      <div className="diff__at">{hunk.header}</div>
      <div className="diff__two">
        <div>
          {left.map((row, at) => (
            <Half key={at} row={row} spaces={spaces} />
          ))}
        </div>
        <div>
          {right.map((row, at) => (
            <Half key={at} row={row} spaces={spaces} />
          ))}
        </div>
      </div>
    </div>
  )
}

const Half = ({ row, spaces }: { row: Row | null; spaces: boolean }): React.JSX.Element => (
  <div className="diff__l" data-d={row?.kind ?? 'gap'}>
    <Text text={row?.text ?? ''} spaces={spaces} />
  </div>
)
