// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod nag_bridge;
mod runtime;
mod tray;
mod windows;

fn main() {
    app::run();
}
