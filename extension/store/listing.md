# Store listing

Paste-ready copy for the Chrome Web Store, Edge Add-ons and
addons.mozilla.org. Permission justifications are in `permissions.md`, the
privacy policy in `privacy.md`, the screenshots to take in `screenshots.md`.

## Name

Nagsworth

## Summary

Chrome Web Store allows 132 characters, AMO 250. This one is 126:

> Shows the Nagsworth desktop app which site you are on, so your web time
> counts toward its breaks and limits. Stays on your PC.

As one line:

Shows the Nagsworth desktop app which site you are on, so your web time counts toward its breaks and limits. Stays on your PC.

## Category

Productivity (all three stores).

## Description

Nagsworth is a desktop app for Windows and Linux that tracks how you spend
time on your computer and reminds you to take breaks, stop gaming after
your limit, or put the screen away at night. This extension is its eyes in
the browser: it tells the app which site the active tab shows, so an hour
on YouTube counts as an hour on YouTube rather than an hour of "browser".

The extension needs the Nagsworth app running on the same computer. It
does nothing on its own.

What you get

- Time per site in the app's dashboard, next to the time in every other
  program.
- Web time counted only while the browser is the window in front and you
  are actually at the computer; a video playing in a background tab does
  not count.
- A toolbar popup with what is being counted right now, today's web time
  and top sites, the progress of your break and limit rules, and buttons to
  snooze or skip a nag, start a break or pause tracking for 30 minutes.
- An optional badge on the toolbar icon with today's web time.

Private by design

- Everything stays on your computer. The extension talks only to the
  Nagsworth app at 127.0.0.1; there is no server, account, analytics or
  advertising.
- Incognito and private tabs are not reported unless you turn that on.
- By default the app stores only the site (example.org), not the whole
  address.
- The extension never reads page contents and runs no code in web pages.

Getting started

1. Install and start the Nagsworth app.
2. In the app's dashboard open Settings and click "Show pairing code".
3. Open the extension's settings, enter the six-digit code and click Pair.

The same extension works in Chrome, Edge, Brave, Opera, Vivaldi and
Firefox.

## Single purpose (Chrome Web Store)

The extension reports the active tab's address and title to the Nagsworth
desktop app running on the same computer, so the app can include time spent
on websites in its activity tracking and break reminders.

## Remote code (Chrome Web Store)

No, I am not using remote code. All JavaScript is bundled in the package;
the extension loads no external scripts, uses no eval and fetches only JSON
from the Nagsworth app at http://127.0.0.1.

## Data use declarations (Chrome Web Store privacy tab)

- Collected: "Web history" (the address and title of the active tab).
  Nothing else in the list.
- Certify: the data is not sold to third parties; not used or transferred
  for purposes unrelated to the item's single purpose; not used or
  transferred to determine creditworthiness or for lending.
- Where it goes: only to the Nagsworth app on the user's own computer
  (127.0.0.1). The developer never receives it.

## Data collection (AMO)

The manifest declares `browsingActivity` as required in
`data_collection_permissions`: the active tab's address and title leave the
extension for the Nagsworth app on the same computer. Nothing is sent off
the device.

## Notes for reviewers (AMO, Edge)

The extension is a companion to the Nagsworth desktop app
(https://github.com/Balls0fSteel/dc1996-Nagsworth) and only talks to it on
http://127.0.0.1, ports 47821 to 47824. Without the app it shows "The
Nagsworth app is not running on this computer" and sends nothing.

The scripts are bundled with Vite. To rebuild from the source zip with
Node 22:

    npm ci
    npm run package

which writes `dist/firefox` and `dist/nagsworth-firefox-<version>.zip`
(`dist/chrome` for Chromium browsers). `npm run check` runs TypeScript and
web-ext lint, `npm test` the unit tests. The sources are in `src/`, the
pages in `static/`; the build script is `scripts/build.mjs`.

To try it: install the app from the project's releases, open its dashboard
(tray icon > Open dashboard), Settings > Browser extension > "Show pairing
code", and enter the code on the extension's settings page.
