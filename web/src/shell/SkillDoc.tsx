import { useEffect, useState } from 'react'

import type { Skill } from '../gen/bindings'
import { ask, commands } from './live'
import { Markdown } from './MarkdownView'
import { useCopied } from './useCopied'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * One skill, as written.
 *
 * The column used to hold a single sentence and three buttons, while the
 * stylesheet carried rules for the headings, paragraphs and lists of a
 * rendered `SKILL.md` — written for a body that was never fetched. A skill is
 * instructions; a panel about skills that will not show them is a table of
 * contents with no book behind it.
 */

export function SkillDoc({
  skill,
  directory,
}: {
  skill: Skill
  /** The installation it was listed from; a name can exist in two. */
  directory: string | null
}): React.JSX.Element {
  const [body, setBody] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, copyText] = useCopied()

  useEffect(() => {
    /* Two reads can be in flight when the reader moves down the list, and
       nothing says the first answers first — without this, one skill's name
       sits over another's instructions. */
    let current = true
    setBody(null)
    setError(null)
    void ask(() => commands.skillsRead(skill.name, directory)).then((answer) => {
      if (!current) return
      setBody(answer.data?.body ?? null)
      setError(answer.error)
    })
    return () => {
      current = false
    }
  }, [skill.name, directory])

  const copy = (): void => copyText(skill.path)

  return (
    <>
      <div className="sk__top">
        <span className="sk__mark">
          <Icon size="s"><path d="M12 3v3M5.6 5.6l2.1 2.1M3 12h3M18 12h3M16.3 7.7l2.1-2.1" /><rect x="7" y="12" width="10" height="9" rx="2" /></Icon>
        </span>
        <div>
          <div className="sk__name">{skill.name}</div>
          <div className="sk__from">{skill.source}</div>
        </div>
      </div>

      <p className="sk__what">{skill.description}</p>

      <div className="sk__acts">
        <button className="btn" onClick={() => void ask(() => commands.pathOpen(skill.path))}>
          {t('workspace.skillDoc.open')}
        </button>
        <button className="btn" onClick={() => void ask(() => commands.pathReveal(skill.path))}>
          {t('workspace.skillDoc.reveal')}
        </button>
        <button className="btn" onClick={copy}>
          {copied ? t('workspace.skillDoc.copied') : t('workspace.skillDoc.copy')}
        </button>
      </div>

      {error && <p className="sk__what">{error}</p>}
      {/* A link beside a SKILL.md is a file outside every project, so it is
          handed to the desktop rather than opened as a tab — a tab would
          resolve it against the project and refuse. */}
      {body !== null && (
        <Markdown
          source={body}
          path={skill.path}
          opens={(path) => void ask(() => commands.pathOpen(path))}
        />
      )}
      <div className="sk__file">{skill.path}</div>
    </>
  )
}
