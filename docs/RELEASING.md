# Releasing

## Local builds

```bash
npm run tauri build
```

Windows produces `target/release/bundle/nsis/Nagsworth_<version>_x64-setup.exe`
(per-user install, no admin rights). On the first run the Tauri bundler
downloads its pinned NSIS toolset from Tauri's GitHub releases and checks
the hashes. `src-tauri/windows/hooks.nsh` adds the `/WAITPID=<pid>` switch
the app's own update uses (below) to Tauri's installer template. On Linux,
`src-tauri/tauri.linux.conf.json` switches the targets to
`.deb` and AppImage; install the WebKitGTK build dependencies listed in
`.github/workflows/ci.yml` first.

```bash
npm run ext:package
npm run ext:source
```

write `extension/dist/nagsworth-chrome-<version>.zip` (Chrome Web
Store, Edge Add-ons), `nagsworth-firefox-<version>.zip` (addons.mozilla.org)
and `nagsworth-extension-source-<version>.zip` (the AMO source upload).

## Versions

Bump the version in `Cargo.toml` (`[workspace.package]`),
`src-tauri/tauri.conf.json`, `package.json`, `ui/package.json` and
`extension/package.json` (the extension manifest takes its version from the
last one). The release workflow refuses a tag whose version differs from any
of them: the app compares the workspace version with the release tags, and
the setup's file name comes from `tauri.conf.json`. Versions are
`MAJOR.MINOR.PATCH`, with a `-rc.1`-style suffix for a pre-release (which
the workflow marks as such on GitHub, and which the app offers only with
"Include pre-releases" on).

## GitHub release

Push a tag `v<version>`. `.github/workflows/release.yml` builds the Windows
installer (and installs, updates and uninstalls it on the runner with
`tools/windows_install_test.ps1`), the `.deb` and AppImage, the two
extension zips and the extension source zip, writes a `SHA256SUMS` over all of them, and attaches
everything to a draft release to review and publish by hand. Started by
hand (`workflow_dispatch`) it builds the same files as artifacts and makes
no release.

Keep the file names: the app's update check looks for
`Nagsworth_<version>_x64-setup.exe` and `SHA256SUMS` among the release's
files (`crates/ns-update/src/version.rs`). A draft release is invisible to
the check; it sees the release once it is published.

## The app updating itself

Nagsworth checks GitHub's releases list 30 s after a start and then twice a
day (Settings > Updates, off with "Check for updates automatically";
"Check now" and the tray's "Check for updates…" ask at once). A newer
version shows as a banner in the dashboard (dismissed per version with
"Later"), in the Settings card and in the tray item's text.

On Windows, a Nagsworth that the setup installed (its Settings > Apps entry
names the folder it runs from) downloads the release's setup to
`%TEMP%\Nagsworth-update`, checks it against the release's `SHA256SUMS`,
and on "Install and restart" checks it once more, starts it silently with
`/S /UPDATE /R /WAITPID=<its pid> /ARGS --hidden` and exits. The setup waits
for the process to exit (`src-tauri/windows/hooks.nsh`), installs over the
same folder without new shortcuts, keeps the data, and starts the new
version in the tray. A copy run from anywhere else, and Linux, get the
release page instead. `crates/ns-update` holds the logic;
`crates/ns-runtime/src/update.rs` the state, `/v1/update/*` the controls;
docs/DECISIONS.md D22 the reasoning.

To try it without a release: serve a fake listing and files locally and
point the app at them (docs/MANUAL_CHECKS.md, "Update check"). A build with
`NAGSWORTH_NO_UPDATE_CHECK` set at compile time has no check (for a package
that a package manager updates).

## Code signing (Windows)

Unsigned installers work but trigger SmartScreen warnings. To sign, add two
repository secrets:

- `WINDOWS_CERTIFICATE`: the code signing `.pfx`, base64-encoded
  (`[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))`).
- `WINDOWS_CERTIFICATE_PASSWORD`: its password.

The release job imports the certificate into the runner's user store and
passes its thumbprint to `tauri build --config src-tauri/tauri.signing.json`
(generated, git-ignored), so `tauri.conf.json` stays free of signing details.
Without the secrets the job builds unsigned. For a local signed build,
import the certificate and write the same file by hand:

```json
{ "bundle": { "windows": { "certificateThumbprint": "<thumbprint>", "digestAlgorithm": "sha256", "timestampUrl": "http://timestamp.digicert.com" } } }
```

## Extension stores

Nothing has been submitted yet: check the name first (PLAN.md), then create
the developer accounts. The texts to paste are final copy in
`extension/store/`: `listing.md` (name, summary, description, category,
single purpose, remote code, notes for reviewers), `permissions.md` (one
justification per permission), `privacy.md` (the privacy policy; the same
text ships in the extension as `privacy.html`) and `screenshots.md` (the
shots to take and their sizes). Run `npm run ext:check` (tsc, build,
`web-ext lint`) and `npm run ext:test` before uploading.

- Chrome Web Store: upload the chrome zip, fill the privacy tab from
  `permissions.md` and `privacy.md` ("does not use remote code"), and
  link the privacy policy (`privacy.md` on GitHub or the release page).
- Edge Add-ons (Partner Center): upload the same chrome zip; Edge needs no
  build of its own. Brave, Opera and Vivaldi install from the Chrome Web
  Store.
- AMO: upload the firefox zip; the add-on id is `nagsworth@nagsworth.org`
  (`scripts/build.mjs`), the minimum is Firefox 140 and the manifest
  declares `browsingActivity` in `data_collection_permissions`. The scripts
  are bundled, so attach `nagsworth-extension-source-<version>.zip` as the
  source code and paste the reviewer notes from `listing.md`.
- The add-on id and the Chrome item id must stay the same across versions;
  the extension's version comes from `extension/package.json`.
