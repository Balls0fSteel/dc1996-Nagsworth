use std::sync::Arc;
use std::time::Duration;

use ns_core::config::Exercises as ExercisesConfig;
use ns_core::{Activity, Clock, ControlCommand, Event, EventBus, NagEvent};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::MissedTickBehavior;

use crate::engine::{Engine, EngineSnapshot};
use crate::rule::RuleSet;

/// Commands only the composition root sends (control commands from users
/// arrive over the event bus).
#[derive(Debug)]
pub enum EngineCommand {
    ReplaceRules(RuleSet),
    /// `tracking.hands_on_games` changed in the config.
    SetHandsOnGames(bool),
    /// `[exercises]` changed in the config.
    SetExercises(ExercisesConfig),
    Shutdown,
}

/// What the engine persists: rule counters and the exercise rotation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EngineState {
    pub counters: Vec<(String, String)>,
    /// JSON, `None` without exercises.
    pub rotation: Option<String>,
}

pub type PersistFn = Box<dyn Fn(EngineState) + Send + Sync>;

/// Runs an [`Engine`]: 1 s ticks, control commands from the bus, nag events
/// to the bus, snapshots to a watch channel, counters persisted every minute.
pub struct EngineTask {
    pub engine: Engine,
    pub clock: Arc<dyn Clock>,
    pub bus: EventBus,
    pub activity: watch::Receiver<Activity>,
    pub snapshot: watch::Sender<EngineSnapshot>,
    pub persist: Option<PersistFn>,
    pub tick: Duration,
}

impl EngineTask {
    fn publish(&self, events: Vec<NagEvent>) {
        for e in events {
            self.bus.publish(Event::Nag(e));
        }
    }

    fn save(&self) {
        if let Some(persist) = &self.persist {
            persist(EngineState {
                counters: self.engine.export_counters(),
                rotation: self.engine.export_rotation(),
            });
        }
    }

    pub async fn run(mut self, mut commands: mpsc::Receiver<EngineCommand>) {
        let mut control = self.bus.subscribe();
        let mut ticker = tokio::time::interval(self.tick);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut save_every = tokio::time::interval(Duration::from_secs(60));
        save_every.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let now = self.clock.now();
                    let activity = self.activity.borrow().clone();
                    let events = self.engine.tick(now, self.clock.offset_s(now), &activity);
                    self.publish(events);
                    self.snapshot.send_replace(self.engine.snapshot(now));
                }
                _ = save_every.tick() => self.save(),
                cmd = commands.recv() => match cmd {
                    Some(EngineCommand::ReplaceRules(rules)) => {
                        let now = self.clock.now();
                        let events = self.engine.replace_rules(rules, now);
                        self.publish(events);
                        self.snapshot.send_replace(self.engine.snapshot(now));
                    }
                    Some(EngineCommand::SetHandsOnGames(on)) => {
                        self.engine.set_hands_on_games(on);
                        self.snapshot.send_replace(self.engine.snapshot(self.clock.now()));
                    }
                    Some(EngineCommand::SetExercises(cfg)) => self.engine.set_exercises_config(cfg),
                    Some(EngineCommand::Shutdown) | None => break,
                },
                event = control.recv() => match event {
                    Ok(Event::Control(cmd)) => {
                        let now = self.clock.now();
                        let events = self.engine.command(&cmd, now);
                        if !matches!(cmd, ControlCommand::Reload) {
                            self.publish(events);
                            self.snapshot.send_replace(self.engine.snapshot(now));
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                },
            }
        }
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use ns_core::{AppRef, CategoryKind, MockClock, SegmentKind, TimeOfDay};

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn ticks_publish_nags_and_snapshots() {
        let rules = RuleSet::from_toml(
            r#"
            [[rules]]
            id = "r"
            trigger = { kind = "interval", every = "3s", break_for = "1s" }
        "#,
        )
        .unwrap();
        let clock = Arc::new(MockClock::new(0));
        let bus = EventBus::default();
        let mut events = bus.subscribe();
        let (_atx, arx) = watch::channel(Activity {
            kind: Some(SegmentKind::Active),
            app: Some(AppRef {
                id: 1,
                exe_name: "a.exe".into(),
                category: CategoryKind::Work,
                is_game: false,
            }),
            ..Activity::default()
        });
        let (stx, srx) = watch::channel(EngineSnapshot::default());
        let saved = Arc::new(Mutex::new(Vec::new()));
        let saved2 = saved.clone();
        let task = EngineTask {
            engine: Engine::new(rules, TimeOfDay::MIDNIGHT, false, true),
            clock: clock.clone(),
            bus: bus.clone(),
            activity: arx,
            snapshot: stx,
            persist: Some(Box::new(move |c| *saved2.lock().unwrap() = c.counters)),
            tick: Duration::from_secs(1),
        };
        let (ctx, crx) = mpsc::channel(4);
        let h = tokio::spawn(task.run(crx));
        let raised = loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            clock.advance(1000);
            if let Ok(Event::Nag(e @ NagEvent::Raise { .. })) = events.try_recv() {
                break e;
            }
            assert!(clock.now() < 10_000, "no nag raised");
        };
        assert_eq!(raised.rule_id(), "r");
        bus.publish(Event::Control(ControlCommand::SkipBreak {
            rule_id: "r".into(),
        }));
        tokio::time::sleep(Duration::from_millis(10)).await;
        // A tick due at the same instant may land after the skip.
        assert_eq!(srx.borrow().rules[0].state, crate::NagState::Idle);
        assert!(srx.borrow().rules[0].accum_ms <= 1000);
        ctx.send(EngineCommand::Shutdown).await.unwrap();
        h.await.unwrap();
        assert_eq!(saved.lock().unwrap().len(), 1);
    }
}
