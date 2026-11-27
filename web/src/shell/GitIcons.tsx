import { Icon, iconSize } from './Icon'


/*
 * The icons of the Changes panel and the diff, drawn here rather than
 * installed: a dozen paths are not worth a dependency, and every other icon in
 * the app is inline SVG already. Shapes follow Lucide, which is what Orca
 * draws, so a hand used to one finds the same marks in the other.
 */

type Paths = readonly string[]

function icon(paths: Paths, extra?: React.ReactNode) {
  return function GitIcon({ size = 14 }: { size?: number }): React.JSX.Element {
    return (
      <Icon size={iconSize(size)} aria-hidden="true">
        {extra}
        {paths.map((d) => (
          <path key={d} d={d} />
        ))}
      </Icon>
    )
  }
}

export const Plus = icon(['M5 12h14', 'M12 5v14'])
export const Minus = icon(['M5 12h14'])
export const Undo = icon(['M9 14 4 9l5-5', 'M4 9h10.5a5.5 5.5 0 0 1 0 11H11'])
export const Trash = icon([
  'M3 6h18',
  'M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6',
  'M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2',
])
export const ChevronDown = icon(['m6 9 6 6 6-6'])
export const ChevronUp = icon(['m18 15-6-6-6 6'])
export const Columns = icon(['M12 3v18'], <rect x="3" y="3" width="18" height="18" rx="2" />)
export const Rows = icon(['M3 12h18'], <rect x="3" y="3" width="18" height="18" rx="2" />)
export const Wrap = icon(['M3 6h18', 'M3 12h15a3 3 0 1 1 0 6h-4', 'm16 16-2 2 2 2', 'M3 18h7'])
export const Fold = icon(['M12 22v-6', 'M12 8V2', 'm15 19-3-3-3 3', 'm15 5-3 3-3-3', 'M4 12h16'])
export const Tree = icon([
  'M21 12h-8',
  'M21 6H8',
  'M21 18h-8',
  'M3 6v4c0 1.1.9 2 2 2h3',
  'M3 10v6c0 1.1.9 2 2 2h3',
])
export const List = icon(['M8 6h13', 'M8 12h13', 'M8 18h13', 'M3 6h.01', 'M3 12h.01', 'M3 18h.01'])
export const Search = icon(['m21 21-4.3-4.3'], <circle cx="11" cy="11" r="8" />)
export const Copy = icon(
  ['M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2'],
  <rect x="8" y="8" width="14" height="14" rx="2" />,
)
export const Clipboard = icon(
  ['M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2'],
  <rect x="8" y="2" width="8" height="4" rx="1" />,
)
export const SelectAll = icon(['M5 3a2 2 0 0 0-2 2', 'M19 3a2 2 0 0 1 2 2', 'M21 19a2 2 0 0 1-2 2', 'M5 21a2 2 0 0 1-2-2', 'M9 3h1', 'M9 21h1', 'M14 3h1', 'M14 21h1', 'M3 9v1', 'M21 9v1', 'M3 14v1', 'M21 14v1', 'M8 12h8'])
export const Close = icon(['M18 6 6 18', 'm6 6 12 12'])
export const Bookmark = icon(['m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z'])
export const Pencil = icon(['M21.2 6.8a2.8 2.8 0 0 0-4-4L3.8 16.2 3 21l4.8-.8Z', 'm15 5 4 4'])
export const Folder = icon(['M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.7-.9l-.8-1.2A2 2 0 0 0 7.9 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'])
