//! Tauri commands for the dashboard and nag windows. Everything else goes
//! through the local HTTP API.

use ns_core::ControlCommand;
use serde::Serialize;
use tauri::{AppHandle, State};

use tauri_plugin_opener::OpenerExt;

use crate::app::AppCtx;
use crate::nag_bridge::{NagBridge, NagView};
use crate::update::{UpdateStatus, Updater};
use crate::windows;

#[derive(Serialize)]
pub struct ApiInfo {
    base_url: String,
    token: String,
}

#[tauri::command]
pub fn get_api_info(ctx: State<'_, AppCtx>) -> Result<ApiInfo, String> {
    Ok(ApiInfo {
        base_url: ctx
            .base_url
            .clone()
            .ok_or("the local API is not running (port in use?)")?,
        token: ctx.token.clone(),
    })
}

#[tauri::command]
pub fn current_nags(bridge: State<'_, NagBridge>) -> Vec<NagView> {
    bridge.current()
}

#[tauri::command]
pub fn nag_action(ctx: State<'_, AppCtx>, rule_id: String, action: String) -> Result<(), String> {
    let cmd = match action.as_str() {
        "snooze" => ControlCommand::Snooze { rule_id },
        "skip" => ControlCommand::SkipBreak { rule_id },
        "break" => ControlCommand::StartBreakNow {
            rule_id: Some(rule_id),
        },
        other => return Err(format!("unknown action `{other}`")),
    };
    ctx.control(cmd);
    Ok(())
}

#[tauri::command]
pub fn open_dashboard(app: AppHandle) {
    windows::show_dashboard(&app);
}

/// The toast page reports its content height so the window fits the cards.
#[tauri::command]
pub fn resize_toast(app: AppHandle, height: f64) -> Result<(), String> {
    windows::resize_toast(&app, height).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_autostart(ctx: State<'_, AppCtx>) -> Result<bool, String> {
    ctx.platform.autostart().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(ctx: State<'_, AppCtx>, enabled: bool) -> Result<(), String> {
    ctx.platform
        .set_autostart(enabled)
        .map_err(|e| e.to_string())
}

// ---- Updates (update.rs) ----

#[tauri::command]
pub fn update_status(updater: State<'_, Updater>) -> UpdateStatus {
    updater.status()
}

#[tauri::command]
pub fn update_check(updater: State<'_, Updater>) {
    updater.check_now();
}

#[tauri::command]
pub fn update_download(updater: State<'_, Updater>) -> Result<(), String> {
    updater.download()
}

#[tauri::command]
pub fn update_cancel(updater: State<'_, Updater>) {
    updater.cancel();
}

#[tauri::command]
pub fn update_install(updater: State<'_, Updater>) -> Result<(), String> {
    updater.install()
}

#[tauri::command]
pub fn update_dismiss(updater: State<'_, Updater>) {
    updater.dismiss();
}

#[tauri::command]
pub fn update_set_auto(updater: State<'_, Updater>, on: bool) {
    updater.set_auto(on);
}

#[tauri::command]
pub fn update_set_beta(updater: State<'_, Updater>, on: bool) {
    updater.set_beta(on);
}

/// The available release's page in the browser (notes, and the files for a copy that cannot
/// install the update itself).
#[tauri::command]
pub fn update_open_release_page(app: AppHandle, updater: State<'_, Updater>) -> Result<(), String> {
    let url = updater.release_url().ok_or("no newer version is known")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
