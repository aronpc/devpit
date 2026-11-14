/*
 * A mark for each agent.
 *
 * Drawn and not fetched. A desktop app that needs the network to draw its own
 * settings pane looks broken on a plane, and the favicon services every other
 * product reaches for are a third party watching which agents somebody has
 * installed.
 *
 * They are devpit's marks, not the vendors' logos — geometric, distinct at
 * thirteen pixels, and making no claim to be anybody's brand. What they have
 * to do is let the eye find a row again, which is all an icon in a list does.
 */

const MARKS: Readonly<Record<string, React.JSX.Element>> = {
  // A burst, for the one whose own mark is a burst.
  claude: <path d="M12 3v18M3 12h18M6 6l12 12M18 6 6 18" />,
  // A ring with a gap, for a model that speaks in turns.
  codex: <path d="M12 4a8 8 0 1 1-5.6 2.3" />,
  // Two strokes converging: a generation.
  gemini: <path d="M12 3c0 5-4 9-9 9 5 0 9 4 9 9 0-5 4-9 9-9-5 0-9-4-9-9Z" />,
  // A bracket pair: source, opened.
  opencode: <path d="M9 5 4 12l5 7M15 5l5 7-5 7" />,
  // A caret, the shape of the editor it came from.
  cursor: <path d="M5 3v18l5-5h8L5 3Z" />,
  // A wing.
  copilot: <path d="M4 14c0-4 3-7 8-7s8 3 8 7-4 5-8 5-8-1-8-5Z M8 13v2M16 13v2" />,
  // An amplitude.
  amp: <path d="M3 12h3l3-7 3 14 3-9 3 4h3" />,
  // A bolt, for the one named after a machine.
  droid: <path d="M13 3 5 14h6l-2 7 9-12h-6l1-6Z" />,
  // A crossing.
  grok: <path d="m5 5 14 14M19 5 5 19" />,
  // An arc lifting off.
  agy: <path d="M4 19c5-2 9-7 11-14M15 5l-4 2M15 5l1 4" />,
  // A pair of arrows, the shape of a pair programmer.
  aider: <path d="M4 9h13l-3-3M20 15H7l3 3" />,
  // A bird's head in three strokes.
  goose: <path d="M7 20V9a5 5 0 0 1 10 0v2M17 11h4M9 6h.01" />,
  // Waves.
  crush: <path d="M3 8c3-3 6 3 9 0s6-3 9 0M3 16c3-3 6 3 9 0s6-3 9 0" />,
}

/** A profile is somebody's own, so it takes the mark of what it runs. */
const FALLBACK = <path d="M4 7h16M4 12h10M4 17h7" />

export function AgentGlyph({
  agent,
  base,
}: {
  agent: string
  /** For a profile: the agent it behaves like, whose mark it borrows. */
  base?: string
}): React.JSX.Element {
  return (
    <svg
      className="agmark"
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {MARKS[agent] ?? (base ? MARKS[base] : undefined) ?? FALLBACK}
    </svg>
  )
}

/** Only the test asks: which agents this build has a mark of its own for. */
export const MARKED: readonly string[] = Object.keys(MARKS)
