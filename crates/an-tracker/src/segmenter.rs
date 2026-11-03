//! Samples in, segments out.
//!
//! [`Segmenter`] is a pure state machine: feed it samples, get back
//! [`Action`]s (open / extend / close). [`SegmenterTask`] runs it against
//! storage, the event bus and the activity watch channel.
//!
//! Rules:
//! - A new segment starts when (kind, app, title) changes.
//! - Switches between active windows only count once the new window has
//!   stayed in front for `min_segment_ms`; shorter flaps are merged into the
//!   surrounding segment.
//! - Going idle is backdated to the last input.
//! - A gap between samples longer than the poll interval plus 5 s becomes a
//!   `suspended` segment (sleep, hibernate, a stalled process).
//! - Segments are split at the local day start so each belongs to one day.
//! - The open segment is written to storage at least every `flush_ms`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use an_core::config::StoreTitles;
use an_core::{
    Activity, AppRef, Clock, Config, ControlCommand, Event, EventBus, LocalDay, ProcessInfo,
    Sample, SegmentKind, SessionState, TimeOfDay, Timestamp, TrackerEvent,
};
use an_platform::Platform;
use an_storage::StorageHandle;
use tokio::sync::{broadcast, mpsc, watch};

use crate::Categorizer;

#[derive(Clone, Debug)]
pub struct SegmenterConfig {
    pub poll_ms: u64,
    pub idle_after_ms: u64,
    pub min_segment_ms: u64,
    pub store_titles: bool,
    pub day_start: TimeOfDay,
    pub gap_ms: u64,
    pub flush_ms: u64,
}

impl SegmenterConfig {
    pub fn from_config(c: &Config) -> Self {
        Self {
            poll_ms: c.tracking.poll_ms,
            idle_after_ms: c.tracking.idle_after_s * 1000,
            min_segment_ms: c.tracking.min_segment_ms,
            store_titles: c.tracking.store_titles == StoreTitles::Full,
            day_start: c.general.day_start,
            ..Self::default()
        }
    }
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            poll_ms: 1000,
            idle_after_ms: 120_000,
            min_segment_ms: 2000,
            store_titles: true,
            day_start: TimeOfDay::new(4, 0).expect("valid time"),
            gap_ms: 5000,
            flush_ms: 10_000,
        }
    }
}

/// What a segment is about. Two samples with equal keys belong to the same segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegKey {
    pub kind: SegmentKind,
    /// Lower-cased exe name, for comparison.
    pub exe: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Open {
        kind: SegmentKind,
        process: Option<ProcessInfo>,
        title: Option<String>,
        start: Timestamp,
        day: LocalDay,
    },
    Extend {
        end: Timestamp,
    },
    Close {
        end: Timestamp,
    },
}

#[derive(Clone, Debug)]
struct Observed {
    key: SegKey,
    process: Option<ProcessInfo>,
}

#[derive(Clone, Debug)]
struct Current {
    key: SegKey,
    process: Option<ProcessInfo>,
    start: Timestamp,
    end: Timestamp,
    day: LocalDay,
}

#[derive(Debug)]
pub struct Segmenter {
    cfg: SegmenterConfig,
    current: Option<Current>,
    pending: Option<(Observed, Timestamp)>,
    last_ts: Option<Timestamp>,
    last_flush: Timestamp,
    paused: bool,
}

impl Segmenter {
    pub fn new(cfg: SegmenterConfig) -> Self {
        Self {
            cfg,
            current: None,
            pending: None,
            last_ts: None,
            last_flush: 0,
            paused: false,
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// The kind a sample stands for, before any debouncing.
    pub fn classify(&self, s: &Sample) -> SegmentKind {
        if self.paused {
            SegmentKind::Paused
        } else {
            match s.session {
                SessionState::Locked => SegmentKind::Locked,
                SessionState::Suspended => SegmentKind::Suspended,
                SessionState::Active if s.idle_ms >= self.cfg.idle_after_ms => SegmentKind::Idle,
                SessionState::Active => SegmentKind::Active,
            }
        }
    }

    fn observe(&self, s: &Sample) -> Observed {
        let kind = self.classify(s);
        let (process, title) = match (&s.fg, kind) {
            (Some(w), SegmentKind::Active) => (
                Some(w.process.clone()),
                self.cfg.store_titles.then(|| w.title.clone()),
            ),
            _ => (None, None),
        };
        Observed {
            key: SegKey {
                kind,
                exe: process.as_ref().map(|p| p.exe_name.to_ascii_lowercase()),
                title,
            },
            process,
        }
    }

    fn day(&self, ts: Timestamp, offset_s: i32) -> LocalDay {
        LocalDay::from_ts(ts, offset_s, self.cfg.day_start)
    }

    fn open(&mut self, o: Observed, start: Timestamp, offset_s: i32, out: &mut Vec<Action>) {
        let day = self.day(start, offset_s);
        out.push(Action::Open {
            kind: o.key.kind,
            process: o.process.clone(),
            title: o.key.title.clone(),
            start,
            day,
        });
        self.current = Some(Current {
            key: o.key,
            process: o.process,
            start,
            end: start,
            day,
        });
        self.last_flush = start;
    }

    fn close(&mut self, end: Timestamp, out: &mut Vec<Action>) {
        if let Some(c) = self.current.take() {
            out.push(Action::Close {
                end: end.max(c.start),
            });
        }
        self.pending = None;
    }

    /// Feeds one sample; `offset_s` is the local UTC offset at the sample.
    pub fn on_sample(&mut self, s: &Sample, offset_s: i32) -> Vec<Action> {
        let mut out = Vec::new();
        let observed = self.observe(s);

        // Clock jump: the machine slept or the process stalled.
        if let Some(last) = self.last_ts
            && s.ts - last > (self.cfg.poll_ms + self.cfg.gap_ms) as i64
        {
            self.close(last, &mut out);
            let gap = Observed {
                key: SegKey {
                    kind: SegmentKind::Suspended,
                    exe: None,
                    title: None,
                },
                process: None,
            };
            self.open(gap, last, offset_s, &mut out);
            self.close(s.ts, &mut out);
        }
        self.last_ts = Some(s.ts);

        // Day boundary: split the open segment at the new day's start.
        if let Some(c) = &self.current {
            let today = self.day(s.ts, offset_s);
            if today != c.day {
                let boundary = today.start_ts(offset_s, self.cfg.day_start).max(c.start);
                let carry = Observed {
                    key: c.key.clone(),
                    process: c.process.clone(),
                };
                self.close(boundary, &mut out);
                self.open(carry, boundary, offset_s, &mut out);
            }
        }

        let Some(cur) = &self.current else {
            self.open(observed, s.ts, offset_s, &mut out);
            return out;
        };

        if observed.key == cur.key {
            self.pending = None;
        } else {
            let from = cur.key.kind;
            let to = observed.key.kind;
            if to == SegmentKind::Idle && from != SegmentKind::Idle {
                // Backdate to the last input, but not before the segment began.
                let at = (s.ts - s.idle_ms as i64).max(cur.start);
                self.close(at, &mut out);
                self.open(observed, at, offset_s, &mut out);
                return out;
            }
            if from != SegmentKind::Active || to != SegmentKind::Active {
                self.close(s.ts, &mut out);
                self.open(observed, s.ts, offset_s, &mut out);
                return out;
            }
            // Active window switch: debounce.
            match &self.pending {
                Some((p, since)) if p.key == observed.key => {
                    let since = *since;
                    if s.ts - since >= self.cfg.min_segment_ms as i64 {
                        self.close(since, &mut out);
                        self.open(observed, since, offset_s, &mut out);
                        // Time since the switch already belongs to the new segment.
                        self.extend_to(s.ts, &mut out);
                        return out;
                    }
                }
                _ => self.pending = Some((observed, s.ts)),
            }
        }

        let end = self
            .pending
            .as_ref()
            .map(|(_, since)| *since)
            .unwrap_or(s.ts);
        self.extend_to(end, &mut out);
        out
    }

    fn extend_to(&mut self, end: Timestamp, out: &mut Vec<Action>) {
        if let Some(c) = &mut self.current {
            c.end = c.end.max(end);
            if c.end - self.last_flush >= self.cfg.flush_ms as i64 {
                out.push(Action::Extend { end: c.end });
                self.last_flush = c.end;
            }
        }
    }

    /// Closes the open segment, e.g. on shutdown.
    pub fn finish(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(end) = self.current.as_ref().map(|c| c.end) {
            self.close(end, &mut out);
        }
        out
    }
}

/// Runs a [`Segmenter`] against storage, the bus and an activity channel.
pub struct SegmenterTask {
    pub segmenter: Segmenter,
    pub storage: StorageHandle,
    pub bus: EventBus,
    pub clock: Arc<dyn Clock>,
    pub platform: Arc<dyn Platform>,
    pub categorizer: Categorizer,
    pub activity: watch::Sender<Activity>,
}

struct CachedApp {
    app: AppRef,
    fetched: Instant,
}

const APP_CACHE_TTL: Duration = Duration::from_secs(30);

impl SegmenterTask {
    pub async fn run(mut self, mut samples: mpsc::Receiver<Sample>) {
        let mut control = self.bus.subscribe();
        let mut cache: HashMap<String, CachedApp> = HashMap::new();
        let mut current_id: Option<i64> = None;
        let mut paused_until: Option<Timestamp> = None;
        let mut last_fg: Option<(Option<String>, Option<String>)> = None;
        let mut last_kind: Option<SegmentKind> = None;

        loop {
            tokio::select! {
                sample = samples.recv() => {
                    let Some(sample) = sample else { break };
                    if let Some(until) = paused_until && sample.ts >= until {
                        paused_until = None;
                        self.segmenter.set_paused(false);
                    }
                    let offset = self.clock.offset_s(sample.ts);
                    let actions = self.segmenter.on_sample(&sample, offset);
                    for action in actions {
                        self.apply(action, &mut current_id, &mut cache).await;
                    }
                    let kind = self.segmenter.classify(&sample);
                    let app = match &sample.fg {
                        Some(w) => self.app_ref(&w.process, &mut cache).await,
                        None => None,
                    };
                    let title = sample.fg.as_ref().map(|w| w.title.clone());
                    let fg_key = (app.as_ref().map(|a| a.exe_name.clone()), title.clone());
                    if last_fg.as_ref() != Some(&fg_key) {
                        self.bus.publish(Event::Tracker(TrackerEvent::ForegroundChanged {
                            app: app.clone(),
                            title: title.clone(),
                            at: sample.ts,
                        }));
                        last_fg = Some(fg_key);
                    }
                    match (last_kind, kind) {
                        (Some(k), SegmentKind::Idle) if k != SegmentKind::Idle => {
                            self.bus.publish(Event::Tracker(TrackerEvent::IdleStarted {
                                at: sample.ts - sample.idle_ms as i64,
                            }));
                        }
                        (Some(SegmentKind::Idle), k) if k != SegmentKind::Idle => {
                            self.bus.publish(Event::Tracker(TrackerEvent::IdleEnded { at: sample.ts }));
                        }
                        _ => {}
                    }
                    last_kind = Some(kind);
                    self.activity.send_replace(Activity {
                        ts: sample.ts,
                        kind: Some(kind),
                        app,
                        title,
                        idle_ms: sample.idle_ms,
                        is_fullscreen: sample.fg.as_ref().is_some_and(|w| w.is_fullscreen),
                    });
                }
                event = control.recv() => {
                    match event {
                        Ok(Event::Control(ControlCommand::PauseTracking { for_ms })) => {
                            self.segmenter.set_paused(true);
                            paused_until = for_ms.map(|ms| self.clock.now() + ms as i64);
                        }
                        Ok(Event::Control(ControlCommand::ResumeTracking)) => {
                            self.segmenter.set_paused(false);
                            paused_until = None;
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                }
            }
        }
        for action in self.segmenter.finish() {
            self.apply(action, &mut current_id, &mut cache).await;
        }
        // Wait for queued writes so a shutdown does not lose the last segment.
        let _ = self.storage.call(|_| Ok(())).await;
    }

    async fn app_ref(
        &self,
        process: &ProcessInfo,
        cache: &mut HashMap<String, CachedApp>,
    ) -> Option<AppRef> {
        let key = process.exe_name.to_ascii_lowercase();
        if let Some(c) = cache.get(&key)
            && c.fetched.elapsed() < APP_CACHE_TTL
        {
            return Some(c.app.clone());
        }
        let steam = self.platform.steam_info();
        let (category, is_game) = self.categorizer.categorize(process, steam.as_ref());
        let p = process.clone();
        let now = self.clock.now();
        match self
            .storage
            .call(move |db| db.get_or_create_app(&p, category, is_game, now))
            .await
        {
            Ok(row) => {
                // Name the running Steam game after its manifest once.
                if row.display_name.is_none()
                    && let Some(steam) = &steam
                    && let Some(path) = &steam.steam_path
                    && crate::gaming::SteamDetector.detect(process, Some(steam))
                    && let Some(name) =
                        crate::gaming::steam::app_name(Path::new(path), steam.running_app_id)
                {
                    let id = row.id;
                    self.storage
                        .send(move |db| db.set_display_name_if_empty(id, &name));
                }
                let app = AppRef {
                    id: row.id,
                    exe_name: row.exe_name,
                    category: row.category_kind,
                    is_game: row.is_game,
                };
                cache.insert(
                    key,
                    CachedApp {
                        app: app.clone(),
                        fetched: Instant::now(),
                    },
                );
                Some(app)
            }
            Err(e) => {
                tracing::warn!("cannot store app {}: {e}", process.exe_name);
                None
            }
        }
    }

    async fn apply(
        &self,
        action: Action,
        current_id: &mut Option<i64>,
        cache: &mut HashMap<String, CachedApp>,
    ) {
        match action {
            Action::Open {
                kind,
                process,
                title,
                start,
                day,
            } => {
                let app = match &process {
                    Some(p) => self.app_ref(p, cache).await,
                    None => None,
                };
                let app_id = app.as_ref().map(|a| a.id);
                let t = title.clone();
                let result = self
                    .storage
                    .call(move |db| {
                        let title_id = match t.as_deref() {
                            Some(t) if !t.is_empty() => Some(db.title_id(t)?),
                            _ => None,
                        };
                        db.open_segment(app_id, title_id, kind, start, day)
                    })
                    .await;
                match result {
                    Ok(id) => {
                        *current_id = Some(id);
                        self.bus
                            .publish(Event::Tracker(TrackerEvent::SegmentOpened {
                                id,
                                kind,
                                app,
                                title,
                                start,
                            }));
                    }
                    Err(e) => {
                        tracing::warn!("cannot open segment: {e}");
                        *current_id = None;
                    }
                }
            }
            Action::Extend { end } => {
                if let Some(id) = *current_id {
                    self.storage.send(move |db| db.extend_segment(id, end));
                    self.bus
                        .publish(Event::Tracker(TrackerEvent::SegmentTick { id, end }));
                }
            }
            Action::Close { end } => {
                if let Some(id) = current_id.take() {
                    self.storage.send(move |db| db.extend_segment(id, end));
                    self.bus
                        .publish(Event::Tracker(TrackerEvent::SegmentClosed { id, end }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use an_core::{MockClock, ProcessInfo, WindowInfo};
    use an_platform::MockPlatform;
    use an_platform::mock::{sample, window};

    use super::*;

    fn cfg() -> SegmenterConfig {
        SegmenterConfig {
            day_start: TimeOfDay::MIDNIGHT,
            ..SegmenterConfig::default()
        }
    }

    fn w(exe: &str) -> Option<WindowInfo> {
        Some(window(exe, exe))
    }

    fn opens(actions: &[Action]) -> Vec<(SegmentKind, Option<String>, Timestamp)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Open {
                    kind,
                    process,
                    start,
                    ..
                } => Some((
                    *kind,
                    process.as_ref().map(|p: &ProcessInfo| p.exe_name.clone()),
                    *start,
                )),
                _ => None,
            })
            .collect()
    }

    fn run(seg: &mut Segmenter, samples: &[Sample]) -> Vec<Action> {
        samples.iter().flat_map(|s| seg.on_sample(s, 0)).collect()
    }

    #[test]
    fn three_samples_two_segments() {
        let mut seg = Segmenter::new(SegmenterConfig {
            min_segment_ms: 0,
            ..cfg()
        });
        let mut actions = run(
            &mut seg,
            &[
                sample(0, w("a.exe"), 0),
                sample(1000, w("b.exe"), 0),
                sample(2000, w("b.exe"), 0),
            ],
        );
        actions.extend(seg.finish());
        let o = opens(&actions);
        assert_eq!(o.len(), 2, "{actions:?}");
        assert_eq!(o[0], (SegmentKind::Active, Some("a.exe".into()), 0));
        assert_eq!(o[1], (SegmentKind::Active, Some("b.exe".into()), 1000));
        assert_eq!(actions.last(), Some(&Action::Close { end: 2000 }));
    }

    #[test]
    fn short_flaps_are_merged() {
        let mut seg = Segmenter::new(cfg());
        let actions = run(
            &mut seg,
            &[
                sample(0, w("a.exe"), 0),
                sample(1000, w("b.exe"), 0), // flap, 1 s
                sample(2000, w("a.exe"), 0),
                sample(3000, w("c.exe"), 0), // real switch
                sample(4000, w("c.exe"), 0),
                sample(5000, w("c.exe"), 0),
            ],
        );
        let o = opens(&actions);
        assert_eq!(o.len(), 2, "{actions:?}");
        assert_eq!(o[1], (SegmentKind::Active, Some("c.exe".into()), 3000));
        assert!(actions.contains(&Action::Close { end: 3000 }));
    }

    #[test]
    fn idle_is_backdated_and_gap_becomes_suspended() {
        let mut seg = Segmenter::new(SegmenterConfig {
            idle_after_ms: 60_000,
            ..cfg()
        });
        let mut samples = vec![sample(0, w("a.exe"), 0)];
        samples.push(sample(70_000, w("a.exe"), 65_000)); // gap > 6 s -> suspended
        samples.push(sample(71_000, w("a.exe"), 66_000));
        let actions = run(&mut seg, &samples);
        let o = opens(&actions);
        assert_eq!(o[1], (SegmentKind::Suspended, None, 0));
        // After the gap the sample is already idle; idle is not backdated into the gap.
        assert_eq!(o[2], (SegmentKind::Idle, None, 70_000));
        assert_eq!(o.len(), 3);

        let mut seg = Segmenter::new(SegmenterConfig {
            idle_after_ms: 3000,
            ..cfg()
        });
        let samples: Vec<_> = (0..6)
            .map(|i| sample(i * 1000, w("a.exe"), (i as u64).saturating_sub(1) * 1000))
            .collect();
        let o = opens(&run(&mut seg, &samples));
        // idle_ms reaches 3000 at ts 4000: last input at 1000.
        assert_eq!(o[1], (SegmentKind::Idle, None, 1000));
    }

    #[test]
    fn splits_at_day_start_and_flushes() {
        let mut seg = Segmenter::new(cfg());
        let day_ms = 86_400_000;
        let samples: Vec<_> = (0..17)
            .map(|i| sample(day_ms - 5000 + i * 1000, w("a.exe"), 0))
            .collect();
        let actions = run(&mut seg, &samples);
        let o = opens(&actions);
        assert_eq!(o.len(), 2);
        assert_eq!(o[1].2, day_ms);
        assert!(actions.contains(&Action::Close { end: day_ms }));
        assert!(actions.iter().any(|a| matches!(a, Action::Extend { .. })));
    }

    #[test]
    fn locked_and_paused_switch_immediately() {
        let mut seg = Segmenter::new(cfg());
        let mut locked = sample(1000, w("a.exe"), 0);
        locked.session = SessionState::Locked;
        let mut actions = run(&mut seg, &[sample(0, w("a.exe"), 0), locked]);
        seg.set_paused(true);
        actions.extend(seg.on_sample(&sample(2000, w("a.exe"), 0), 0));
        let kinds: Vec<_> = opens(&actions).into_iter().map(|o| o.0).collect();
        assert_eq!(
            kinds,
            [
                SegmentKind::Active,
                SegmentKind::Locked,
                SegmentKind::Paused
            ]
        );
    }

    #[tokio::test]
    async fn task_writes_segments_and_activity() {
        let storage = StorageHandle::open_in_memory().unwrap();
        let bus = EventBus::default();
        let mut events = bus.subscribe();
        let (activity_tx, activity_rx) = watch::channel(Activity::default());
        let task = SegmenterTask {
            segmenter: Segmenter::new(SegmenterConfig {
                min_segment_ms: 0,
                flush_ms: 1000,
                ..cfg()
            }),
            storage: storage.clone(),
            bus: bus.clone(),
            clock: Arc::new(MockClock::new(0)),
            platform: Arc::new(MockPlatform::default()),
            categorizer: Categorizer::default(),
            activity: activity_tx,
        };
        let (tx, rx) = mpsc::channel(16);
        let h = tokio::spawn(task.run(rx));
        for s in [
            sample(0, w("code.exe"), 0),
            sample(1000, w("code.exe"), 0),
            sample(2000, w("game.exe"), 0),
            sample(3000, w("game.exe"), 0),
        ] {
            tx.send(s).await.unwrap();
        }
        drop(tx);
        h.await.unwrap();
        assert_eq!(
            activity_rx.borrow().app.as_ref().unwrap().exe_name,
            "game.exe"
        );
        let day = LocalDay::from_ts(0, 0, TimeOfDay::MIDNIGHT);
        let usage = storage
            .call(move |db| db.usage_by_app(day, day))
            .await
            .unwrap();
        assert_eq!(usage.len(), 2);
        assert_eq!(usage.iter().map(|u| u.ms).sum::<i64>(), 3000);
        let code = usage.iter().find(|u| u.exe_name == "code.exe").unwrap();
        assert_eq!(code.category_kind, an_core::CategoryKind::Work);
        let mut opened = 0;
        while let Ok(e) = events.try_recv() {
            if matches!(e, Event::Tracker(TrackerEvent::SegmentOpened { .. })) {
                opened += 1;
            }
        }
        assert_eq!(opened, 2);
    }
}
