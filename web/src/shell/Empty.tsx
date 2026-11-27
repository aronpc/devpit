import { Icon } from './Icon'

/*
 * A place with nothing in it yet: its icon on a raised tile, what is missing,
 * one sentence on how it fills, and the action that fills it when there is
 * one. Every empty pane in the shell says it this way.
 */
export function Empty({
  icon,
  title,
  said,
  className,
  children,
}: {
  /** The glyph's drawing, on the 24 grid. */
  icon: React.ReactNode
  title: React.ReactNode
  said?: React.ReactNode
  className?: string
  children?: React.ReactNode
}): React.JSX.Element {
  return (
    <div className={className ? `exempty ${className}` : 'exempty'}>
      <Icon size="l">{icon}</Icon>
      <span className="exempty__t">{title}</span>
      {said && <span className="exempty__d">{said}</span>}
      {children}
    </div>
  )
}

/* The glyphs the empty panes use. */
export const TRAY = <path d="M3 13h5l1.5 3h5L16 13h5M5.5 5h13L21 13v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6Z" />
export const CLOCK = <path d="M12 7v5l3 2M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0" />
export const FOLDER = <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
export const CHECK = <path d="M20 6 9 17l-5-5" />
export const SPARK = <path d="M12 3v4M12 17v4M3 12h4M17 12h4M6 6l2.5 2.5M15.5 15.5 18 18M6 18l2.5-2.5M15.5 8.5 18 6" />
