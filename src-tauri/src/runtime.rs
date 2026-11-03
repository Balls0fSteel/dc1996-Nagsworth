//! Composition root. Starts every tracking, rules and API task on a dedicated
//! tokio runtime. Uses no Tauri types, so a headless service build can reuse
//! it unchanged.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use an_api::{ApiState, Auth, ConfigStore};
use an_core::config::{self, Config};
use an_core::{
    Activity, Clock, ControlCommand, Event, EventBus, LocalDay, NagEvent, NagLevel, SystemClock,
};
use an_platform::Platform;
use an_rules::{Engine, EngineCommand, EngineSnapshot, EngineTask, RuleSet};
use an_storage::StorageHandle;
use an_tracker::segmenter::SegmenterTask;
use an_tracker::web::WebTask;
use an_tracker::{Categorizer, Sampler, Segmenter, SegmenterConfig, WebSegmenter};
use anyhow::Context;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

const READING_MODE_KEY: &str = "engine.reading_mode";

pub struct Runtime {
    rt: tokio::runtime::Runtime,
    pub bus: EventBus,
    pub storage: StorageHandle,
    pub activity: watch::Receiver<Activity>,
    pub engine: watch::Receiver<EngineSnapshot>,
    pub api_addr: Option<SocketAddr>,
    pub token: String,
    pub data_dir: PathBuf,
    engine_cmd: mpsc::Sender<EngineCommand>,
    sampler: JoinHandle<()>,
    segmenter: JoinHandle<()>,
    engine_task: JoinHandle<()>,
}

/// Validates, saves and applies config file text.
struct FileConfigStore {
    path: PathBuf,
    config: Arc<RwLock<Config>>,
    engine_cmd: mpsc::Sender<EngineCommand>,
}

impl FileConfigStore {
    fn apply(&self, text: &str) -> Result<(), String> {
        let cfg = Config::from_toml(text).map_err(|e| e.to_string())?;
        let rules = RuleSet::from_toml(text).map_err(|e| e.to_string())?;
        *self.config.write().unwrap() = cfg;
        self.engine_cmd
            .try_send(EngineCommand::ReplaceRules(rules))
            .map_err(|e| format!("engine not running: {e}"))
    }

    fn reload(&self) -> Result<(), String> {
        let loaded = config::load(&self.path).map_err(|e| e.to_string())?;
        self.apply(&loaded.text)
    }
}

impl ConfigStore for FileConfigStore {
    fn read(&self) -> String {
        std::fs::read_to_string(&self.path)
            .unwrap_or_else(|_| config::DEFAULT_CONFIG_TOML.to_string())
    }

    fn write(&self, text: &str) -> Result<(), String> {
        // Validate before touching the file.
        Config::from_toml(text).map_err(|e| e.to_string())?;
        RuleSet::from_toml(text).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, text).map_err(|e| e.to_string())?;
        self.apply(text)
    }
}

impl Runtime {
    pub fn start(data_dir: PathBuf, platform: Arc<dyn Platform>) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("create {}", data_dir.display()))?;
        let config_path = data_dir.join("config.toml");
        if config::ensure_default_file(&config_path)? {
            tracing::info!("wrote default config to {}", config_path.display());
        }
        let loaded = config::load(&config_path)?;
        let rules = match RuleSet::from_toml(&loaded.text) {
            Ok(r) => r,
            Err(e) => {
                // A broken rules section must not stop tracking.
                tracing::error!("{e}; starting without rules");
                RuleSet::default()
            }
        };
        let cfg = loaded.config;
        let storage = StorageHandle::open(&data_dir.join("activity.db"))?;
        let token = match storage.call_blocking(|db| db.kv_get(an_api::auth::TOKEN_KEY))? {
            Some(t) => t,
            None => {
                let t = an_api::generate_token();
                let t2 = t.clone();
                storage.call_blocking(move |db| db.kv_set(an_api::auth::TOKEN_KEY, &t2))?;
                t
            }
        };
        let reading_mode = storage
            .call_blocking(|db| db.kv_get(READING_MODE_KEY))?
            .map(|v| v == "true")
            .unwrap_or(cfg.general.reading_mode);
        let saved_counters = storage.call_blocking(|db| db.rule_states())?;

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("an-rt")
            .enable_all()
            .build()?;
        let _guard = rt.enter();

        let bus = EventBus::default();
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let config = Arc::new(RwLock::new(cfg.clone()));

        // Session events from the platform thread go straight onto the bus.
        {
            let bus = bus.clone();
            if let Err(e) = platform.session_events(Box::new(move |e| {
                bus.publish(Event::Session(e));
            })) {
                tracing::warn!("session events unavailable: {e}");
            }
        }

        // Built-in game list plus the user's known_games.toml.
        let known = data_dir.join("known_games.toml");
        let categorizer = match an_tracker::gaming::KnownExeList::builtin_and_file(&known) {
            Ok(list) => Categorizer::new(list),
            Err(e) => {
                tracing::warn!("ignoring known games file: {e}");
                Categorizer::default()
            }
        };

        // Tracking: sampler -> segmenter -> storage + bus + activity watch.
        let (activity_tx, activity_rx) = watch::channel(Activity::default());
        let (sample_tx, sample_rx) = mpsc::channel(64);
        let sampler = Sampler::spawn(
            platform.clone(),
            clock.clone(),
            Duration::from_millis(cfg.tracking.poll_ms.max(100)),
            sample_tx,
        );
        let segmenter = rt.spawn(
            SegmenterTask {
                segmenter: Segmenter::new(SegmenterConfig::from_config(&cfg)),
                storage: storage.clone(),
                bus: bus.clone(),
                clock: clock.clone(),
                platform: platform.clone(),
                categorizer,
                activity: activity_tx,
            }
            .run(sample_rx),
        );

        // Web visits from the extension.
        let (ext_tx, ext_rx) = mpsc::channel(256);
        rt.spawn(
            WebTask {
                segmenter: WebSegmenter::new(cfg.web.store_url, cfg.general.day_start),
                storage: storage.clone(),
                bus: bus.clone(),
                clock: clock.clone(),
                activity: activity_rx.clone(),
            }
            .run(ext_rx),
        );

        // Rules engine.
        let mut engine = Engine::new(rules, cfg.general.day_start, reading_mode);
        engine.import_counters(&saved_counters);
        let (snap_tx, snap_rx) = watch::channel(engine.snapshot(clock.now()));
        let (engine_cmd, engine_cmd_rx) = mpsc::channel(8);
        let persist_storage = storage.clone();
        let persist_clock = clock.clone();
        let engine_task = rt.spawn(
            EngineTask {
                engine,
                clock: clock.clone(),
                bus: bus.clone(),
                activity: activity_rx.clone(),
                snapshot: snap_tx,
                persist: Some(Box::new(move |counters| {
                    let now = persist_clock.now();
                    persist_storage.send(move |db| db.save_rule_states(&counters, now));
                })),
                tick: Duration::from_secs(1),
            }
            .run(engine_cmd_rx),
        );

        let config_store = Arc::new(FileConfigStore {
            path: config_path,
            config: config.clone(),
            engine_cmd: engine_cmd.clone(),
        });

        rt.spawn(side_effects(
            bus.clone(),
            storage.clone(),
            platform.clone(),
            config_store.clone(),
        ));
        rt.spawn(retention(storage.clone(), clock.clone(), config.clone()));

        let api_state = ApiState {
            storage: storage.clone(),
            bus: bus.clone(),
            clock: clock.clone(),
            config: config.clone(),
            config_store,
            auth: Auth::new(token.clone()),
            engine: snap_rx.clone(),
            activity: activity_rx.clone(),
            ext_tx,
            ext_clients: Arc::default(),
            backup_dir: data_dir.join("backups"),
        };
        let api_addr = match rt.block_on(an_api::serve(api_state, &cfg.api.bind, cfg.api.port)) {
            Ok(addr) => Some(addr),
            Err(e) => {
                tracing::error!("local API unavailable: {e}");
                None
            }
        };
        drop(_guard);

        Ok(Self {
            rt,
            bus,
            storage,
            activity: activity_rx,
            engine: snap_rx,
            api_addr,
            token,
            data_dir,
            engine_cmd,
            sampler,
            segmenter,
            engine_task,
        })
    }

    pub fn api_base_url(&self) -> Option<String> {
        self.api_addr.map(|a| format!("http://{a}/v1"))
    }

    /// Stops sampling, closes the open segment and saves rule counters.
    pub fn shutdown(self) {
        self.sampler.abort();
        let _ = self.engine_cmd.try_send(EngineCommand::Shutdown);
        let Runtime {
            rt,
            segmenter,
            engine_task,
            storage,
            ..
        } = self;
        rt.block_on(async move {
            let wait = async {
                let _ = segmenter.await;
                let _ = engine_task.await;
                let _ = storage.call(|_| Ok(())).await;
            };
            if tokio::time::timeout(Duration::from_secs(3), wait)
                .await
                .is_err()
            {
                tracing::warn!("shutdown timed out");
            }
        });
        rt.shutdown_timeout(Duration::from_secs(1));
    }
}

/// Logs nags, locks the workstation for `lock` nags, and handles the control
/// commands that touch config or settings.
async fn side_effects(
    bus: EventBus,
    storage: StorageHandle,
    platform: Arc<dyn Platform>,
    config_store: Arc<FileConfigStore>,
) {
    let mut rx = bus.subscribe();
    loop {
        let event = match rx.recv().await {
            Ok(e) => e,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("side effects lagged by {n} events");
                continue;
            }
            Err(_) => break,
        };
        match event {
            Event::Nag(nag) => {
                let (kind, level, reason) = match &nag {
                    NagEvent::Raise { level, .. } => ("raise", Some(*level), None),
                    NagEvent::Escalate { level, .. } => ("escalate", Some(*level), None),
                    NagEvent::BreakStarted { .. } => ("break_started", None, None),
                    NagEvent::Dismiss { reason, .. } => ("dismiss", None, Some(*reason)),
                };
                let rule_id = nag.rule_id().to_string();
                let level_s = level.map(|l| json_str(&l));
                let reason_s = reason.map(|r| json_str(&r));
                let at = match &nag {
                    NagEvent::Raise { at, .. }
                    | NagEvent::Escalate { at, .. }
                    | NagEvent::Dismiss { at, .. } => *at,
                    NagEvent::BreakStarted { .. } => SystemClock.now(),
                };
                storage.send(move |db| {
                    db.log_nag(&rule_id, kind, level_s.as_deref(), reason_s.as_deref(), at)
                });
                if level == Some(NagLevel::Lock) {
                    let p = platform.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = p.lock_workstation() {
                            tracing::warn!("cannot lock workstation: {e}");
                        }
                    });
                }
            }
            Event::Control(ControlCommand::Reload) => {
                if let Err(e) = config_store.reload() {
                    tracing::error!("config reload failed: {e}");
                }
            }
            Event::Control(ControlCommand::SetReadingMode { on }) => {
                storage
                    .send(move |db| db.kv_set(READING_MODE_KEY, if on { "true" } else { "false" }));
            }
            _ => {}
        }
    }
}

fn json_str<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Prunes old segments every six hours.
async fn retention(storage: StorageHandle, clock: Arc<dyn Clock>, config: Arc<RwLock<Config>>) {
    let mut every = tokio::time::interval(Duration::from_secs(6 * 3600));
    loop {
        every.tick().await;
        let (days, day_start) = {
            let c = config.read().unwrap();
            (c.tracking.retention_days, c.general.day_start)
        };
        if days == 0 {
            continue;
        }
        let now = clock.now();
        let mut cutoff = LocalDay::from_ts(now, clock.offset_s(now), day_start);
        for _ in 0..days {
            cutoff = cutoff.prev();
        }
        let before_ts = now - days as i64 * 86_400_000;
        match storage.call(move |db| db.prune(cutoff, before_ts)).await {
            Ok(0) => {}
            Ok(n) => tracing::info!("pruned {n} segments before {cutoff}"),
            Err(e) => tracing::warn!("prune failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use an_platform::MockPlatform;
    use an_platform::mock::{sample, window};

    use super::*;

    #[test]
    fn starts_serves_and_shuts_down() {
        let dir = std::env::temp_dir().join(format!("an-runtime-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let platform = Arc::new(MockPlatform::scripted(vec![sample(
            SystemClock.now(),
            Some(window("code.exe", "main.rs")),
            0,
        )]));
        let rt = Runtime::start(dir.clone(), platform).unwrap();
        assert!(dir.join("config.toml").exists());
        assert_eq!(rt.engine.borrow().rules.len(), 4);
        assert!(rt.api_addr.is_some());
        let token = rt.token.clone();
        rt.shutdown();

        // The token survives a restart.
        let rt = Runtime::start(dir.clone(), Arc::new(MockPlatform::default())).unwrap();
        assert_eq!(rt.token, token);
        rt.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
