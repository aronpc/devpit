# Interface rules

The rules the window's stylesheets and components keep, and the test that
holds each one. They exist so that a new pane looks like it belongs without
anyone having to remember how the last one was drawn.

## Rules

**Tokens only.** A font size, a radius, a control's height, a shadow, a
duration or a curve comes from `web/src/theme/tokens.css`, never a number
written in a part. A value that has to stay literal says why on its own line,
with a comment that starts `raw:`; that comment is the whole allowlist. Held by
`web/src/shell/styles/scale.test.ts`, which fails on any other literal in
`web/src/shell/styles/*.css`. The same test refuses a `var(--…)` that
`tokens.css` does not define.

**One ruler for controls.** A control is one of four heights: `--ctl-xs` (20),
`--ctl-s` (28), `--ctl-m` (32) and `--ctl-l` (40). Each has its own radius,
`--ctl-r-xs` to `--ctl-r-l`, about 30% of the height. A control nested in
another keeps its corners concentric: the outer radius is the inner one plus
the padding between them.

**Elevation, not borders.** A surface says how high it sits with one of three
shadows. `--e1` is for a tile or card on the page, `--e2` for a card that asks
for attention, and `--e3` for anything floating (a menu, a popover, ⌘K, a
sheet, a toast). Floating surfaces also take `--top`, the light on their upper
edge. A border is for a field's edge (`--border-ctl`), not for lifting a
surface off another.

**Accent means devpit is doing something.** The accent marks work in progress,
a live row, the primary action and the keyboard focus. Amber (`--warning`)
means something waits on you. A state is a chip with an icon and a word
(`CardState`, `.stchip`), never a colour alone and never a tinted card. A
card's fill stays neutral; `stylesheet.test.ts` holds the permission, question
and draft cards to it.

**One focus ring.** Every control shows the same ring, and only for the
keyboard (`:focus-visible`). A field shows the same colour on its own edge.

**Motion enters, it does not leave.** Things arrive on `--dur-1` to `--dur-4`
with `--ease-out` (or `--ease-panel` for a panel that moves the layout), and
leave at once. With reduced motion asked for, nothing animates. Held by
`scale.test.ts`.

**Icons are drawn by `Icon`.** Every icon in the shell goes through
`web/src/shell/Icon.tsx`. It draws on the 24 grid in `currentColor`, with one
stroke width and round ends, at `--icon-s`, `--icon-m` or `--icon-l`. The
agent marks and the charts are the only exceptions. Held by `icon.test.tsx`.

**One pattern per kind of thing.** An empty pane is `Empty`: an icon tile, a
title, one sentence and its actions. A passing word is a `Toast`, in the one
stack at the foot of the window. Every card in a turn is `--chat-card-w` wide.

**AA contrast everywhere.** Every text token reaches 4.5:1 on every surface it
is drawn on, in dark, light and more contrast, including on a hover, a
selected row and a chip's tint. Held by `web/src/theme/contrast.test.ts`. That
test paints translucent layers over their surface in order, the way the window
does.

## Seeing it

`make shots` draws a made-up board on its own display and keeps the
screenshots under `target/launch-assets/`. The nightly end-to-end run keeps
the shell's set as a build artifact, so one night can be compared with the
next by eye.
