/*
 * Every icon in the shell is drawn here: on the 24 grid, in the text colour,
 * with one stroke width and round ends, at one of three sizes. The drawing is
 * the caller's children; the size and the line are not the caller's to pick.
 */

export type IconSize = 's' | 'm' | 'l'

export const ICON_STROKE = 2

const PX: Record<IconSize, number> = { s: 14, m: 16, l: 20 }

/** The size a drawing made at `px` pixels lands on. */
export function iconSize(px: number): IconSize {
  return px <= 14 ? 's' : px <= 17 ? 'm' : 'l'
}

type Props = Omit<React.SVGProps<SVGSVGElement>, 'width' | 'height' | 'viewBox' | 'fill' | 'stroke' | 'strokeWidth' | 'ref'> & {
  size?: IconSize
  /** A solid shape rather than a line. */
  filled?: boolean
}

export function Icon({ size = 's', filled = false, className, children, ...rest }: Props): React.JSX.Element {
  return (
    <svg
      className={className ? `icon icon--${size} ${className}` : `icon icon--${size}`}
      width={PX[size]}
      height={PX[size]}
      viewBox="0 0 24 24"
      fill={filled ? 'currentColor' : 'none'}
      stroke={filled ? 'none' : 'currentColor'}
      strokeWidth={filled ? undefined : ICON_STROKE}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...rest}
    >
      {children}
    </svg>
  )
}
