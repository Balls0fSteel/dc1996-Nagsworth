# End-to-end check of the browser extension on Windows.
#
#   npm run ext:build
#   npm run tauri -- build --debug --no-bundle
#   tools\ext_e2e.ps1 -Browser edge      # or firefox (needs `npm install --prefix extension`)
#
# Starts the debug app, a local page server and the browser on the hidden
# desktop (L:\dev\tools\run_hidden, or -RunHidden <path>) with throwaway
# profiles and data under %TEMP%\an-ext-e2e. A headless browser cannot type
# the pairing code, so the test copy of the extension pairs itself through the
# same /pair endpoint (tools\seed_ext.py). Passes when the app has stored the
# extension's tab events and lists its heartbeat.
param(
  [ValidateSet("edge", "firefox")] [string]$Browser = "edge",
  [string]$RunHidden = "L:\dev\tools\run_hidden\run_hidden.ps1",
  [string]$Python = $(if ($env:NAGSWORTH_PYTHON) { $env:NAGSWORTH_PYTHON } else { "python" })
)
$ErrorActionPreference = "Stop"
$Repo = Split-Path $PSScriptRoot
$S = Join-Path $env:TEMP "an-ext-e2e"
New-Item -ItemType Directory -Force "$S\pages" | Out-Null
foreach ($n in "a", "b", "c") { Set-Content "$S\pages\$n.html" "<!doctype html><title>Page $n</title><h1>Page $n</h1>" }
$env:NAGSWORTH_DATA_DIR = "$S\data-$Browser"
Remove-Item -Recurse -Force $env:NAGSWORTH_DATA_DIR -ErrorAction SilentlyContinue

function Query([string]$sql) {
  & $Python -I -c "import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute(sys.argv[2]).fetchone())" "$env:NAGSWORTH_DATA_DIR\activity.db" $sql
}

$pids = @()
$ok = $false
try {
  $pids += & $RunHidden -NoWait "$Repo\target\debug\nagsworth.exe" --no-tray
  $base = "http://127.0.0.1:47821/v1"
  for ($i = 0; $i -lt 60; $i++) { try { Invoke-RestMethod "$base/health" | Out-Null; break } catch { Start-Sleep -Milliseconds 500 } }
  $token = (Query "select value from settings where key='api.token'").Trim("()',")
  $hdr = @{ Authorization = "Bearer $token" }
  $code = (Invoke-RestMethod "$base/pair/start" -Method Post -Headers $hdr).code
  $pids += & $RunHidden -NoWait $Python -I -m http.server 8765 --bind 127.0.0.1 -d "$S\pages"

  if ($Browser -eq "edge") {
    $ext = & $Python -I "$PSScriptRoot\seed_ext.py" "$Repo\extension\dist\chrome" "$S\ext-edge" $code 47821
    Remove-Item -Recurse -Force "$S\edge-profile" -ErrorAction SilentlyContinue
    $pids += & $RunHidden -NoWait "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --headless=new "--user-data-dir=$S\edge-profile" "--load-extension=$ext" "--disable-extensions-except=$ext" --disable-features=DisableLoadExtensionCommandLineSwitch --no-first-run --remote-debugging-port=9333 http://127.0.0.1:8765/a.html
    Start-Sleep -Seconds 6
    # Tab changes over the DevTools HTTP endpoint.
    foreach ($page in "b", "c") {
      $t = Invoke-RestMethod -Method Put "http://127.0.0.1:9333/json/new?http://127.0.0.1:8765/$page.html"
      Start-Sleep -Seconds 2
      Invoke-RestMethod "http://127.0.0.1:9333/json/activate/$($t.id)" | Out-Null
      Start-Sleep -Seconds 2
    }
  } else {
    $ext = & $Python -I "$PSScriptRoot\seed_ext.py" "$Repo\extension\dist\firefox" "$S\ext-firefox" $code 47821
    $cmd = "$S\web-ext.cmd"
    Set-Content $cmd -Encoding ascii -Value @(
      "@echo off",
      "cd /d `"$Repo\extension`"",
      "call npx web-ext run --source-dir=`"$ext`" --firefox=`"C:\Program Files\Mozilla Firefox\firefox.exe`" --no-reload --no-input --arg=-headless --start-url=http://127.0.0.1:8765/a.html --start-url=http://127.0.0.1:8765/b.html --start-url=http://127.0.0.1:8765/c.html > `"$S\web-ext.log`" 2>&1"
    )
    $pids += & $RunHidden -NoWait cmd.exe /c $cmd
  }

  # The extension flushes on a 30 s alarm and sends a heartbeat every minute.
  for ($i = 0; $i -lt 24; $i++) {
    Start-Sleep -Seconds 5
    $seen = [int](Query "select count(*) from ext_events_seen").Trim("(),")
    $clients = @((Invoke-RestMethod "$base/state" -Headers $hdr).extensions)
    "t+$(($i + 1) * 5)s events=$seen heartbeats=$($clients.Count)"
    if ($seen -gt 0 -and $clients.Count -gt 0) { $ok = $true; break }
  }
  "extensions: $($clients | ConvertTo-Json -Compress -Depth 4)"
} finally {
  foreach ($p in $pids) { if ($p) { taskkill /T /F /PID $p 2>&1 | Out-Null } }
}
if ($ok) { "PASS ($Browser)" } else { "FAIL ($Browser)"; exit 1 }
