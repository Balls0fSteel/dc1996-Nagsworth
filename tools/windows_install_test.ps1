# Clean-machine check of the NSIS installer, run on a Windows test machine
# (over SSH is fine: nothing needs a desktop) or by the release workflow on
# its runner.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File windows_install_test.ps1 -Setup .\Nagsworth_0.1.0_x64-setup.exe
#
# Installs silently for the current user, checks the files and the Settings >
# Apps entry, starts the installed app hidden with a scratch data folder,
# checks the local API (health, 401 without a token, state with it), stops it,
# runs the setup again as Nagsworth's own update does (/S /UPDATE /R
# /WAITPID=<pid>: it must wait for that process, install over the folder and
# start the app again), stops that, uninstalls silently and checks it is gone.
# Prints ok / FAIL per check.
param(
  [Parameter(Mandatory)] [string]$Setup,
  [string]$Data = (Join-Path $env:TEMP "an-install-test")
)
$ErrorActionPreference = "Stop"
$failures = 0
function Check([string]$name, [bool]$cond) {
  if ($cond) { "ok   $name" } else { "FAIL $name"; $script:failures++ }
}
function UninstallEntry {
  Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like "nagsworth*" }
}
function StopApp {
  Get-Process nagsworth -ErrorAction SilentlyContinue | ForEach-Object {
    Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    Wait-Process -Id $_.Id -Timeout 10 -ErrorAction SilentlyContinue
  }
}

$setupPath = (Resolve-Path $Setup).Path
Check "no earlier install" ($null -eq (UninstallEntry))
Check "nothing running" ($null -eq (Get-Process nagsworth -ErrorAction SilentlyContinue))

$p = Start-Process $setupPath -ArgumentList "/S" -Wait -PassThru
Check "silent install exits 0 ($($p.ExitCode))" ($p.ExitCode -eq 0)
$entry = UninstallEntry
Check "Settings > Apps entry" ($null -ne $entry)
$dir = if ($entry.InstallLocation) { $entry.InstallLocation.Trim('"') } else { Join-Path $env:LOCALAPPDATA "Nagsworth" }
$exe = Join-Path $dir "nagsworth.exe"
Check "installed exe at $exe" (Test-Path $exe)
"version: $((Get-Item $exe).VersionInfo.ProductVersion), entry version: $($entry.DisplayVersion)"
$menu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Nagsworth.lnk"
Check "Start menu shortcut" (Test-Path $menu)

Remove-Item -Recurse -Force $Data -ErrorAction SilentlyContinue
$env:NAGSWORTH_DATA_DIR = $Data
$app = Start-Process $exe -ArgumentList "--hidden", "--no-tray" -PassThru
try {
  $base = "http://127.0.0.1:47821/v1"
  $health = $null
  for ($i = 0; $i -lt 60 -and -not $health; $i++) {
    try { $health = Invoke-RestMethod "$base/health" } catch { Start-Sleep -Milliseconds 500 }
  }
  Check "app answers /health" ($health.ok -eq $true)
  $code = try { Invoke-WebRequest "$base/state" -UseBasicParsing | Out-Null; 200 } catch { [int]$_.Exception.Response.StatusCode }
  Check "/state without token is 401 ($code)" ($code -eq 401)
  Check "config written to the data folder" (Test-Path "$Data\config.toml")
  Check "database created" (Test-Path "$Data\activity.db")
  $log = Get-Content "$Data\nagsworth.log" -Raw -ErrorAction SilentlyContinue
  Check "log says started" ($log -match "started; data in")
} finally {
  Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
}

# Nagsworth's own update (crates/ns-update, src-tauri/windows/hooks.nsh): the setup waits for
# the process /WAITPID names (here a sleep it must outwait, standing in for the exiting app),
# installs over the installation found and starts nagsworth.exe again with /ARGS.
$marker = Join-Path $dir "update-marker.txt"
Set-Content $marker "left by the install test: /UPDATE keeps the folder"
$sleeper = Start-Process powershell -PassThru -WindowStyle Hidden -ArgumentList '-NoProfile', '-Command', 'Start-Sleep 8'
$clock = [Diagnostics.Stopwatch]::StartNew()
# not -Wait: Windows PowerShell's also waits for the children, and the restarted app is one
$p = Start-Process $setupPath -PassThru -ArgumentList '/S', '/UPDATE', '/R', "/WAITPID=$($sleeper.Id)", '/ARGS', '--hidden --no-tray'
$p.WaitForExit()
Check "update setup exits 0 ($($p.ExitCode))" ($p.ExitCode -eq 0)
Check "update setup waited for /WAITPID ($([int]$clock.Elapsed.TotalSeconds) s)" ($sleeper.HasExited -and $clock.Elapsed.TotalSeconds -ge 7)
Check "exe still installed after the update" (Test-Path $exe)
Check "update kept the folder's other files" (Test-Path $marker)
Remove-Item $marker -ErrorAction SilentlyContinue
# /R starts the app through the desktop's shell (Explorer), so it needs a session with one:
# over SSH (no desktop) the check is skipped; run the script as a scheduled task with /IT, or
# from a console, to cover it.
$restarted = $null
$session = (Get-Process -Id $PID).SessionId
$shell = Get-Process explorer -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $session }
if ($shell) {
  for ($i = 0; $i -lt 30 -and -not $restarted; $i++) {
    Start-Sleep 1
    $restarted = Get-Process nagsworth -ErrorAction SilentlyContinue | Select-Object -First 1
  }
  Check "update setup started Nagsworth again (/R)" ($null -ne $restarted)
  if ($restarted) {
    Check "the restarted app is the installed one" ($restarted.Path -eq $exe)
  }
} else {
  "skip update setup started Nagsworth again (/R): no desktop shell in session $session"
  Start-Sleep 5
}
StopApp

$uninstaller = Join-Path $dir "uninstall.exe"
Check "uninstaller present" (Test-Path $uninstaller)
$u = Start-Process $uninstaller -ArgumentList "/S" -Wait -PassThru
# NSIS uninstallers copy themselves and return early; wait for the files to go.
for ($i = 0; $i -lt 30 -and (Test-Path $exe); $i++) { Start-Sleep -Milliseconds 500 }
Check "silent uninstall exits 0 ($($u.ExitCode))" ($u.ExitCode -eq 0)
Check "exe removed" (-not (Test-Path $exe))
Check "Start menu shortcut removed" (-not (Test-Path $menu))
Check "Settings > Apps entry removed" ($null -eq (UninstallEntry))
Remove-Item -Recurse -Force $Data -ErrorAction SilentlyContinue

"$failures failure(s)"
exit $failures
