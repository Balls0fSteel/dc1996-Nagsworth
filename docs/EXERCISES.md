# Break exercises: design brief

Drafted 2026-11-04_222508_1793831108. Status: built 2026-11-05_001500_1793834100 (CHANGELOG.md);
the open questions 2-5 stayed as proposed (no audio cues, no completion
tracking yet, band tier off by default, no habits); revisited
2026-11-04_231240_1793833960: audio cue (opt-in), completion tracking and a few habit items
are to be built, band tier stays off (see "Open questions"); built
2026-11-04_232332_1793834612 (CHANGELOG.md). Differences from the
plan are noted inline as "Built:".
Decided 2026-11-04_223500_1793831700: open question 1, a third default rule `move-break`
(every 30 min, 2 min, toast), is in; see "Default rules".
Meant to be handed to an implementation session as its whole task once the
open questions at the end are decided. Read AGENTS.md, PLAN.md and
CHANGELOG.md first; the repository conventions there apply.

## Goal

When a break starts, Nagsworth tells the user what to do with it instead of
the fixed line "Stand up, look away, stretch." What it suggests depends on
how much time has passed:

- the **length of the break** (a 30 s micro-break fits one eye exercise; a
  10 min rest break fits a walk plus a short routine),
- the **time sat without a real break** (the longer the stretch, the more
  the routine leans on getting up and walking, which is where the evidence
  is strongest),
- **today's total** at the desk (after a long day the lower body and a walk
  come first).

The catalogue holds only exercises with published evidence for the thing
they are meant to help (sitting-related cardiometabolic risk, neck and
shoulder pain, digital eye strain, stress), is large enough to rotate so
the same break does not repeat day after day, and stays honest about how
strong the evidence is. Not medical advice; every routine carries a "stop
if it hurts" line.

## What the evidence supports (and what it does not)

Searched 2026-11-04; the papers are listed under "Sources". Grades used in
the catalogue: **A** = meta-analysis or several RCTs, moderate certainty;
**B** = one RCT or several small controlled trials; **C** = pre/post or
single lab study, plausible mechanism, low risk. Nothing below C ships.

| Finding | Grade | What it means for the catalogue |
|---|---|---|
| Interrupting sitting with light walking lowers postprandial glucose and insulin; standing alone helps less (Buffey 2022, meta-analysis of 7 crossover trials). | A | Every break of 2 min or more gets a walk. |
| Dose: 5 min walking every 30 min was the only dose that lowered both glucose and blood pressure; 1 min every 30 or 60 min still lowered blood pressure and improved mood (Diaz 2023, n = 11 crossover). | B | The walk's length scales with time sat: 1-2 min after 30 min, 3 min after 45-60 min, 5 min after more. |
| 3 min of bodyweight squatting every 45 min lowered glucose as much as 3 min of walking and more than one 30 min walk (Zhejiang 2023, n = 18 men, CGM). | B | Squats and sit-to-stands are the indoor substitute for a walk. |
| "Exercise snacks" (stair climbing, bodyweight bouts under 1 min, several times a day) improve cardiorespiratory fitness in inactive adults (BJSM 2025 meta-analysis, moderate certainty). | A | Stair climbing and brisk bodyweight items are an opt-in tier for longer breaks. |
| Neck/shoulder strengthening at work reduces pain in symptomatic office workers (Chen 2018, 27 RCTs, SMD 0.59, GRADE moderate; Andersen trials: even 3 x 20 min or 9 x 7 min a week works). | A | Isometric neck presses, scapular squeezes and, with a band, pull-aparts and rows. Strength items are the only ones graded A for pain. |
| Active micro-breaks (stretching, strengthening) beat passive breaks for discomfort and fatigue; the trials are small and at risk of bias (Radwan 2022, Vitoulas 2022 reviews). | B | Stretches stay in, graded B, and never make up a whole routine on their own. |
| Short regular exercise reduces low back pain in office workers; 10-15 min on 3-5 days is suggested as enough (Gobbo 2019 review). | B | Hip flexor, thoracic extension and glute items in the longer routines. |
| Deep neck flexor training reduces forward head posture; low to moderate certainty, small trials (FHP meta-analyses). | B | Chin tucks, graded B, few reps. |
| 20-20-20 reminders reduced dry eye and eye strain symptoms in two weeks; signs did not change and the effect faded after the reminders stopped (Talens-Estarelles 2022, n = 29 pre/post). A 20 s break alone did not help in Johnson & Rosenfield. | C | Far focus is the default micro-break item, labelled for symptoms, not vision. |
| Complete blinks matter more than blink rate; close-squeeze-open x 15, three times a day, reduced incomplete blinks and symptoms while practised (Aston 2025 randomized routine comparison). | B | Blink squeezes as a micro-break item. |
| 5 min of cyclic sighing daily improved mood and lowered respiratory rate more than mindfulness (Balban 2023 RCT). | B | One breathing item, offered at the end of longer routines and as a micro-break option. |
| ACSM: static stretches held 10-30 s to mild tightness, 2-4 repetitions. | guideline | Every stretch in the catalogue uses 20-30 s holds, no bouncing. |

Left out on purpose:

- **Soleus push-ups** as a headline item: one lab study by one group with a
  sitting-only comparator, 4.5 h of continuous contractions and no
  replication. Calf raises stay as an ordinary standing item.
- **Palming, eye rolling, "vision training"**: no evidence for screen use.
- **Neck circles, ballistic stretches, loaded spinal flexion**: discouraged
  in the physiotherapy literature and not needed.
- **Nerve gliding**: evidence only for people who already have carpal tunnel
  syndrome, as an add-on to treatment; nothing on prevention.
- Anything that needs the floor by default (glute bridges, planks): available
  behind the `floor` equipment flag only.

## Catalogue

Shipped as `config/exercises.toml`, `include_str!`'d like `known_games.toml`,
with an optional user file `<data dir>\exercises.toml` whose entries override
or add by `id` (`enabled = false` hides a shipped one). Fields:

```toml
[[exercise]]
id = "far-focus"
name = "Look far away"
tags = ["eyes"]               # body region or purpose, see below
posture = "seated"            # seated | standing | floor
seconds = 20                  # one set, including the hold
reps = "20 s"                 # shown next to the name
equipment = []                # "band", "stairs", "floor", "chair" (default available)
cue = "Look at something at least 6 m away for 20 seconds."   # toast one-liner
steps = [                     # overlay: 1-4 short lines
  "Look out of a window or across the room.",
  "Pick the farthest object you can see and keep your eyes on it.",
  "Blink normally.",
]
evidence = "C"
why = "Reduced eye strain and dry eye symptoms in computer users in two weeks (Talens-Estarelles 2022)."
```

Tags double as body regions for rotation: `eyes`, `neck`, `shoulders`,
`upper_back`, `wrists`, `lower_back`, `hips`, `legs`, `walk`, `cardio`,
`breathing`.

### Micro items (20-45 s, seated, toast or one overlay step)

| id | name | region | dose | grade |
|---|---|---|---|---|
| `far-focus` | Look far away | eyes | 20 s at 6 m+ | C |
| `blink-squeeze` | Full blinks | eyes | close, squeeze 2 s, open, x 10 | B |
| `near-far` | Near-far focus | eyes | thumb at 30 cm, then far, x 10 | C |
| `chin-tuck` | Chin tucks | neck | 8 x 3 s hold | B |
| `neck-iso` | Neck presses | neck | palm on forehead, then each side, push 5 s, x 3 | A |
| `upper-trap-stretch` | Ear to shoulder | neck | 20 s each side | B |
| `scapular-squeeze` | Shoulder blade squeezes | shoulders | 10 x 3 s | A |
| `shoulder-rolls` | Shoulder rolls back | shoulders | 10 slow | B |
| `seated-twist` | Seated spine twist | upper_back | 20 s each side | B |
| `wrist-flexor` | Wrist flexor stretch | wrists | arm straight, palm up, 20 s each | B |
| `wrist-extensor` | Wrist extensor stretch | wrists | palm down, 20 s each | B |
| `finger-spread` | Finger spread and fist | wrists | 10 slow | C |
| `sit-stand` | Sit to stand | legs | 5 slow, no hands | B |
| `cyclic-sigh` | Cyclic sighing | breathing | double inhale through the nose, long exhale, x 5 | B |

### Standing items (45 s to 3 min)

| id | name | region | dose | grade |
|---|---|---|---|---|
| `walk` | Walk | walk | 1, 2, 3 or 5 min (planner sets it) | A |
| `stairs` | Stair climb | cardio | 1-2 min, `stairs` flag | A |
| `squats` | Bodyweight squats | legs | 10-15, to a chair if needed | B |
| `calf-raises` | Calf raises | legs | 15-20 | B |
| `hip-flexor-stretch` | Hip flexor stretch | hips | half-kneel or standing lunge, 30 s each | B |
| `standing-hamstring` | Hamstring stretch | hips | heel on the chair, 30 s each | B |
| `thoracic-extension` | Upper back extension | upper_back | hands behind head, lean back over the chair top, 10 | B |
| `wall-angels` | Wall slides | shoulders | back to the wall, arms up and down, 10 | B |
| `doorway-chest` | Doorway chest stretch | shoulders | forearms on the frame, 30 s | B |
| `side-bend` | Standing side bend | lower_back | 20 s each side | B |
| `band-pull-apart` | Band pull-aparts | shoulders | 15, `band` flag | A |
| `band-row` | Band rows | upper_back | 15, `band` flag | A |
| `glute-bridge` | Glute bridges | hips | 15, `floor` flag | B |
| `bird-dog` | Bird dog | lower_back | 8 each side, `floor` flag | B |

28 items; 23 without any flag (stairs, the two band items and the two floor
items need one). That gives a 10 min routine dozens of distinct
region-balanced combinations before anything repeats, and about a week of
different micro-breaks at 20 min intervals.

## Picking a routine

A pure planner in a new module `crates/ns-rules/src/exercises/`
(`catalog.rs`: parse and validate the TOML; `planner.rs`: selection;
`rotation.rs`: persisted state), tested with fixed seeds and `MockClock`.
It is pure so the engine can call it from `StartBreakNow` and the tick that
starts a break:

```rust
pub struct PlanInput {
    pub break_ms: i64,          // the rule's break_for
    pub sat_ms: i64,            // continuous time since the last rest >= break_for
    pub today_active_ms: i64,   // the day's active time so far
    pub rule_tags: Option<Vec<Tag>>,  // per-rule focus, e.g. ["eyes"]
    pub equipment: Vec<Equipment>,
    pub exclude: Vec<ExerciseId>,
    pub seed: u64,              // day + rule id + nth break today
}
pub struct Routine { pub steps: Vec<Step>, pub note: Option<String> }
pub struct Step { pub id: ExerciseId, pub seconds: u32, pub reps: String }
```

Rules, in order:

1. **Budget.** The routine's step seconds add up to at most `break_ms`,
   leaving 10% slack for reading. Below 60 s: exactly one micro item.
   60 s to 4 min: a walk if `sat_ms >= 30 min`, otherwise two or three
   standing items. 5 min and more: walk first, then items, breathing last.
2. **Walk length by time sat** (Diaz 2023): `sat_ms < 30 min` → 1 min,
   `< 60 min` → 2 min, `< 90 min` → 3 min, otherwise 5 min, capped at half
   the break. The routine's `note` says why: "2 h 10 min without a real
   break: walk first." Squats or sit-to-stands replace the walk when the
   rule's tags say `indoor` or the user turned walks off.
3. **One item per region** inside a routine, and a routine of 3+ items
   covers at least one of `neck|shoulders|upper_back`, one of
   `hips|legs|lower_back` and one of `eyes` when the rule has no tags.
4. **Rotation.** Among eligible items, weight = 1 + days since last shown
   (never shown = 8), halved for the item shown in the previous break of
   the same rule. Pick with a seeded `rand::rngs::SmallRng`. The same micro
   item never shows twice in a row.
5. **Day load.** When `today_active_ms > 4 h`, items tagged `legs|hips|walk`
   get double weight (the Chen and Buffey findings favour the lower body
   and walking once the day is long).
6. **Evidence first.** Within a region, grade A items get 1.5x weight so
   the strength items come around more often than the stretches.

Rotation state (`last_shown: BTreeMap<ExerciseId, LocalDay>`, `last_by_rule`)
is persisted in `settings` under `exercises.rotation` (JSON) through the
storage actor, like the rule counters. It is a hint, not a counter: losing
it only changes variety.

Built: the planner is `crates/ns-rules/src/exercises/{catalog,planner,rotation}.rs`
with `ExerciseSetup` holding the three together; the engine plans when a
break rule's nag is *raised* (so the toast can show the first item's cue)
and reuses that routine when the break starts; a break started from the
tray plans at start. The seed is a hash of the rule id and the timestamp.
The walk is capped at 60% of the budget, so a 2 min move break walks 64 s
and a 10 min rest break after an hour walks the full 5 min. `today_active_ms`
is the engine's own count of active ticks since the day start (any scope).

## Where it plugs in

- **ns-core**: `NagEvent::BreakStarted` gains `routine: Option<Routine>`;
  `Config` gains `[exercises]` (`enabled`, `equipment`, `exclude`,
  `walks = true`); `Rule` gains `exercises: Option<RuleExercises>`
  (`false` to opt out, or `{ tags = ["eyes"] }`). `Routine` and `Step` live
  in ns-core so the API and the Tauri shell see them without depending on
  the planner.
- **ns-rules**: the engine takes a `Planner` (trait object, `NoPlanner` in
  tests that do not care) and fills `routine` when a break starts; `sat_ms`
  is the rule's `accum_ms` for interval and continuous rules, and the time
  since the last `BreakTaken` dismissal for the others.
- **ns-storage**: `nag_events` gets a `routine` TEXT column (JSON, nullable,
  migration 003) so the history page can show what was suggested; the
  rotation state goes in `settings`.
- **ns-api**: `GET /v1/exercises` returns the merged catalogue (for the
  Settings page); `/state`'s engine snapshot carries the current routine
  when a break runs; `/nags` rows include it. The extension popup is out of
  scope. Built: `GET /v1/exercises` returns `{exercises, config}`;
  `RuleSnapshot.routine` is set while the nag is open or the break runs.
- **ns-app** `nag_bridge.rs`: `NagView` gains `routine`; the overlay gets
  it with `nags-changed`. Built: step advancing lives entirely in the
  overlay page (no `"next"` command): steps run on their own seconds from
  the break's start, and "Next" or Space restarts the schedule from the
  next step at that moment.
- **UI** `Nag.svelte`:
  - Overlay: ring timer on the left as now; on the right the routine as a
    list, the current step large with its own countdown and its 1-4 lines,
    the rest dimmed. Auto-advance when a step's seconds run out; Space or
    "Next" skips ahead; the `note` sits under the rule name; "Stop if it
    hurts" in small print; Esc still ends the break. The overlay is per
    monitor, so the list renders on each.
  - Toast (micro-breaks): the item's `cue` under the message with a 20 s
    countdown after "Start break" is clicked, instead of nothing. Built: the
    toast shows "Try: <name> · <reps>" and the cue before the break starts;
    a started break always opens the overlay (existing behaviour), which
    shows the single step with its countdown.
  - Settings > Exercises card: enable, equipment checkboxes (band, stairs,
    floor), walks on/off, the catalogue as a list with per-item toggles
    (writes `exclude`), each item expandable to its steps, grade and `why`.
  - Rules page: a chip "eyes" / "no exercises" on rule cards that set
    `exercises`.
- **Tray / dashboard**: nothing new; "Start break now" already reaches the
  engine.

### Default rules (decided 2026-11-04_223500_1793831700)

Three break rules ship in `config/default.toml`, spaced so they line up
with the evidence (light activity every 30 min, a longer rest every hour):

```toml
[[rules]]
id = "micro-break"
name = "Micro-break: eyes and neck"
trigger = { kind = "interval", every = "20m", break_for = "30s" }
escalation = [ { after = "0s", level = "toast" } ]
exercises = { tags = ["eyes", "neck", "shoulders", "wrists"] }   # stays at the desk

[[rules]]
id = "move-break"
name = "Move: get up for two minutes"
trigger = { kind = "interval", every = "30m", break_for = "2m" }
escalation = [ { after = "0s", level = "toast" } ]
exercises = { tags = ["walk", "legs", "hips"] }   # a walk, squats or sit-to-stands

[[rules]]
id = "rest-break"
name = "Take a 10 minute break"
trigger = { kind = "interval", every = "60m", break_for = "10m" }
scope = "any"
idle_policy = "pause"
escalation = [ { after = "0s", level = "toast" }, { after = "90s", level = "overlay" } ]
snooze = { duration = "5m", max_per_day = 3 }
# no exercises key: the full planner (walk first, then a region-balanced routine)
```

`micro-break` moves from every 10 min to every 20 min (the 20-20-20
spacing) so the three rules do not fire within minutes of each other;
`rest-break` moves from 50 to 60 min. Because rest resets every interval
rule whose `break_for` is covered, a 10 min rest break also satisfies the
other two, and a 2 min move break satisfies the micro-break: the engine's
existing "lock/idle time counts as rest" semantics already do this. When
two breaks start together the bridge already shows one countdown (the
longest) and demotes the other to a toast. Existing installs keep their
own `config.toml`; the new rule is only in the shipped defaults, noted in
the changelog and in docs/MANUAL_CHECKS.md.

## Work items

0. `config/default.toml`: the three default rules above; the rule tests
   in `crates/ns-rules/src/rule.rs` that read the default config
   (`micro-break` expectations) follow.
1. `config/exercises.toml` with the 28 items above, each with `cue`,
   `steps`, `evidence`, `why`; a validation test (unique ids, seconds > 0,
   known tags, every flag known, steps 1-4 lines, `why` cites a source
   listed in this document).
2. `ns-rules::exercises`: catalogue loading with user overrides, planner,
   rotation; tests for the budget rule at 30 s / 2 min / 10 min, walk
   scaling at 20 / 45 / 75 / 120 min sat, region coverage, no repeat in a
   row, seeded determinism, excluded and flagged items never appear.
3. Engine integration and `NagEvent::BreakStarted.routine`; storage
   migration 003 and the rotation setting; API endpoint and snapshot field.
4. Overlay and toast UI; Settings card; rule chip. `npm run ui:check`.
5. docs: this file's status line, ARCHITECTURE.md (new module), the config
   comments in `config/default.toml`, MANUAL_CHECKS.md (a 10 s break on the
   hidden desktop: capture the overlay per monitor with `run_hidden.ps1
   -Screenshot`; the toast needs `PrintWindow`, see AGENTS.md).
6. `tools/seed_demo.py`: a few nag events with routines for the history
   page.

Acceptance: `cargo test --workspace`, clippy `-D warnings`, fmt, `ui:check`
clean; a hidden-desktop run with `break_for = "10s"` shows a one-item
routine, and with `break_for = "10m"` after a seeded 2 h of activity shows a
5 min walk first; the Linux cross-check still passes (no platform code
touched).

## Open questions

1. ~~A third default rule?~~ Decided 2026-11-04_223500_1793831700: yes, see "Default
   rules" above.
2. **Audio cues** for step changes on the overlay (the app already beeps for
   deferred overlays). Decided 2026-11-04_231240_1793833960: a soft tick on step change behind
   a Settings switch, off by default. Built: `[exercises] step_sound`, a
   WebAudio tick played by the first overlay window only.
3. **Completion tracking**: record whether the user let the routine run or
   ended early, and show "breaks taken with exercises" on History. Decided
   2026-11-04_231240_1793833960: build it now (was "later"). Built: Esc during a
   break now ends it (`DismissReason::EndedEarly`; the engine used to ignore
   `SkipBreak` while a break ran), `GET /v1/stats/breaks` pairs each
   `break_started` with its dismiss, and History has a "Breaks" card
   (started, ran to the end, ended early, with exercises).
4. **Band tier on by default?** The strength items have the best evidence
   for pain, but need a band at the desk. Decided 2026-11-04_231240_1793833960: off until the
   user ticks "I have a resistance band" (as built).
5. **Drink water** and similar non-exercise habits. Decided 2026-11-04_231240_1793833960: add
   two or three items under a `habits` tag (drink water, look out of a
   window), off by default. Built: `drink-water` and `daylight` (grade C,
   tag `habits`), `[exercises] habits`; they ignore a rule's tags in breaks
   of a minute or more and every long break gets one.

## Sources

- Buffey AJ et al. The acute effects of interrupting prolonged sitting with
  standing and light-intensity walking on cardiometabolic biomarkers:
  systematic review and meta-analysis. Sports Med 2022.
  https://pubmed.ncbi.nlm.nih.gov/35147898/
- Duran AT, Diaz KM et al. Breaking up prolonged sitting to improve
  cardiometabolic risk: dose-response analysis of a randomized crossover
  trial. Med Sci Sports Exerc 2023. DOI 10.1249/MSS.0000000000003109.
  https://www.cuimc.columbia.edu/node/25711
- Zhejiang University trial, 3 min squatting or walking every 45 min vs one
  30 min walk, CGM, 2023. ISRCTN15716578.
  https://www.isrctn.com/ISRCTN15716578
- Exercise snacks and cardiorespiratory fitness in inactive adults:
  systematic review and meta-analysis. Br J Sports Med 2025.
  https://bmjgroup.com/?p=13475
- Chen X et al. Workplace-based interventions for neck pain in office
  workers: systematic review and meta-analysis. Phys Ther 2018.
  https://research-repository.griffith.edu.au/items/32398aae-fcdb-496c-8ef5-b23db68ad6a3
- Andersen CH et al. Effect of scheduled neck/shoulder strength training
  (1x60, 3x20, 9x7 min/week) in office workers. Br J Sports Med 2012.
  https://bjsm.bmj.com/content/46/14/1004
- Resistance exercise training in sedentary office workers: systematic
  review and meta-analysis. J Occup Rehabil 2025.
  https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12906527/
- Radwan A et al. Active micro-breaks in sedentary work: systematic review.
  Cogent Engineering 2022. https://doaj.org/article/b3c2d7ac5b084dbfb2253dd67f51a356
- Vitoulas S et al. Active micro-break physiotherapy for workers:
  systematic review of 13 RCTs. Healthcare 2022.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC9603092
- Gobbo S et al. Physical exercise is confirmed to reduce low back pain
  symptoms in office workers: systematic review. J Funct Morphol Kinesiol
  2019. https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7739349/
- Deep neck flexor strengthening on forward head posture: systematic review
  and meta-analysis (4 studies; evidence insufficient on its own).
  https://archive.org/download/effect-of-deep-neck-flexor-strengthening-on-forward-head-posture-a-systemic-review-and-meta-analyses/10.4328ACAM.20258.pdf
- Talens-Estarelles C et al. The effects of breaks on digital eye strain,
  dry eye and binocular vision: testing the 20-20-20 rule. Cont Lens
  Anterior Eye 2022. https://publications.aston.ac.uk/id/eprint/44152
- Aston University blink exercise routine comparison, 2025 (close-squeeze-
  blink x 15, three times a day). https://www.eurekalert.org/news-releases/1088216
- Balban MY et al. Brief structured respiration practices enhance mood and
  reduce physiological arousal. Cell Rep Med 2023.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC9873947
- ACSM stretching and flexibility guidelines update, 2021 (10-30 s holds).
  https://www.acsm.org/blog-detail/acsm-certified-blog/2021/03/18/stretching-and-flexibility-guidelines-update
- Hamilton MT et al. soleus push-up, iScience 2022, and the authors' 2025
  perspective (why it is not a headline item).
  https://www.frontiersin.org/journals/endocrinology/articles/10.3389/fendo.2025.1601474/full
- Ballestero-Pérez R et al. Effectiveness of nerve gliding exercises on
  carpal tunnel syndrome: systematic review. J Manipulative Physiol Ther
  2017 (treatment only, limited evidence).
  https://produccioncientifica.ucm.es/documentos/5df8a5992999525886b6ee70
