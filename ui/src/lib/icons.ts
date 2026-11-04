// Inline icon set: SVG path data on a 24x24 grid, drawn as strokes
// (round caps and joins, 1.75 wide) by `ui/Icon.svelte`. Circles are written
// as two arcs so every icon is a single `d` string.

export const icons = {
  // Navigation
  today: "M4 7a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2zM4 10h16M8 3v4M16 3v4M12 13.5v.01M9.5 16h5",
  history: "M4 20h16M7 16v-5M12 16V6M17 16v-8",
  apps: "M4 5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1zM14 5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1zM4 15a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1zM14 15a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1z",
  web: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M3.5 9h17M3.5 15h17M12 3c-2.5 2.6-3.5 5.6-3.5 9s1 6.4 3.5 9M12 3c2.5 2.6 3.5 5.6 3.5 9s-1 6.4-3.5 9",
  rules: "M3.5 6l1.5 1.5L8 4.5M3.5 13l1.5 1.5L8 11.5M11 6h10M11 13h10M4 19.5h.01M11 19.5h10",
  settings: "M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1M13 6a2 2 0 1 0 4 0a2 2 0 1 0-4 0M7 12a2 2 0 1 0 4 0a2 2 0 1 0-4 0M15 18a2 2 0 1 0 4 0a2 2 0 1 0-4 0",

  // Controls
  play: "M7 4.5v15a1 1 0 0 0 1.5.86l12-7.5a1 1 0 0 0 0-1.72l-12-7.5A1 1 0 0 0 7 4.5z",
  pause: "M7 5h3v14H7zM14 5h3v14h-3z",
  coffee: "M4 9h12v6a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5zM16 10h1.5a2.5 2.5 0 0 1 0 5H16M8 3v2.5M12 3v2.5",
  book: "M3 5.5A1.5 1.5 0 0 1 4.5 4H9a3 3 0 0 1 3 3v13a2.5 2.5 0 0 0-2.5-2.5h-5A1.5 1.5 0 0 1 3 16zM21 5.5A1.5 1.5 0 0 0 19.5 4H15a3 3 0 0 0-3 3v13a2.5 2.5 0 0 1 2.5-2.5h5A1.5 1.5 0 0 0 21 16z",
  snooze: "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5zM15 4h4l-4 4.5h4",
  skip: "M5 5.5v13l9-6.5zM18 5v14",
  check: "M5 12.5l4.5 4.5L19 7.5",
  x: "M6 6l12 12M18 6L6 18",
  copy: "M9 9a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2zM15 7V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h1",
  search: "M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0M20 20l-4-4",
  "chevron-left": "M15 6l-6 6 6 6",
  "chevron-right": "M9 6l6 6-6 6",
  "chevron-down": "M6 9l6 6 6-6",
  "chevron-up": "M6 15l6-6 6 6",
  plus: "M12 5v14M5 12h14",
  pencil: "M4 20h4L19 9a2.83 2.83 0 0 0-4-4L4 16zM13.5 6.5l4 4",
  refresh: "M20 11a8 8 0 0 0-14.5-4.5L4 8M4 4v4h4M4 13a8 8 0 0 0 14.5 4.5L20 16M20 20v-4h-4",
  download: "M12 4v11M7.5 10.5L12 15l4.5-4.5M5 19h14",
  "external-link": "M14 4h6v6M20 4l-9 9M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4",

  // Status
  alert: "M12 4L2.8 19.5a1 1 0 0 0 .87 1.5h16.66a1 1 0 0 0 .87-1.5zM12 10v4.5M12 17.5v.01",
  info: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M12 11v5.5M12 7.5v.01",
  shield: "M12 3l7.5 3v5.5c0 4.5-3.2 8-7.5 9.5c-4.3-1.5-7.5-5-7.5-9.5V6zM9 12l2 2 4-4",
  clock: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M12 7v5l3 2",
  gamepad:
    "M7 7h10a5 5 0 0 1 4.9 6l-.7 3.3a2.5 2.5 0 0 1-4.4 1l-1.8-2.3H9l-1.8 2.3a2.5 2.5 0 0 1-4.4-1L2.1 13A5 5 0 0 1 7 7zM7.5 10v4M5.5 12h4M15.5 11h.01M18 13h.01",
  monitor: "M3 6a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM8 21h8M12 17v4",
  lock: "M5 12a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2zM8 10V7a4 4 0 0 1 8 0v3M12 15v2",
  zap: "M13 3L4 14h7l-1 7 9-11h-7z",
  dot: "M12 12h.01",
} as const satisfies Record<string, string>;

export type IconName = keyof typeof icons;
