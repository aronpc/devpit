import type { Slash } from './useSlash'
import { intoView } from './intoView'
import { t } from '../i18n'

/* The commands matching what was typed after `/`, above the field. */
export function SlashMenu({ slash }: { slash: Slash }): React.JSX.Element | null {
  if (slash.items.length === 0) return null
  return (
    <div className="slash" role="listbox" aria-label={t('sessions.slash.label')}>
      {slash.items.map((command, index) => (
        <button
          key={command}
          className="slash__i"
          role="option"
          aria-selected={index === slash.at}
          ref={index === slash.at ? intoView : undefined}
          // Down, not click: a click moves focus out of the field first.
          onMouseDown={(event) => {
            event.preventDefault()
            slash.choose(command)
          }}
        >
          /{command}
        </button>
      ))}
    </div>
  )
}
