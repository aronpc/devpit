import type { Project } from '../gen/bindings'
import { hueOf, initials } from './rail'
import { Icon, iconSize } from './Icon'

/*
 * How a project looks in the rail: the icon a person chose — one of these, or
 * an emoji — in the colour they chose, or its initials in a hue of its own
 * when nobody has chosen yet.
 *
 * An icon is stored as `icon:<name>`, so a name here can never be mistaken for
 * an emoji and an emoji can never name one of these. Shapes follow Lucide.
 */

export const ICONS: Readonly<Record<string, readonly string[]>> = {
  folder: ['M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.7-.9l-.8-1.2A2 2 0 0 0 7.9 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'],
  code: ['m16 18 6-6-6-6', 'm8 6-6 6 6 6'],
  terminal: ['m4 17 6-6-6-6', 'M12 19h8'],
  rocket: ['M4.5 16.5c-1.5 1.3-2 5-2 5s3.7-.5 5-2c.7-.8.7-2.1-.1-2.9a2.2 2.2 0 0 0-2.9-.1Z', 'm12 15-3-3a22 22 0 0 1 2-3.9A12.9 12.9 0 0 1 22 2c0 2.7-.8 7.5-6 11a22.4 22.4 0 0 1-4 2Z', 'M9 12H4s.6-3 2-4c1.6-1.1 5 0 5 0', 'M12 15v5s3-.6 4-2c1.1-1.6 0-5 0-5'],
  globe: ['M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20', 'M2 12h20', 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20'],
  box: ['M21 8a2 2 0 0 0-1-1.7l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.7l7 4a2 2 0 0 0 2 0l7-4a2 2 0 0 0 1-1.7Z', 'm3.3 7 8.7 5 8.7-5', 'M12 22V12'],
  database: ['M3 5v14a9 3 0 0 0 18 0V5', 'M3 12a9 3 0 0 0 18 0', 'M12 2a9 3 0 1 0 0 6 9 3 0 1 0 0-6'],
  server: ['M4 2h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Z', 'M4 14h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2Z', 'M6 6h.01', 'M6 18h.01'],
  cloud: ['M17.5 19H9a7 7 0 1 1 6.7-9h1.8a4.5 4.5 0 1 1 0 9Z'],
  cpu: ['M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z', 'M9 9h6v6H9z', 'M15 2v2', 'M9 2v2', 'M15 20v2', 'M9 20v2', 'M2 15h2', 'M2 9h2', 'M20 15h2', 'M20 9h2'],
  phone: ['M7 2h10a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Z', 'M12 18h.01'],
  book: ['M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20'],
  flask: ['M10 2v7.3a2 2 0 0 1-.3 1L4.3 19.3A1.8 1.8 0 0 0 5.8 22h12.4a1.8 1.8 0 0 0 1.5-2.7l-5.4-9A2 2 0 0 1 14 9.3V2', 'M8.5 2h7', 'M7 16h10'],
  zap: ['M4 14a1 1 0 0 1-.8-1.6l9.9-10.2a.5.5 0 0 1 .9.5l-1.9 6A1 1 0 0 0 13 10h7a1 1 0 0 1 .8 1.6l-9.9 10.2a.5.5 0 0 1-.9-.5l1.9-6A1 1 0 0 0 11 14z'],
  flame: ['M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.4-.5-2-1-3-1.1-2.1-.2-4 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.2.4-2.3 1-3.3.3 1.6 1.6 2.8 3.5 2.8Z'],
  leaf: ['M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.5 19 2c1 2 2 4.2 2 8 0 5.5-4.8 10-10 10Z', 'M2 21c0-3 1.9-5.4 5.1-6C9.5 14.5 12 13 13 12'],
  heart: ['M19 14c1.5-1.5 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.8 0-3 .5-4.5 2-1.5-1.5-2.7-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4 3 5.5l7 7Z'],
  star: ['M11.5 2.3a.5.5 0 0 1 1 0l2.3 4.7a2 2 0 0 0 1.5 1.1l5.2.8a.5.5 0 0 1 .3.9l-3.8 3.6a2 2 0 0 0-.6 1.9l.9 5.2a.5.5 0 0 1-.8.6l-4.6-2.5a2 2 0 0 0-1.9 0l-4.6 2.5a.5.5 0 0 1-.8-.6l.9-5.2a2 2 0 0 0-.6-1.9L1.5 9.8a.5.5 0 0 1 .3-.9l5.2-.8a2 2 0 0 0 1.5-1.1Z'],
  shield: ['M20 13c0 5-3.5 7.5-7.7 9a1 1 0 0 1-.7 0C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.2-2.7a1.2 1.2 0 0 1 1.6 0C14.5 3.8 17 5 19 5a1 1 0 0 1 1 1z'],
  briefcase: ['M16 20V4a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16', 'M4 6h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2Z'],
  bag: ['M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z', 'M3 6h18', 'M16 10a4 4 0 0 1-8 0'],
  game: ['M6 12h4', 'M8 10v4', 'M15 13h.01', 'M18 11h.01', 'M17.3 5H6.7a4 4 0 0 0-4 3.6L2 14.6A3 3 0 0 0 5 18c1 0 2-.5 2.6-1.3L9 15h6l1.4 1.7A3.4 3.4 0 0 0 19 18a3 3 0 0 0 3-3.4l-.7-6A4 4 0 0 0 17.3 5Z'],
  music: ['M9 18V5l12-2v13', 'M6 15a3 3 0 1 0 0 6 3 3 0 0 0 0-6', 'M18 13a3 3 0 1 0 0 6 3 3 0 0 0 0-6'],
  palette: ['M12 22a1 1 0 0 1 0-20 10 9 0 0 1 10 9 5 5 0 0 1-5 5h-2.2a1.8 1.8 0 0 0-1.4 2.8l.3.4a1.8 1.8 0 0 1-1.4 2.8Z', 'M13.5 6.5h.01', 'M17.5 10.5h.01', 'M6.5 12.5h.01', 'M8.5 7.5h.01'],
}

/** The swatches the dialog offers; any `#rrggbb` is accepted. */
export const COLOURS: readonly string[] = [
  '#e2795b', '#e0b36a', '#62c987', '#4fb3a9', '#5b9be2',
  '#7c6fe0', '#b46fd6', '#e06fa8', '#8a8f98', '#c9a27a',
]

export const iconName = (icon: string | null): string | null =>
  icon?.startsWith('icon:') ? icon.slice(5) : null

export function ProjectMark({
  project,
  size = 36,
}: {
  project: Pick<Project, 'id' | 'name' | 'icon' | 'color'>
  size?: number
}): React.JSX.Element {
  const named = iconName(project.icon)
  const paths = named ? ICONS[named] : undefined
  const style = {
    width: size,
    height: size,
    ...(project.color
      ? { ['--mark' as string]: project.color }
      : { ['--hue' as string]: hueOf(project.id) }),
  }
  return (
    <span className="pmark" data-chosen={project.color ? 'true' : undefined} style={style}>
      {paths ? (
        <Icon size={iconSize(size * 0.5)} aria-hidden="true">
          {paths.map((d) => (
            <path key={d} d={d} />
          ))}
        </Icon>
      ) : project.icon ? (
        <span className="pmark__emoji" style={{ fontSize: size * 0.5 }}>{project.icon}</span>
      ) : (
        initials(project.name)
      )}
    </span>
  )
}
