#!/usr/bin/env bash
# Runtime checks of the Linux platform backend without a desktop session:
#   X11      Xvfb + xfwm4, xterm windows switched with xdotool
#   Wayland  headless sway, foot windows focused with swaymsg, input from wtype
#   logind   Lock/Unlock signals and Session.Lock in this login session
#   autostart the XDG .desktop file
# Needs: Xvfb xfwm4 xterm xdotool wmctrl sway foot wtype (Debian/Ubuntu packages of
# the same names). Run from the repository: tools/linux_checks.sh
set -u
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -q -p ns-platform --example probe || exit 1
P=target/debug/examples/probe
pass=0
fail=0
check() {
  if eval "$2"; then echo "ok   $1"; pass=$((pass + 1)); else echo "FAIL $1"; fail=$((fail + 1)); fi
}
idle_of() { grep -oE 'idle +[0-9]+' <<<"$1" | tail -1 | grep -oE '[0-9]+'; }
cleanup() { kill "${pids[@]}" 2>/dev/null; wait 2>/dev/null; }
pids=()
trap cleanup EXIT

echo "== X11 (Xvfb + xfwm4)"
Xvfb :77 -screen 0 1280x800x24 -nolisten tcp >/dev/null 2>&1 & pids+=($!)
sleep 1
export DISPLAY=:77
xfwm4 --compositor=off >/dev/null 2>&1 & pids+=($!)
sleep 1
xterm -T "alpha window" -e sleep 600 >/dev/null 2>&1 & pids+=($!)
xterm -T "beta window" -e sleep 600 >/dev/null 2>&1 & pids+=($!)
sleep 2
xdotool search --name "alpha window" windowactivate --sync 2>/dev/null
out=$($P 1); echo "$out" | tail -1
check "x11: active window is xterm 'alpha window'" 'grep -q "xterm .*alpha window" <<<"$out"'
xdotool search --name "beta window" windowactivate --sync 2>/dev/null
out=$($P 1); echo "$out" | tail -1
check "x11: switch to 'beta window'" 'grep -q "xterm .*beta window" <<<"$out"'
sleep 3
out=$($P 1); i=$(idle_of "$out")
check "x11: idle grows without input ($i ms)" '[ "${i:-0}" -ge 2500 ]'
xdotool mousemove 100 100 mousemove 200 200
out=$($P 1); i=$(idle_of "$out")
check "x11: input resets idle ($i ms)" '[ "${i:-99999}" -lt 1500 ]'
wmctrl -r :ACTIVE: -b add,fullscreen
sleep 1
out=$($P 1); echo "$out" | tail -1
check "x11: fullscreen window detected" 'grep -q "fullscreen true" <<<"$out"'
cleanup; pids=()
unset DISPLAY

echo "== Wayland (headless sway)"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
cfg=$(mktemp)
echo "output HEADLESS-1 resolution 1280x800" >"$cfg"
before=$(ls "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$')
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman sway -c "$cfg" >/tmp/an-sway.log 2>&1 & pids+=($!)
sleep 3
export WAYLAND_DISPLAY=$(comm -13 <(echo "$before") <(ls "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$') | tail -1)
export SWAYSOCK=$(ls -t "$XDG_RUNTIME_DIR"/sway-ipc.*.sock | head -1)
echo "WAYLAND_DISPLAY=$WAYLAND_DISPLAY"
swaymsg -q exec "foot --app-id=term-alpha --title='alpha wl' sleep 600"
swaymsg -q exec "foot --app-id=term-beta --title='beta wl' sleep 600"
sleep 3
swaymsg -q '[app_id=term-alpha] focus'
out=$($P 1); echo "$out" | tail -1
check "wayland: focused toplevel is term-alpha 'alpha wl'" 'grep -q "term-alpha .*alpha wl" <<<"$out"'
swaymsg -q '[app_id=term-beta] focus'
out=$($P 1); echo "$out" | tail -1
check "wayland: focus moves to term-beta" 'grep -q "term-beta .*beta wl" <<<"$out"'
# Each probe is a new Wayland client: idle is counted from its own
# subscription, so sample over a few seconds (the last line is at ~3 s).
out=$($P 4); i=$(idle_of "$out")
check "wayland: idle grows without input ($i ms)" '[ "${i:-0}" -ge 2000 ]'
( sleep 2.5; wtype x ) &
out=$($P 4); i=$(idle_of "$out")
check "wayland: input resets idle ($i ms)" '[ "${i:-99999}" -lt 1500 ]'
swaymsg -q fullscreen enable
sleep 1
out=$($P 1); echo "$out" | tail -1
check "wayland: fullscreen toplevel detected" 'grep -q "fullscreen true" <<<"$out"'
cleanup; pids=()
rm -f "$cfg"
unset WAYLAND_DISPLAY SWAYSOCK

echo "== logind"
sid="${XDG_SESSION_ID:-}"
$P --session 8 >/tmp/an-session.log 2>&1 & pids+=($!)
sleep 2
$P --lock | tail -1
sleep 1
loginctl unlock-session "$sid" 2>&1
sleep 1
loginctl lock-session "$sid" 2>&1
sleep 1
loginctl unlock-session "$sid" 2>&1
wait "${pids[@]}" 2>/dev/null; pids=()
cat /tmp/an-session.log
check "logind: Session.Lock from lock_workstation emits Locked" 'grep -q "Locked" /tmp/an-session.log'
check "logind: unlock emits Unlocked" 'grep -q "Unlocked" /tmp/an-session.log'

echo "== autostart"
out=$($P --autostart); echo "$out"
check "autostart: entry created" 'grep -q "autostart now: Ok(true)" <<<"$out"'
check "autostart: entry removed" 'grep -q "autostart after: Ok(false)" <<<"$out"'

echo "$pass passed, $fail failed"
exit "$fail"
