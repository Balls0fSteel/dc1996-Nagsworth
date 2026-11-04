# Releasing

## Local builds

```bash
npm run tauri build
```

Windows produces `target/release/bundle/nsis/Nagsworth_<version>_x64-setup.exe`
(per-user install, no admin rights). On the first run the Tauri bundler
downloads its pinned NSIS toolset from Tauri's GitHub releases and checks
the hashes. On Linux, `src-tauri/tauri.linux.conf.json` switches the targets to
`.deb` and AppImage; install the WebKitGTK build dependencies listed in
`.github/workflows/ci.yml` first.

```bash
npm run ext:package
```

writes `extension/dist/nagsworth-chrome-<version>.zip` (Chrome Web
Store) and `nagsworth-firefox-<version>.zip` (addons.mozilla.org).

## Versions

Bump the version in `Cargo.toml` (`[workspace.package]`),
`src-tauri/tauri.conf.json`, `package.json`, `ui/package.json` and
`extension/package.json` (the extension manifest takes its version from the
last one).

## GitHub release

Push a tag `v<version>`. `.github/workflows/release.yml` builds the Windows
installer, the `.deb` and AppImage, and the two extension zips, and attaches
them to a draft release to review and publish by hand.

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

- Chrome Web Store: upload the chrome zip. The listing needs a privacy note:
  the extension sends tab URLs and titles only to `127.0.0.1`.
- AMO: upload the firefox zip; the add-on id is
  `nagsworth@nagsworth.org` (`scripts/build.mjs`). Source code
  upload is required because the scripts are bundled: attach a zip of
  `extension/` without `node_modules` and `dist`.
