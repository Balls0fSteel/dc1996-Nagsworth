//! Tauri commands for the dashboard and nag windows: only what needs the
//! window system. Everything else, the nags, autostart and updates included,
//! goes through the local HTTP API (docs/HEADLESS.md).

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::app::AppCtx;
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
pub fn open_dashboard(app: AppHandle) {
    windows::show_dashboard(&app);
}

/// The toast page reports its content height so the window fits the cards.
#[tauri::command]
pub fn resize_toast(app: AppHandle, height: f64) -> Result<(), String> {
    windows::resize_toast(&app, height).map_err(|e| e.to_string())
}

/// Opens a release page of this project in the browser (the update banner's
/// "Release notes"). Other URLs are refused.
#[tauri::command]
pub fn open_release_page(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with(&format!("{}/", ns_update::RELEASES_URL)) {
        return Err("not a release page of this project".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
