import { ofPath } from './languages'
import { Icon } from './Icon'

/*
 * What kind of file a row is about.
 *
 * Keyed off `languages.ts`, which already knows a hundred and fifty spellings
 * for the purpose of colouring them — one table, two readers, and a new
 * extension arrives in both at once.
 *
 * Shapes and not colours: the colour on these rows means the git status, and
 * a second colour saying something else on the same glyph would be two
 * messages in one mark.
 */

type Glyph = 'code' | 'markup' | 'style' | 'data' | 'config' | 'doc' | 'image'

/** Which shape stands for each language, by the id `languages.ts` gives it. */
const SHAPES: Readonly<Record<string, Glyph>> = {
  html: 'markup',
  css: 'style',
  json: 'data',
  graphql: 'data',
  protobuf: 'data',
  sql: 'data',
  toml: 'config',
  yaml: 'config',
  ini: 'config',
  nix: 'config',
  terraform: 'config',
  make: 'config',
  docker: 'config',
}

/** Files a language table has nothing to say about, but a reader does. */
const BY_EXTENSION: Readonly<Record<string, Glyph>> = {
  md: 'doc',
  markdown: 'doc',
  mdx: 'doc',
  txt: 'doc',
  rst: 'doc',
  adoc: 'doc',
  png: 'image',
  jpg: 'image',
  jpeg: 'image',
  gif: 'image',
  webp: 'image',
  avif: 'image',
  ico: 'image',
  bmp: 'image',
  pdf: 'doc',
  woff: 'image',
  woff2: 'image',
  ttf: 'image',
  otf: 'image',
}

function shapeOf(path: string): Glyph {
  const extension = (path.split('/').pop() ?? '').split('.').pop()?.toLowerCase() ?? ''
  const known = BY_EXTENSION[extension]
  if (known) return known
  const language = ofPath(path)
  return language ? (SHAPES[language.id] ?? 'code') : 'doc'
}

const PATHS: Readonly<Record<Glyph, React.JSX.Element>> = {
  code: <path d="m8 6-5 6 5 6M16 6l5 6-5 6" />,
  markup: <path d="m9 7-4 5 4 5M15 7l4 5-4 5M13 4l-2 16" />,
  style: <path d="M12 3s6 5.7 6 9.4A6 6 0 0 1 6 12.4C6 8.7 12 3 12 3Z" />,
  data: <path d="M8 4H7a2 2 0 0 0-2 2v3a2 2 0 0 1-2 2 2 2 0 0 1 2 2v3a2 2 0 0 0 2 2h1M16 4h1a2 2 0 0 1 2 2v3a2 2 0 0 0 2 2 2 2 0 0 0-2 2v3a2 2 0 0 1-2 2h-1" />,
  config: <path d="M4 7h10M18 7h2M4 17h4M12 17h8M16 4v6M8 14v6" />,
  doc: <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z M14 3v5h5" />,
  image: <path d="M4 5h16v14H4Z M8.5 10.5a1 1 0 1 0 0-.1 M20 16l-5-5-9 8" />,
}

/** The mark for this path, taking its colour from whoever draws it. */
export function FileGlyph({ path }: { path: string }): React.JSX.Element {
  return (
    <Icon size="s" className="gitrow__ico" aria-hidden="true">
      {PATHS[shapeOf(path)]}
    </Icon>
  )
}

/** Only the test asks — the component draws itself. */
export const shapeFor = shapeOf
