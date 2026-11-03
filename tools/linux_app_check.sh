#!/usr/bin/env bash
# Runs the built app (target/debug/activity_nagger, from
# `npm run tauri -- build --debug`) under Xvfb + xfwm4 with an xterm in front
# and checks it end to end: the API, tracking of the X11 window, a break
# overlay window, and a screenshot of the screen.
# Needs: Xvfb xfwm4 xterm xdotool sqlite3 imagemagick. Usage:
#   tools/linux_app_check.sh [screenshot.png]
set -u
cd "$(dirname "$0")/.."
shot="${1:-/tmp/an-app.png}"
data=$(mktemp -d)
pass=0
fail=0
check() {
  if eval "$2"; then echo "ok   $1"; pass=$((pass + 1)); else echo "FAIL $1"; fail=$((fail + 1)); fi
}
pids=()
cleanup() { kill "${pids[@]}" 2>/dev/null; wait 2>/dev/null; rm -rf "$data"; }
trap cleanup EXIT

Xvfb :79 -screen 0 1600x900x24 -nolisten tcp >/dev/null 2>&1 & pids+=($!)
sleep 1
export DISPLAY=:79
# A private session bus, as a desktop session would have.
eval "$(dbus-launch --sh-syntax)"
pids+=("$DBUS_SESSION_BUS_PID")
xfwm4 --compositor=off >/dev/null 2>&1 & pids+=($!)
sleep 1

AN_DATA_DIR="$data" target/debug/activity_nagger --hidden >"$data/stdout.log" 2>&1 & pids+=($!)
base=http://127.0.0.1:47821/v1
for _ in $(seq 1 60); do curl -sf "$base/health" >/dev/null && break; sleep 0.5; done
check "app answers /health" 'curl -sf "$base/health" | grep -q "\"ok\":true"'
code=$(curl -s -o /dev/null -w '%{http_code}' "$base/state")
check "/state without token is 401 ($code)" '[ "$code" = 401 ]'
token=$(sqlite3 "$data/activity.db" "select value from settings where key='api.token'")
auth=(-H "Authorization: Bearer $token")

xterm -T "tracked window" -e sleep 600 >/dev/null 2>&1 & pids+=($!)
sleep 2
xdotool search --name "tracked window" windowactivate --sync 2>/dev/null
sleep 3
state=$(curl -sf "${auth[@]}" "$base/state")
check "state: the xterm is the current activity" 'grep -q "\"exe_name\":\"xterm\"" <<<"$state"'
sleep 12
apps=$(curl -sf "${auth[@]}" "$base/stats/apps")
echo "apps: $apps"
check "stats: xterm has tracked time" 'grep -qE "\"exe_name\":\"xterm\".*\"ms\":[1-9]" <<<"$apps"'
tl=$(curl -sf "${auth[@]}" "$base/stats/timeline")
check "timeline: a segment titled 'tracked window'" 'grep -q "tracked window" <<<"$tl"'

curl -sf -X POST "${auth[@]}" "$base/control/break/start" >/dev/null
sleep 4
overlays=$(xdotool search --name '^Break$' 2>/dev/null | wc -l)
check "break: overlay window open ($overlays)" '[ "$overlays" -ge 1 ]'
import -window root "$shot" 2>/dev/null
check "screenshot written ($shot)" '[ -s "$shot" ]'
nags=$(curl -sf "${auth[@]}" "$base/nags")
check "nag log has break_started" 'grep -q "break_started" <<<"$nags"'

grep -iE "tray|warn|error" "$data/activity_nagger.log" | tail -5
echo "$pass passed, $fail failed"
exit "$fail"
