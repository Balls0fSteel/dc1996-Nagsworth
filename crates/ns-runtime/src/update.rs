//! Nagsworth updating itself (`ns-update`, docs/DECISIONS.md D22): the check timer, what the
//! dashboard's banner, Settings card and the tray menu show, the download and the setup's
//! start. The state goes on the bus as `Event::Update` whenever it changes (the API serves it
//! and streams it); starting the setup publishes `AppEvent::ExitForUpdate` for the host to
//! exit. `update.check`, `update.beta` and `update.seen` live in the settings table.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ns_core::{AppEvent, AvailableView, DownloadView, Event, EventBus, UpdateStatus};
use ns_storage::StorageHandle;
use ns_update::{Checker, Kind, Setup, Version};

const CHECK_KEY: &str = "update.check";
const BETA_KEY: &str = "update.beta";
const SEEN_KEY: &str = "update.seen";
/// How often the timer looks whether an automatic check is due.
const TICK: Duration = Duration::from_secs(5);

struct Inner {
    checker: Checker,
    current: Version,
    beta: bool,
    can_install: bool,
    /// Ends a download on its way.
    cancel: Option<Arc<AtomicBool>>,
    download: Option<DownloadView>,
    setup: Option<(Version, Setup)>,
    error: Option<String>,
    last_check: Option<i64>,
}

#[derive(Clone)]
pub struct Updater {
    inner: Arc<Mutex<Inner>>,
    storage: StorageHandle,
    bus: EventBus,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Updater {
    /// Reads the stored settings; nothing runs until [`Updater::spawn`].
    pub fn new(storage: StorageHandle, bus: EventBus) -> Updater {
        let get = |key: &'static str| {
            storage
                .call_blocking(move |db| db.kv_get(key))
                .ok()
                .flatten()
        };
        let auto = get(CHECK_KEY).as_deref() != Some("false");
        let beta = get(BETA_KEY).as_deref() == Some("true");
        let seen = get(SEEN_KEY).and_then(|s| Version::parse(&s));
        let can_install = ns_update::CAN_SELF_UPDATE && ns_update::installed().is_some();
        Updater {
            inner: Arc::new(Mutex::new(Inner {
                checker: Checker::new(auto, Instant::now(), seen),
                current: Version::current(),
                beta,
                can_install,
                cancel: None,
                download: None,
                setup: None,
                error: None,
                last_check: None,
            })),
            storage,
            bus,
        }
    }

    /// Starts the timer thread for the automatic checks.
    pub fn spawn(&self) {
        if !ns_update::AVAILABLE {
            tracing::info!("update check: built without one");
            return;
        }
        let this = self.clone();
        std::thread::Builder::new()
            .name("ns-update".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(TICK);
                    let due = this.inner.lock().unwrap().checker.due(Instant::now());
                    if due {
                        this.run_check();
                    }
                }
            })
            .expect("spawn update thread");
    }

    pub fn status(&self) -> UpdateStatus {
        self.inner.lock().unwrap().status()
    }

    fn emit(&self) {
        let status = self.inner.lock().unwrap().status();
        self.bus.publish(Event::Update(status));
    }

    /// "Check now" (the Settings card, the tray): a check at once, unless one is on its way.
    pub fn check_now(&self) {
        let start = {
            let mut inner = self.inner.lock().unwrap();
            inner.error = None;
            inner.checker.start_manual()
        };
        self.emit();
        if start {
            self.run_check();
        }
    }

    /// Fetches the listing on a thread and applies the answer.
    fn run_check(&self) {
        let this = self.clone();
        std::thread::spawn(move || {
            let result = ns_update::fetch();
            let finished = {
                let mut inner = this.inner.lock().unwrap();
                let beta = inner.beta;
                let current = inner.current.clone();
                let f = inner.checker.finish(result, &current, beta);
                inner.last_check = Some(now_ms());
                inner.error = f.answer.as_ref().err().cloned();
                f
            };
            match (&finished.kind, &finished.answer) {
                (_, Ok(Some(v))) => tracing::info!("update check: {v} is available"),
                (_, Ok(None)) => tracing::info!("update check: this is the newest version"),
                (Kind::Automatic, Err(e)) => tracing::warn!("update check failed: {e}"),
                (Kind::Manual, Err(e)) => tracing::info!("update check failed: {e}"),
            }
            this.emit();
        });
    }

    /// Downloads the available version's setup, reporting progress as it goes.
    pub fn download(&self) -> Result<(), String> {
        let (version, cancel) = {
            let mut inner = self.inner.lock().unwrap();
            if !inner.can_install {
                return Err("this copy was not installed by the setup".into());
            }
            if inner.cancel.is_some() {
                return Ok(());
            }
            let v = inner
                .checker
                .available()
                .cloned()
                .ok_or("no newer version is known")?;
            let cancel = Arc::new(AtomicBool::new(false));
            inner.cancel = Some(cancel.clone());
            inner.download = Some(DownloadView {
                got: 0,
                total: None,
            });
            inner.setup = None;
            inner.error = None;
            (v, cancel)
        };
        self.emit();
        let this = self.clone();
        std::thread::spawn(move || {
            let dir = ns_update::download_dir();
            let mut shown = u64::MAX;
            let result = ns_update::download_setup(&version, &dir, &mut |got, total| {
                // about 200 updates at most
                let step = total.map_or(got >> 18, |t| got * 200 / t.max(1));
                if step != shown {
                    shown = step;
                    this.inner.lock().unwrap().download = Some(DownloadView { got, total });
                    this.emit();
                }
                !cancel.load(Ordering::Relaxed)
            });
            {
                let mut inner = this.inner.lock().unwrap();
                inner.cancel = None;
                inner.download = None;
                match result {
                    Ok(setup) => {
                        tracing::info!("update: {} downloaded and checked", setup.path.display());
                        inner.setup = Some((version, setup));
                    }
                    Err(e) if cancel.load(Ordering::Relaxed) => {
                        tracing::info!("update: download cancelled ({e})");
                    }
                    Err(e) => {
                        tracing::warn!("update: download failed: {e}");
                        inner.error = Some(e);
                    }
                }
            }
            this.emit();
        });
        Ok(())
    }

    pub fn cancel(&self) {
        if let Some(c) = &self.inner.lock().unwrap().cancel {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// Starts the downloaded setup and asks the host to exit: the setup waits for this
    /// process, installs over the same folder and starts the new version.
    pub fn install(&self) -> Result<(), String> {
        let (version, setup) = {
            let inner = self.inner.lock().unwrap();
            inner.setup.clone().ok_or("no setup is downloaded")?
        };
        ns_update::run_setup(&setup).map_err(|e| {
            tracing::warn!("update: cannot start the setup: {e}");
            format!("cannot start the setup: {e}")
        })?;
        tracing::info!("update: setup for {version} started, exiting");
        self.bus.publish(Event::App(AppEvent::ExitForUpdate));
        Ok(())
    }

    /// "Later": hides the banner for this version.
    pub fn dismiss(&self) {
        let seen = {
            let mut inner = self.inner.lock().unwrap();
            inner.checker.dismiss();
            inner.checker.dismissed().map(|v| v.to_string())
        };
        if let Some(seen) = seen {
            self.storage.send(move |db| db.kv_set(SEEN_KEY, &seen));
        }
        self.emit();
    }

    pub fn set_auto(&self, on: bool) {
        self.inner
            .lock()
            .unwrap()
            .checker
            .set_automatic(on, Instant::now());
        self.storage
            .send(move |db| db.kv_set(CHECK_KEY, if on { "true" } else { "false" }));
        self.emit();
    }

    /// Pre-releases on or off; a check follows, as the answer may change.
    pub fn set_beta(&self, on: bool) {
        self.inner.lock().unwrap().beta = on;
        self.storage
            .send(move |db| db.kv_set(BETA_KEY, if on { "true" } else { "false" }));
        self.check_now();
    }

    /// The available version's page on GitHub (the release page: notes and files).
    pub fn release_url(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .checker
            .available()
            .map(Version::release_url)
    }
}

impl Inner {
    fn status(&self) -> UpdateStatus {
        UpdateStatus {
            current: self.current.to_string(),
            enabled: ns_update::AVAILABLE,
            auto_check: self.checker.automatic(),
            beta: self.beta,
            can_install: self.can_install,
            checking: self.checker.running().is_some(),
            available: self.checker.available().map(|v| AvailableView {
                version: v.to_string(),
                url: v.release_url(),
            }),
            banner: self.checker.banner().map(|v| v.to_string()),
            download: self.download.clone(),
            ready: self.setup.as_ref().map(|(v, _)| v.to_string()),
            error: self.error.clone(),
            last_check: self.last_check,
        }
    }
}
