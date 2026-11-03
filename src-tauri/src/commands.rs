//! Tauri commands for the dashboard and nag windows. Everything else goes
//! through the local HTTP API.

use an_core::ControlCommand;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app::AppCtx;
use crate::nag_bridge::{NagBridge, NagView};
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
