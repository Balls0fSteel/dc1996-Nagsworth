//! Opens and closes the toast and overlay windows to match the service's
//! nag board (`ns_runtime::nag_board`), and exits when the updater asks.
//!
//! The pages themselves read the nags from the API (`/v1/nags/current` and
//! the event stream); this side only owns the windows. It reads the board
//! and the bus in-process for now; once the GUI runs apart from the service
//! (docs/HEADLESS.md, phase 3) the same two inputs come from the event
//! stream.

use ns_core::{AppEvent, Event, EventBus, NagsView};
use tauri::AppHandle;
use tokio::sync::{broadcast, watch};

use crate::windows;

/// Runs on Tauri's runtime until the board's sender goes away.
pub fn spawn(app: AppHandle, mut nags: watch::Receiver<NagsView>) {
    tauri::async_runtime::spawn(async move {
        let mut shown = None;
        loop {
            let want = {
                let v = nags.borrow_and_update();
                (v.toast, v.overlay)
            };
            if shown != Some(want) {
                match windows::sync_nag_windows(&app, want.0, want.1) {
                    Ok(()) => shown = Some(want),
                    Err(e) => tracing::warn!("cannot update nag windows: {e}"),
                }
            }
            if nags.changed().await.is_err() {
                break;
            }
        }
    });
}

/// Exits the app when the update's setup has started and waits for it.
pub fn spawn_exit_watch(app: AppHandle, bus: &EventBus) {
    let mut rx = bus.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Event::App(AppEvent::ExitForUpdate)) => {
                    tracing::info!("exiting for the update");
                    app.exit(0);
                    break;
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
