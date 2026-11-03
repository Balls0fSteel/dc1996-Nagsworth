# Clean-machine check of the NSIS installer, run on a Windows test machine
# (over SSH is fine: nothing needs a desktop).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File windows_install_test.ps1 -Setup .\activity_nagger_0.1.0_x64-setup.exe
#
# Installs silently for the current user, checks the files and the Settings >
# Apps entry, starts the installed app hidden with a scratch data folder,
# checks the local API (health, 401 without a token, state with it), stops it,
# uninstalls silently and checks it is gone. Prints ok / FAIL per check.
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
    Where-Object { $_.DisplayName -like "activity_nagger*" }
}

$setupPath = (Resolve-Path $Setup).Path
Check "no earlier install" ($null -eq (UninstallEntry))

$p = Start-Process $setupPath -ArgumentList "/S" -Wait -PassThru
Check "silent install exits 0 ($($p.ExitCode))" ($p.ExitCode -eq 0)
$entry = UninstallEntry
Check "Settings > Apps entry" ($null -ne $entry)
$dir = if ($entry.InstallLocation) { $entry.InstallLocation.Trim('"') } else { Join-Path $env:LOCALAPPDATA "activity_nagger" }
$exe = Join-Path $dir "activity_nagger.exe"
Check "installed exe at $exe" (Test-Path $exe)
"version: $((Get-Item $exe).VersionInfo.ProductVersion), entry version: $($entry.DisplayVersion)"

Remove-Item -Recurse -Force $Data -ErrorAction SilentlyContinue
$env:AN_DATA_DIR = $Data
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
  $log = Get-Content "$Data\activity_nagger.log" -Raw -ErrorAction SilentlyContinue
  Check "log says started" ($log -match "started; data in")
} finally {
  Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
}

$uninstaller = Join-Path $dir "uninstall.exe"
Check "uninstaller present" (Test-Path $uninstaller)
$u = Start-Process $uninstaller -ArgumentList "/S" -Wait -PassThru
# NSIS uninstallers copy themselves and return early; wait for the files to go.
for ($i = 0; $i -lt 30 -and (Test-Path $exe); $i++) { Start-Sleep -Milliseconds 500 }
Check "silent uninstall exits 0 ($($u.ExitCode))" ($u.ExitCode -eq 0)
Check "exe removed" (-not (Test-Path $exe))
Check "Settings > Apps entry removed" ($null -eq (UninstallEntry))
Remove-Item -Recurse -Force $Data -ErrorAction SilentlyContinue

"$failures failure(s)"
exit $failures
