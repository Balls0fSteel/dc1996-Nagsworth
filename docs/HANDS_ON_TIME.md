# Hands-on time: input-gated gaming limits

Drafted 2026-11-04_104856_1793785736. Status: implementation in progress
(see PLAN.md "Hands-on time").

## The problem

A game is "in front" for as long as its window has the focus, whether the
player is playing, reading a wiki on the second monitor, chatting, or
watching a cutscene with the controller in their lap. The daily gaming
limit (`gaming-daily`, 2 h) counts all of that, because the Game scope
defaults to `idle_policy = "ignore"`: `GetLastInputInfo` does not see
gamepads, so without ignoring idle a controller player would never be
counted at all.

The limit is meant to protect hands and wrists (RSI), so it should count
the time the hands are actually working: keyboard, mouse or controller
input. An evening can then be 1 h 50 min in front of the PC and only
1 h of hands-on gaming, and only the latter burns the budget.

## What changes for the user

- **Gamepads count as input.** On Windows the sampler polls XInput once
  per second; a button, stick, trigger or d-pad change counts like a key
  press. A controller player is no longer "idle" after two minutes.
- **Hands-on time.** Every segment also records how much of it was
  hands-on: the union of the `input_window_s` (default 5 s) windows after
  each input. The dashboard shows both numbers for games: "Game 1h 50m in
  front · 1h 02m hands-on". Totals, exports and per-app rows carry the
  hands-on milliseconds next to the active milliseconds.
- **Rules can count hands-on time.** A third idle policy, `"input"`,
  counts a tick only while the last input (any device) is at most
  `input_window_s` ago. With `[tracking] hands_on_games = true` (default)
  rules with the Game category scope default to `"input"` instead of
  `"ignore"`; a rule can still set `idle_policy` explicitly. The Settings
  page gets a switch for the toggle and the number of seconds.
- Nothing changes for presence stats: "Active" and the per-app time in
  front stay what they were, so the two numbers can be compared.

## Config

```toml
[tracking]
# Seconds that one key press, mouse move or controller input counts as
# hands-on time. 0 disables hands-on accounting (everything counts 0).
input_window_s = 5
# Rules for the Game category count hands-on time by default
# (idle_policy = "input" instead of "ignore").
hands_on_games = true
# Poll XInput controllers (Windows) so gamepad use counts as input.
gamepad = true
```

Per rule: `idle_policy = "pause" | "ignore" | "input"`.

## Design

### Platform (`ns-platform`)

- `Platform::gamepad_idle_time(&self) -> Option<Duration>`: time since the
  last controller input, `None` when the backend has no gamepad support
  or no controller is connected. Default `None`.
- `Platform::sample()` folds it in: `idle_ms = min(idle_time,
  gamepad_idle_time)`. `Sample.idle_ms` therefore means "since the last
  input on any device". The segmenter's idle classification and the
  backdating of idle segments get this for free.
- Windows: `XInputGetState` on slots 0-3 (`Win32_UI_Input_XboxController`).
  A slot counts as input when its state changes beyond the dead zones
  (`XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE` 7849, right 8689, trigger
  threshold 30) or a button changes; `dwPacketNumber` alone is not used
  because stick drift bumps it every poll. Disconnected slots are
  re-checked every 3 s rather than every poll (the API is slow for empty
  slots). The last-change instant lives in a `Mutex` in `WindowsPlatform`.
  Steam Input and most third-party pads present as XInput; DualShock over
  plain HID does not (backlog: raw HID / GameInput).
- Linux: `None` for now (backlog: evdev `/dev/input/event*` with
  `EV_KEY`/`EV_ABS` from `ID_INPUT_JOYSTICK` devices, needs the `input`
  group).
- Mock: `MockPlatform` scripts carry `idle_ms` already, so nothing to add.

### Core (`ns-core`)

- `Config.tracking`: `input_window_s: u64` (5), `hands_on_games: bool`
  (true), `gamepad: bool` (true). `config/default.toml` documents them.
- `Activity.hands_on: bool`: true when the latest sample's `idle_ms` is
  at most `input_window_s * 1000` and the kind is `Active` or `Idle`.
  Set by `SegmenterTask`, read by the engine and `/state`.

### Tracker (`ns-tracker`)

- `SegmenterConfig.input_window_ms` (5000; 0 disables).
- `Segmenter` keeps `input_ms` for the open segment. On each sample with
  a previous sample, `dt = ts - last_ts` capped at `poll_ms + gap_ms`;
  if `idle_ms <= input_window_ms` and the segment kind is `Active`,
  `input_ms += dt`. `Action::Extend` and `Action::Close` carry
  `input_ms` (the segment's total so far), `Action::Open` starts at 0.
  Day splits carry nothing over (the new segment starts at 0; the closed
  one keeps what it had). Idle segments never accrue (they start at the
  last input by construction).
- `SegmenterTask` sets `Activity.hands_on` and passes `input_ms` to
  storage.

### Storage (`ns-storage`)

- Migration `002_hands_on.sql`: `segments.input_ms INTEGER NOT NULL
  DEFAULT 0`, `daily_app_totals.input_ms INTEGER NOT NULL DEFAULT 0`,
  `daily_kind_totals.input_ms INTEGER NOT NULL DEFAULT 0`.
- `extend_segment(id, end, input_ms)`: the delta of `input_ms` against
  the stored value is added to the totals in the same transaction as the
  end delta (never negative; clamp).
- `AppUsage`, `CategoryUsage`, `DailyUsage`, `KindTotal`, `TimelineEntry`
  and `ExportSegment` gain `input_ms`. The CSV export gets an `input_ms`
  column.

### Rules (`ns-rules`)

- `IdlePolicy::Input`. `Rule::idle_policy()` becomes
  `idle_policy(&self, hands_on_games: bool)`: explicit value, else
  Game scope → `Input` if `hands_on_games` else `Ignore`, else `Pause`.
- `Engine` gets `hands_on_games: bool`; `EngineCommand::ReplaceRules`
  stays, plus `EngineCommand::SetHandsOnGames(bool)` (or the runtime
  passes it with the rules). `engaged` for policy `Input` is
  `activity.hands_on` (kind `Active` or `Idle`; locked, suspended and
  paused never count). Reading mode still turns `Pause` into `Ignore`
  and leaves `Input` alone: reading mode is about break timers, not
  about game budgets.
- The rule snapshot's `message`/chips need nothing new; the UI reads
  `idle_policy` from `GET /rules` and the `[tracking]` keys from the
  TOML text.

### API (`ns-api`)

- `GET /state`: `activity.hands_on` (already in `Activity`).
- `GET /stats/*`: `input_ms` on every row that has `ms`.
- `GET /rules`: `idle_policy` may be `"input"`.

### UI

- Settings > Tracking card (new): switch "Count hands-on gaming only"
  (`hands_on_games`) and a number field "Seconds per input"
  (`input_window_s`), both written by editing the `[tracking]` table in
  the config text and `PUT /rules` (the same path the per-rule enable
  switches use). A note explains gamepads: XInput on Windows, keyboard
  and mouse only on Linux.
- Today: the Game tile shows hands-on time as the value and "of 1h 50m
  in front" as the subtitle when the two differ; apps table gets a
  "Hands-on" column for games. The status block says "Playing" vs "In
  front" from `activity.hands_on` when the app is a game.
- Rules: the idle chip reads "hands-on" for `input`, with the default
  computed from `hands_on_games`.
- History: unchanged for now (presence per category); backlog item to add
  a hands-on series.

## Edge cases

- **Cutscenes and loading screens** count as not hands-on. That is the
  intended reading: no input, no strain.
- **Stick drift**: the dead zones above; a pad with drift beyond 7849
  counts as input permanently. Backlog: a per-slot calibration or a
  "treat sticks as input only with buttons" switch if it shows up.
- **Mouse jitter** from a desk bump counts 5 s. Harmless.
- **Rumble / vibration** does not change the state and is not input.
- **Sleep and clock jumps**: the segmenter's gap rule closes the segment
  before the gap; the gap's `suspended` segment has no input.
- **Day boundary**: hands-on splits with the segment; daily totals stay
  per day.
- **Old databases**: migration 002 adds zero-filled columns; history
  before the change shows 0 hands-on, and the UI hides the hands-on
  number when it is 0 and the presence number is not.
- **`input_window_s = 0`**: hands-on is never set and `input` policy
  rules never count; the config loader clamps to 0..=600.

## Not in scope

- Linux gamepad input (evdev), raw HID pads on Windows, GameInput.
- Counting mouse "intensity" (clicks per minute) or distinguishing
  reading from playing in a browser.
- A separate hands-on daily limit for non-game categories (the `input`
  policy already works on any rule; only the default differs).
