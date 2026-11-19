import type { MentionMenu as Menu } from './useMention'
import { intoView } from './intoView'
import { t } from '../i18n'

/* The project's files matching what was typed after `@`, above the field —
   the slash menu's place and look, since only one is ever open. */
export function MentionMenu({ menu }: { menu: Menu }): React.JSX.Element | null {
  if (menu.items.length === 0) return null
  return (
    <div className="slash" role="listbox" aria-label={t('sessions.mention.label')}>
      {menu.items.map((path, index) => {
        const cut = path.lastIndexOf('/') + 1
        return (
          <button
            key={path}
            className="slash__i mention__i"
            role="option"
            aria-selected={index === menu.at}
            ref={index === menu.at ? intoView : undefined}
            onMouseDown={(event) => {
              event.preventDefault()
              menu.choose(path)
            }}
          >
            <span className="mention__n">{path.slice(cut)}</span>
            {cut > 0 && <span className="mention__d">{path.slice(0, cut - 1)}</span>}
          </button>
        )
      })}
    </div>
  )
}
