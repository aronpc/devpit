import { Icon } from './Icon'

/* The icons the search field uses for what is not a pane. Panes bring their
   own from `paneList`. */

export const GEAR = (
  <Icon size="s">
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82 2 2 0 1 1-2.83 2.83 1.65 1.65 0 0 0-2.82 1.18 2 2 0 1 1-4 0A1.65 1.65 0 0 0 7.26 19.4a2 2 0 1 1-2.83-2.83A1.65 1.65 0 0 0 3.09 13a2 2 0 1 1 0-4A1.65 1.65 0 0 0 4.6 7.26a2 2 0 1 1 2.83-2.83A1.65 1.65 0 0 0 10 3.09a2 2 0 1 1 4 0 1.65 1.65 0 0 0 2.74 1.51 2 2 0 1 1 2.83 2.83A1.65 1.65 0 0 0 20.91 11a2 2 0 1 1 0 4 1.65 1.65 0 0 0-1.51 1Z" />
  </Icon>
)

export const DOC = (
  <Icon size="s">
    <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
    <path d="M14 2v5h5" />
  </Icon>
)

export const CARD = (
  <Icon size="s">
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <path d="M9 3v18" />
  </Icon>
)

export const TALK = (
  <Icon size="s">
    <path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9.9 9.9 0 0 1-4.2-.9L3 20.5l1.6-4.4A8.4 8.4 0 0 1 3.6 11.5 8.4 8.4 0 0 1 12 3.1a8.4 8.4 0 0 1 9 8.4Z" />
  </Icon>
)

export const KEYBOARD = (
  <Icon size="s">
    <rect x="2" y="5" width="20" height="14" rx="2" />
    <path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M18 13h.01M9 15h6" />
  </Icon>
)
