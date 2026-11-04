# Permission justifications

One answer per permission, for the Chrome Web Store privacy tab and the
reviewer notes on AMO and Edge Add-ons. The manifest asks for exactly these;
there are no content scripts and no optional permissions.

## tabs

Reads the address and title of the active tab when the user switches tabs,
when the active tab navigates and when a browser window gains focus, so the
Nagsworth desktop app on the same computer can count time per website. The
extension never reads page contents.

## storage

Keeps the pairing token for the local Nagsworth app, a random client id,
the user's settings, and a queue of tab changes that waits while the app is
closed (storage.local), plus the retry time and the last reported tab for
the popup (storage.session). Nothing is synced.

## alarms

Wakes the background worker every 30 seconds to deliver queued tab changes
to the local app and every minute to report the active tab, because a
Manifest V3 service worker stops between events.

## idle

Delivers queued tab changes when the user goes idle or locks the screen, so
the app's record is up to date at the moments it decides whether a break was
taken.

## Host permission http://127.0.0.1/*

The extension's only connection: the Nagsworth desktop app's local API on
this computer (ports 47821 to 47824). It sends tab changes and reads today's
totals and reminder status for the popup. No other host is contacted.
