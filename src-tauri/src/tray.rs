//! Tray icon and menu. A background thread keeps the tooltip and the
//! pause/reading-mode items in sync with the runtime.

use std::time::Duration;

use an_core::{ControlCommand, SegmentKind};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::app::AppCtx;
use crate::nag_bridge::NagBridge;
use crate::windows;

const PAUSE_MINUTES: u64 = 30;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open dashboard", true, None::<&str>)?;
    let brk = MenuItem::with_id(app, "break", "Start break now", true, None::<&str>)?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        "Pause tracking for 30 min",
        true,
        None::<&str>,
    )?;
    let reading =
        CheckMenuItem::with_id(app, "reading", "Reading mode", true, false, None::<&str>)?;
    let pair = MenuItem::with_id(app, "pair", "Pair browser extension…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&open, &sep1, &brk, &pause, &reading, &pair, &sep2, &quit],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("tray icon".into()))?;
    let tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("activity_nagger")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let ctx = app.state::<AppCtx>();
            tracing::info!("tray menu: {}", event.id().as_ref());
            match event.id().as_ref() {
                "open" => windows::show_dashboard(app),
                "break" => ctx.control(ControlCommand::StartBreakNow { rule_id: None }),
                "pause" => {
                    let paused = ctx.activity.borrow().kind == Some(SegmentKind::Paused);
                    ctx.control(if paused {
                        ControlCommand::ResumeTracking
                    } else {
                        ControlCommand::PauseTracking {
                            for_ms: Some(PAUSE_MINUTES * 60_000),
                        }
                    });
                }
                "reading" => {
                    let on = !ctx.engine.borrow().reading_mode;
                    ctx.control(ControlCommand::SetReadingMode { on });
                }
                "pair" => {
                    if let Err(e) = windows::show_pairing(app) {
                        tracing::warn!("cannot open pairing window: {e}");
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                windows::show_dashboard(tray.app_handle());
            }
        })
        .build(app)?;
    tracing::info!("tray icon created");

    // Keep the tooltip and menu state current.
    let app = app.clone();
    std::thread::Builder::new()
        .name("an-tray".into())
        .spawn(move || {
            let mut last = (String::new(), None, None);
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let ctx = app.state::<AppCtx>();
                let activity = ctx.activity.borrow().clone();
                let reading_mode = ctx.engine.borrow().reading_mode;
                let paused = activity.kind == Some(SegmentKind::Paused);
                // A nag that cannot be shown (exclusive fullscreen) goes here.
                let bridge = app.state::<NagBridge>();
                let due = if bridge.is_deferred() {
                    bridge.current()
                } else {
                    Vec::new()
                };
                let tip = match (&activity.kind, &activity.app) {
                    _ if !due.is_empty() => format!(
                        "activity_nagger: {} (waiting for fullscreen to end)",
                        due.iter()
                            .map(|n| n.rule_name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    (Some(SegmentKind::Active), Some(a)) => {
                        format!("activity_nagger: {}", a.exe_name)
                    }
                    (Some(k), _) => format!("activity_nagger: {k}"),
                    _ => "activity_nagger".to_string(),
                };
                if last.0 != tip {
                    let _ = tray.set_tooltip(Some(&tip));
                }
                if last.1 != Some(paused) {
                    let _ = pause.set_text(if paused {
                        "Resume tracking"
                    } else {
                        "Pause tracking for 30 min"
                    });
                }
                if last.2 != Some(reading_mode) {
                    let _ = reading.set_checked(reading_mode);
                }
                last = (tip, Some(paused), Some(reading_mode));
            }
        })
        .expect("spawn tray thread");
    Ok(())
}
