# Browser extension 0.2: build brief

Drafted 2026-11-04_133502_1793795702. Status: built 2026-11-04_140801_1793797681 (CHANGELOG.md);
the open questions were decided as the brief proposed (controls in the
popup, badge off by default), and Firefox's minimum went to 140.
Meant to be handed to an implementation session (planned: Opus 5.5, medium
effort) as its whole task. Read AGENTS.md, PLAN.md and CHANGELOG.md first;
the repository conventions there apply.

## What already exists (do not rebuild)

The extension in `extension/` is milestone M4 and works end to end. It was
verified on 2026-11-03 in headless Edge 156 and Firefox 159 by
`tools/ext_e2e.ps1` (pairing, queued tab events, heartbeats), and both store
zips are built by `npm run ext:package`.

| File | Role |
|---|---|
| `extension/manifest.base.json` | MV3 manifest shared by both browsers: permissions `tabs`, `storage`, `alarms`, `idle`; host permission `http://127.0.0.1/*`; `options_ui` and `action.default_popup` both point at `options.html`. |
| `extension/scripts/build.mjs` | Bundles `src/background.ts` and `src/options.ts` with Vite into self-contained IIFEs (D16), copies `static/` and `icons/`, writes `dist/chrome` (`background.service_worker`) and `dist/firefox` (`background.scripts`, gecko id `nagsworth@nagsworth.org`, `strict_min_version` 115). |
| `extension/scripts/package.mjs` | Dependency-free zip writer; emits `dist/nagsworth-chrome-<v>.zip` and `dist/nagsworth-firefox-<v>.zip`. |
| `extension/src/background.ts` | Listens to `tabs.onActivated`, `tabs.onUpdated` (active tab, url or title changed), `tabs.onRemoved`, `windows.onFocusChanged`, `idle.onStateChanged`. Events go into the queue; a flush alarm every 30 s and at 20 queued items, a heartbeat alarm every minute carrying the active tab. Backoff 5 s to 5 min while the app is down. Incognito events are rewritten to `focus_lost` with no url unless the user opted in. |
| `extension/src/queue.ts` | `storage.local` FIFO capped at 5000 events, serialized read-modify-write. |
| `extension/src/api.ts` | Probes ports 47821-47824 with `GET /v1/health` (expects `name == "nagsworth"`), remembers the port, `POST` with the bearer token, forgets the token on 401, `pair(code)` exchanges a code for the token. |
| `extension/src/browser.ts` | `ext` = `browser` or `chrome`; browser family from the user agent (`chrome`, `edge`, `firefox`, `opera`, `vivaldi`, `brave`); settings (`clientId`, `token`, `port`) in `storage.local`. |
| `extension/src/options.ts`, `static/options.html` | Status line, pairing form, incognito checkbox, unpair button, queued count. Used as both the options page and the toolbar popup. |
| `tools/ext_e2e.ps1`, `tools/seed_ext.py` | Hidden-desktop end-to-end run in Edge and Firefox with throwaway profiles; the test copy of the extension pairs itself through `/pair`. |

### Host side the extension talks to (`crates/ns-api/src/routes.rs`)

- `GET /v1/health` is public. `POST /v1/pair {code}` returns `{token}`;
  codes come from the dashboard (Settings > Browser extension) or the
  pairing window and are valid 5 minutes, one use.
- `POST /v1/ext/events {client_id, events: [ExtEvent]}` where `ExtEvent` is
  `{browser, ts, kind, tab_id, window_id, url, title, incognito}` and `kind`
  is one of `tab_activated`, `tab_updated`, `tab_removed`, `focus_lost`,
  `heartbeat`. The host deduplicates by (`client_id`, `ts`, `tab_id`), drops
  incognito events unless `web.include_incognito` is set, and cuts URLs to
  the origin when `web.store_url = "host"`. Reply: `{accepted, duplicates,
  dropped}`.
- `POST /v1/ext/heartbeat {client_id, browser, version, active}` registers
  the client (shown under Settings > Paired browsers and in
  `GET /state`.`extensions` as `[client_id, {browser, version, last_seen}]`)
  and feeds `active` to the web segmenter as a `heartbeat` event.
- `GET /v1/state` returns `{now, today, activity, tracking_paused, engine,
  extensions}`; `engine.rules[]` are `RuleSnapshot {id, name, trigger,
  enabled, state, accum_ms, target_ms, rest_ms, break_ms, snoozes_left,
  message}` (`crates/ns-rules/src/engine.rs`).
- `GET /v1/stats/web?from=<YYYY-MM-DD>&to=<YYYY-MM-DD>` (both optional,
  default today) returns per-host totals; see `stats_web` for the row shape.
- Controls: `POST /v1/control/snooze {rule_id}`, `/control/skip {rule_id}`,
  `/control/break/start` (optional `{rule_id}`), `/control/pause` (optional
  `{minutes}`), `/control/resume`.
- CORS allows `chrome-extension://` and `moz-extension://` origins, so the
  fetches work in Firefox even where MV3 treats host permissions as optional.
- The web segmenter (`crates/ns-tracker/src/web.rs`) only accrues time while
  the browser is the foreground app and the user is not idle; the extension
  does not need to judge that.

## Goal

Turn the working prototype into something a user installs from a store and
lives with for months: tracking that is right at the edges (first start,
private windows, app restarts, a sleeping service worker), a toolbar popup
that shows what is being counted and today's numbers, unit tests, and
store-ready packages with the listing material written. One codebase, both
browsers; the Chrome zip also serves Edge, Brave, Opera and Vivaldi.

## Work items

### 1. Tracking edge cases (`background.ts`)

1. **Focus moves to a window the extension cannot read.** A Firefox private
   window without "Run in Private Windows", or a Chrome incognito window
   without "Allow in Incognito", makes `tabs.query({active: true, windowId})`
   return nothing in `windows.onFocusChanged`. Today nothing is recorded and
   the previous tab keeps accruing while the user reads elsewhere. Record
   `focus_lost` whenever the focused window yields no readable active tab.
2. **Start-up.** On `runtime.onInstalled`, `runtime.onStartup` and at every
   worker start, send one heartbeat at once when paired, so the active tab is
   known within seconds instead of up to a minute. The host treats heartbeats
   as idempotent state, so this is safe.
3. **Worker lifetime.** Chrome stops an idle MV3 worker after about 30 s and
   module-level state (`backoffMs`, `retryAt`, `flushing`) is lost. Keep the
   retry deadline in `storage.session` (Chrome 102+, Firefox 115+) so a
   restarted worker does not hammer a closed app every 30 s; set
   `minimum_chrome_version` accordingly. The alarms are re-created at each
   start already; keep that.
4. **Tabs in unfocused windows.** `tabs.onUpdated` reports `tab.active` for
   the active tab of every window, focused or not. The host ignores a
   `tab_updated` whose `tab_id` is not the tab it is counting, so this is
   correct today; add a unit test that documents it rather than changing the
   behaviour.
5. **Incognito scrub** stays in the extension (as well as on the host): the
   url and title of a private tab must never leave the browser unless the
   user opted in. Cover it with a test.

### 2. Port discovery (`api.ts`)

- `findPort` probes the four ports one after another with a 1.5 s timeout
  each, so the popup can take 6 s to say "not running". Probe the remembered
  port first, then the others in parallel; a closed app must be reported
  within about 2 s.
- Every `post` calls `findPort`, which is an extra `/health` round trip per
  flush. Keep the last good port in memory for the worker's lifetime and
  re-probe only after a connection failure.

### 3. Toolbar popup (new `static/popup.html`, `src/popup.ts`)

The options page keeps pairing and settings; the popup becomes the thing the
user clicks during the day. Plain TypeScript and HTML like the options page,
no framework; both color schemes via `prefers-color-scheme`, colors taken
from the dashboard's tokens in `ui/src/lib/ui`. Contents:

- **Status line**: app not running / not paired (with a link that opens the
  options page) / paired as `<browser>` on port `<n>`, and when the last
  flush succeeded.
- **Now**: the site the extension last reported and whether the app is
  counting it. `GET /state`.`activity` tells whether the foreground app is a
  browser and whether the user is idle; show "Counting", "Browser not in
  front" or "Idle".
- **Today**: total web time and the top five hosts from
  `GET /stats/web` for today, as "1h 12m".
- **Nags**: the enabled rules from `engine.rules` with their progress
  (`accum_ms` of `target_ms`, state), for example "Rest break in 12 min" or
  "Gaming limit 1h 02m of 2h"; the `message` field is ready-made text.
- **Controls**: Snooze and Skip for a rule that is `Pending` or `Active`,
  "Start break now", and "Pause tracking 30 min" / "Resume" (see the control
  routes above). The whole app is affected by these, which is what the tray
  menu does too.
- **Badge** (setting, off by default): today's web time on the toolbar icon
  ("1h"), refreshed by the heartbeat alarm and when the popup opens.

Manifest: `action.default_popup` becomes `popup.html`; `options_ui` stays
`options.html`; add `popup` to the entry list in `build.mjs`.

### 4. Options page

- Keep pairing, unpair, the incognito checkbox and the queued count.
- Add the badge toggle and a "Forget everything" button that clears
  `storage.local` and `storage.session` (token, queue, client id).
- When the incognito checkbox is on but the browser has not allowed the
  extension in private windows (`extension.isAllowedIncognitoAccess()`),
  say so and name the place to allow it (`about:addons` in Firefox,
  `chrome://extensions` in Chromium browsers). Without that permission the
  extension never sees those tabs and the checkbox does nothing.
- Link to the privacy text (item 6) from the page.

### 5. Unit tests

- Add `vitest` as a devDependency of `extension/`, `npm run ext:test` at the
  root, and run it in the extension step of `.github/workflows/ci.yml`
  (after `npm run ext:check`).
- A small fake of the WebExtension API in `extension/test/` (storage.local
  and storage.session backed by maps, alarms, tabs and windows event
  emitters, `runtime.getManifest`) so the modules load unchanged.
- Cases: queue FIFO order and the 5000 cap; `findPort` order, parallel
  probing and the timeout; token forgotten on 401; incognito scrub in
  `record` and in the heartbeat; `focus_lost` for an unreadable focused
  window; `tab_updated` from an unfocused window is sent unchanged (item
  1.4); browser family from user-agent strings for all six families; the
  retry deadline survives a simulated worker restart.

### 6. Store readiness

- `npm run ext:check` additionally runs `web-ext lint --source-dir
  dist/firefox` (web-ext is already a devDependency; build first) and must
  pass without errors.
- `npm run ext:source` zips the `extension/` sources without `node_modules`
  and `dist` for the AMO source upload, reusing the `zip` function in
  `scripts/package.mjs`. Wire it into the release workflow next to the
  store zips and `SHA256SUMS` (docs/RELEASING.md).
- `extension/store/` with the listing material, written and ready to paste:
  - `listing.md`: name, the short summary (Chrome Web Store allows 132
    characters), the long description, category Productivity, the
    single-purpose statement Chrome asks for, and the "no remote code"
    declaration.
  - `permissions.md`: one justification per permission (`tabs`, `storage`,
    `alarms`, `idle`, host `http://127.0.0.1/*`), in the wording the store
    forms expect.
  - `privacy.md`: what is collected (url and title of the active tab,
    browser family, a random client id), where it goes (only
    `127.0.0.1`, ports 47821-47824, to the Nagsworth app on the same
    computer; no remote server, no analytics), that private windows are off
    by default, that the data lives in the app's SQLite database and how to
    delete it (unpair, "Forget everything", the app's data folder).
  - `screenshots.md`: the list of shots to take (1280x800 for both stores,
    plus 440x280 small tile for Chrome): popup paired, popup with a nag,
    options page with the pairing code, the dashboard's Web page.
- Edge Add-ons takes the Chrome zip; add the note to docs/RELEASING.md.
- Keep the add-on id `nagsworth@nagsworth.org`. Do not submit anything:
  the name has not been checked against the stores (PLAN.md next step 4)
  and the accounts do not exist yet.

### 7. Verification tooling and docs

- Extend `tools/ext_e2e.ps1` so the Edge run also opens
  `chrome-extension://<id>/popup.html` as a tab through the DevTools
  `json/new` endpoint, reads its text with `Runtime.evaluate` and saves a
  `Page.captureScreenshot` PNG. Node 22's global `WebSocket` is enough for
  the DevTools connection; put the client in `tools/ext_popup_check.mjs`.
  Pass when the text shows the paired status and today's hosts. Firefox has
  no equivalent hook; the Firefox popup is a manual check.
- Update docs/ARCHITECTURE.md ("Extension"), docs/RELEASING.md ("Extension
  stores"), docs/MANUAL_CHECKS.md ("Extension, by hand": popup contents,
  the badge, the private-window hint, Firefox popup), README.md (the
  extension section), PLAN.md and CHANGELOG.md.

## Host-side changes

Keep them to a minimum. The popup should be built from `/state` and
`/stats/web` as they are. If something is really missing, add it to an
existing response rather than a new route, add a test in
`crates/ns-api/src/tests.rs`, and do not change the event protocol: an
extension 0.1 must keep working against the new app and extension 0.2
against app 0.1 (treat missing fields as absent).

## Constraints

- Permissions stay exactly `tabs`, `storage`, `alarms`, `idle` and the
  `127.0.0.1` host permission. No `<all_urls>`, no content scripts, no
  `webNavigation`, `history` or `scripting`. Nothing leaves the machine.
- One codebase, IIFE bundles from `scripts/build.mjs` (D16). If an API
  needs a newer browser than Firefox 115, raise `strict_min_version` and
  say so in the changelog.
- TypeScript strict, `npm run ext:check` clean, no UI framework in the
  extension; keep each built script small.
- Testing conventions from AGENTS.md: the app and browsers run on the
  hidden desktop through `run_hidden.ps1`; `NAGSWORTH_PYTHON` points the
  e2e script at the venv Python; one descriptive commit per work item; no
  AI attribution in commits; timestamped entries in PLAN.md and
  CHANGELOG.md.

## Acceptance

- `npm run ext:check` (tsc and web-ext lint), `npm run ext:test`,
  `npm run ext:build`, `npm run ext:package`, `npm run ext:source`: all
  clean.
- `npm run tauri -- build --debug --no-bundle`, then
  `tools\ext_e2e.ps1 -Browser edge` and `-Browser firefox`: PASS, and the
  Edge run saves a popup screenshot that shows the paired status.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets` stay
  clean if the host was touched.
- The listing files read as final copy, not placeholders.
- PLAN.md "Next steps" and CHANGELOG.md updated; docs/MANUAL_CHECKS.md lists
  what the harness could not cover.

## Out of scope

- Safari: there is no macOS backend (D1).
- Per-site rules ("1 h of YouTube"): `Scope` in `ns-rules` has no host
  variant; that is a host-side design of its own.
- Blocking pages or nagging inside the browser: the app's toast and overlay
  already do that for every app, including browsers.
- Counting background media (`tab.audible`): would change what "web time"
  means; a later decision.
- Submitting to the stores.

## Open questions for the owner

- Should the popup offer the controls (snooze, skip, pause) at all, or only
  show state? The brief says yes, mirroring the tray menu.
- Badge default: off (as written) or on.
- Whether the Firefox minimum version may be raised from 115.
