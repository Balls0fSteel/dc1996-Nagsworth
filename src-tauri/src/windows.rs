//! Dashboard, toast, overlay and pairing windows.

use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const MAIN: &str = "main";
pub const TOAST: &str = "toast";
pub const PAIR: &str = "pair";
const OVERLAY_PREFIX: &str = "overlay-";

const TOAST_W: f64 = 380.0;
const TOAST_H: f64 = 210.0;
const TOAST_MARGIN: f64 = 16.0;

pub fn show_dashboard<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn show_pairing<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(PAIR) {
        w.show()?;
        return w.set_focus();
    }
    WebviewWindowBuilder::new(app, PAIR, WebviewUrl::App("pair.html".into()))
        .title("Pair browser extension")
        .inner_size(420.0, 300.0)
        .resizable(false)
        .build()?;
    Ok(())
}

/// Opens or closes the toast and per-monitor overlays.
pub fn sync_nag_windows<R: Runtime>(
    app: &AppHandle<R>,
    toast: bool,
    overlay: bool,
) -> tauri::Result<()> {
    match (toast, app.get_webview_window(TOAST)) {
        (true, None) => open_toast(app)?,
        (false, Some(w)) => w.close()?,
        _ => {}
    }
    let overlays: Vec<WebviewWindow<R>> = app
        .webview_windows()
        .into_iter()
        .filter(|(label, _)| label.starts_with(OVERLAY_PREFIX))
        .map(|(_, w)| w)
        .collect();
    if overlay && overlays.is_empty() {
        open_overlays(app)?;
    } else if !overlay {
        for w in overlays {
            w.close()?;
        }
    }
    Ok(())
}

fn open_toast<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, TOAST, WebviewUrl::App("toast.html".into()))
        .title("activity_nagger")
        .inner_size(TOAST_W, TOAST_H)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()?;
    // Bottom-right of the primary monitor's work area, in physical pixels.
    if let Some(m) = app.primary_monitor()? {
        let scale = m.scale_factor();
        let area = m.work_area();
        let w_px = (TOAST_W * scale) as i32;
        let h_px = (TOAST_H * scale) as i32;
        let margin = (TOAST_MARGIN * scale) as i32;
        let x = area.position.x + area.size.width as i32 - w_px - margin;
        let y = area.position.y + area.size.height as i32 - h_px - margin;
        w.set_position(PhysicalPosition::new(x, y))?;
    }
    w.show()
}

fn open_overlays<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let monitors = app.available_monitors()?;
    for (i, m) in monitors.iter().enumerate() {
        let label = format!("{OVERLAY_PREFIX}{i}");
        let w = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("overlay.html".into()))
            .title("Break")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build()?;
        // Physical coordinates avoid mixed-DPI rounding between monitors.
        w.set_position(PhysicalPosition::new(m.position().x, m.position().y))?;
        w.set_size(PhysicalSize::new(m.size().width, m.size().height))?;
        w.show()?;
        if i == 0 {
            w.set_focus()?;
        }
    }
    Ok(())
}
