//! Tauri setup: single instance, tray, hidden main window that closes to the
//! tray, nag windows, and runtime shutdown on quit.

use std::sync::{Arc, Mutex, RwLock};

use ns_core::config::Config;
use ns_core::{Activity, ControlCommand, Event, EventBus};
use ns_platform::Platform;
use ns_rules::EngineSnapshot;
use tauri::{Manager, RunEvent, WindowEvent};
use tokio::sync::watch;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

use crate::nag_bridge::NagBridge;
use crate::runtime::Runtime;
use crate::update::Updater;
use crate::{commands, tray, windows};

/// What commands and the tray need from the runtime.
pub struct AppCtx {
    pub bus: EventBus,
    pub token: String,
    pub base_url: Option<String>,
    pub activity: watch::Receiver<Activity>,
    pub engine: watch::Receiver<EngineSnapshot>,
    pub platform: Arc<dyn Platform>,
    pub config: Arc<RwLock<Config>>,
}

impl AppCtx {
    pub fn control(&self, cmd: ControlCommand) {
        self.bus.publish(Event::Control(cmd));
    }
}

struct RuntimeSlot(Mutex<Option<Runtime>>);

fn init_logging(data_dir: &std::path::Path) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = std::fs::create_dir_all(data_dir);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join("nagsworth.log"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false);
    match file {
        Ok(f) => builder
            .with_writer(std::io::stderr.and(Mutex::new(f)))
            .init(),
        Err(_) => builder.with_writer(std::io::stderr).init(),
    }
}

pub fn run() {
    let data_dir = ns_core::config::data_dir();
    init_logging(&data_dir);
    let args: Vec<String> = std::env::args().collect();
    let start_hidden = args.iter().any(|a| a == "--autostart" || a == "--hidden");
    // For tests on a hidden desktop: no icon on the user's taskbar.
    let no_tray = args.iter().any(|a| a == "--no-tray");

    let platform = ns_platform::create_platform();
    let runtime = match Runtime::start(data_dir, platform.clone()) {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("cannot start: {e:#}");
            eprintln!("Nagsworth: cannot start: {e:#}");
            std::process::exit(1);
        }
    };
    tracing::info!(
        "started; data in {}, api {}",
        runtime.data_dir.display(),
        runtime
            .api_base_url()
            .unwrap_or_else(|| "unavailable".into())
    );

    let ctx = AppCtx {
        bus: runtime.bus.clone(),
        token: runtime.token.clone(),
        base_url: runtime.api_base_url(),
        activity: runtime.activity.clone(),
        engine: runtime.engine.clone(),
        platform: platform.clone(),
        config: runtime.config.clone(),
    };
    let bus = runtime.bus.clone();
    let engine = runtime.engine.clone();
    let bridge_platform = platform;
    let bridge = NagBridge::default();
    let updater = Updater::new(runtime.storage.clone());
    let updater_for_setup = updater.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_dashboard(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .manage(ctx)
        .manage(bridge.clone())
        .manage(updater)
        .manage(RuntimeSlot(Mutex::new(Some(runtime))))
        .invoke_handler(tauri::generate_handler![
            commands::get_api_info,
            commands::current_nags,
            commands::nag_action,
            commands::nag_prefs,
            commands::open_dashboard,
            commands::resize_toast,
            commands::get_autostart,
            commands::set_autostart,
            commands::update_status,
            commands::update_check,
            commands::update_download,
            commands::update_cancel,
            commands::update_install,
            commands::update_dismiss,
            commands::update_set_auto,
            commands::update_set_beta,
            commands::update_open_release_page,
        ])
        .setup(move |app| {
            // Without a tray (no Explorer, another desktop) the dashboard is
            // the only way in, so show it.
            let tray_ok = !no_tray
                && tray::create(app.handle())
                    .inspect_err(|e| tracing::warn!("no tray icon: {e}"))
                    .is_ok();
            bridge.spawn(app.handle().clone(), &bus, engine, bridge_platform);
            updater_for_setup.spawn(app.handle().clone());
            if !start_hidden || !tray_ok {
                windows::show_dashboard(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the dashboard hides it; the app lives in the tray.
            if window.label() == windows::MAIN
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("build tauri app");

    app.run(|app, event| match event {
        // Closing the last window keeps running; only "Quit" exits.
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(rt) = app.state::<RuntimeSlot>().0.lock().unwrap().take() {
                tracing::info!("shutting down");
                rt.shutdown();
            }
        }
        _ => {}
    });
}
