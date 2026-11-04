# Nagsworth browser extension: privacy

The Nagsworth extension tells the Nagsworth desktop app, running on the same
computer, which tab you are looking at, so the app can count your time on
the web next to the time it counts in other programs. It has no server of
its own and sends nothing to the internet.

## What the extension collects

- The address (URL) and title of the active tab, when you switch tabs,
  when the active tab loads a new page or changes its title, and once a
  minute while a browser window has focus.
- That a tab was closed, or that no browser window has focus any more.
- The browser family (for example "firefox" or "edge"), worked out from the
  browser's user-agent string, and the extension's version.
- A random identifier made when the extension is installed, so the app can
  tell two browsers apart. It is not linked to you or your device.

It does not read page contents, form fields, passwords, cookies, browsing
history, bookmarks or downloads, and it runs no code inside web pages.

## Where it goes

Only to the Nagsworth app on this computer, at `http://127.0.0.1` on port
47821 (or 47822 to 47824 if that one is taken). The connection never leaves
the machine. There is no remote server, no analytics, no advertising and no
crash reporting, and no data is sold or shared with anyone.

Until the app accepts them, events wait in the browser's extension storage
(at most 5000; the oldest are dropped first).

## Private windows

Tabs in incognito or private windows are not reported. When such a window
has focus the app only learns that no normal tab is being looked at. You can
turn reporting of private tabs on in the extension's settings; the browser
must also allow the extension in private windows (`about:addons` in Firefox,
the extensions page in Chromium browsers), and the app has its own
`web.include_incognito` setting that is off by default.

## Where the data is kept

The app stores what it receives in its SQLite database in its data folder
(`%LOCALAPPDATA%\nagsworth` on Windows, `~/.local/share/nagsworth` on
Linux). By default it keeps only the site of each address (for example
`example.org`) and drops the rest; its `web.store_url = "full"` setting
keeps whole addresses.

## How to delete it

- **Unpair** in the extension's settings stops reporting; the app also lists
  paired browsers under Settings > Paired browsers.
- **Forget everything** in the extension's settings removes the pairing
  token, queued events and the random identifier from the browser.
  Removing the extension does the same.
- Time already recorded is deleted in the app, or by removing the app's
  data folder.

## Contact

Questions and reports: the project's issue tracker at
https://github.com/Balls0fSteel/dc1996-Nagsworth/issues
