import { useEffect, useRef, useState } from 'react'
import { t } from '../i18n'
import { useKeys, useShortcut } from './useShortcut'
import { useShell } from './useShell'
import { abandoned } from './typing'

const Search = (): React.JSX.Element => (
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.3-4.3" />
  </svg>
)

const Tick = (): React.JSX.Element => (
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
    <path d="M20 6 9 17l-5-5" />
  </svg>
)

/*
 * At rest this used to be bare text, which reads as the window's title — the
 * one thing a control must not look like. It carries its own surface, and the
 * double chevron is the glyph that means "switches between values" rather than
 * "expands downwards".
 *
 * Each row says what is still running in that project, because switching does
 * not stop it. No other project switcher has to carry that; this one does.
 */
export function ProjectPicker({ onAdd }: { onAdd: () => void }): React.JSX.Element {
  const { project, setProject, projects, projectsError } = useShell()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const field = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (open) field.current?.focus()
  }, [open])

  /* ⌘P is heard open or shut, and here rather than in the shell, which would
     have to hold this menu's state. Escape only matters while it is open. */
  useShortcut('project', () => {
    setQuery('')
    setOpen((was) => !was)
  })
  const keys = useKeys()('project')
  useEffect(() => {
    const shut = (): void => setOpen(false)
    const key = (event: KeyboardEvent): void => {
      if (open && abandoned(event)) setOpen(false)
    }
    if (open) document.addEventListener('click', shut)
    document.addEventListener('keydown', key)
    return () => {
      document.removeEventListener('click', shut)
      document.removeEventListener('keydown', key)
    }
  }, [open])

  const listed = projects.filter(
    (row) => !row.orchestrator /* the rail's own group */ && `${row.name} ${row.rootPath}`.toLowerCase().includes(query.trim().toLowerCase()),
  )

  return (
    <>
      <button
        className="pick"
        aria-haspopup="true"
        aria-expanded={open}
        title={t('workspace.projectPicker.switch', { keys })}
        onClick={(event) => {
          event.stopPropagation()
          setQuery('')
          setOpen((was) => !was)
        }}
      >
        <span className="pick__name">{project?.name ?? t('workspace.projectPicker.noProject')}</span>
        <span className="pick__chev">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="m8 9 4-4 4 4M8 15l4 4 4-4" />
          </svg>
        </span>
      </button>

      {open && (
        <div className="pickpop" role="menu" onClick={(event) => event.stopPropagation()}>
          <div className="pickpop__q">
            <Search />
            <input
              ref={field}
              className="pickpop__in"
              type="text"
              autoComplete="off"
              spellCheck={false}
              placeholder={t('workspace.projectPicker.find')}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>

          {listed.length > 0 && <div className="pickpop__g">{t('workspace.projectPicker.projects')}</div>}
          {listed.map((row) => (
            <button
              key={row.id}
              className="prow"
              onClick={() => {
                setProject(row.id)
                setOpen(false)
              }}
            >
              <span className="prow__tick" data-on={String(row.id === project?.id)}>
                <Tick />
              </span>
              <span className="prow__b">
                <span className="prow__n">{row.name}</span>
                <span className="prow__p">{row.unreadable ?? row.rootPath}</span>
              </span>
              {/* A repository that cannot be read stays listed and says so;
                  dropping it out would read as never having been added. */}
              <span className="prow__live" data-live={row.unreadable ? 1 : row.worktrees.length}>
                <span className="prow__dot" />
                {row.unreadable ? t('workspace.projectPicker.unreadable') : t('workspace.projects.worktrees', { count: row.worktrees.length })}
              </span>
            </button>
          ))}
          {projectsError && <div className="pickpop__none">{projectsError}</div>}
          {!projectsError && listed.length === 0 && (
            <div className="pickpop__none">{t('workspace.projectPicker.noMatch')}</div>
          )}

          <div className="pickpop__sep" />
          <button
            className="newmenu__item"
            role="menuitem"
            onClick={() => {
              setOpen(false)
              onAdd()
            }}
          >
            <span className="newmenu__ico">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 5v14M5 12h14" />
              </svg>
            </span>
            <span className="newmenu__label">{t('workspace.projectPicker.add')}</span>
          </button>
        </div>
      )}
    </>
  )
}
