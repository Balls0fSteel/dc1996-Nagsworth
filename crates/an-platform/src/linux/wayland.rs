//! Wayland: idle from `ext-idle-notify-v1`, the focused window from
//! `wlr-foreign-toplevel-management` (wlroots compositors such as Sway and
//! Hyprland). Compositors without these protocols (GNOME) fall back to
//! XWayland for X clients and report idle as unsupported.
//!
//! A thread owns the connection and keeps the shared state current.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wayland_client::backend::ObjectId;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1, zwlr_foreign_toplevel_manager_v1,
};

/// Idle notifications fire after this much inactivity; it is added back to
/// the idle time.
const IDLE_TIMEOUT: Duration = Duration::from_millis(1000);

#[derive(Clone, Debug, Default)]
pub struct Toplevel {
    pub app_id: String,
    pub title: String,
    pub activated: bool,
    pub fullscreen: bool,
}

#[derive(Debug, Default)]
pub struct Shared {
    pub has_idle: bool,
    pub has_toplevels: bool,
    idle_since: Option<Instant>,
    focused: Option<Toplevel>,
}

impl Shared {
    pub fn idle_ms(&self) -> u64 {
        self.idle_since
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn focused(&self) -> Option<Toplevel> {
        self.focused.clone()
    }
}

struct State {
    shared: Arc<Mutex<Shared>>,
    pending: HashMap<ObjectId, Toplevel>,
    current: HashMap<ObjectId, Toplevel>,
}

impl State {
    fn publish_focus(&self) {
        let focused = self.current.values().find(|t| t.activated).cloned();
        self.shared.lock().unwrap().focused = focused;
    }
}

/// Connects to `$WAYLAND_DISPLAY` and starts the event thread.
pub fn spawn() -> Option<Arc<Mutex<Shared>>> {
    let conn = Connection::connect_to_env().ok()?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
    let qh = queue.handle();
    let shared = Arc::new(Mutex::new(Shared::default()));

    let seat: Option<wl_seat::WlSeat> = globals.bind(&qh, 1..=7, ()).ok();
    let notifier: Option<ext_idle_notifier_v1::ExtIdleNotifierV1> =
        globals.bind(&qh, 1..=1, ()).ok();
    if let (Some(seat), Some(notifier)) = (&seat, &notifier) {
        notifier.get_idle_notification(IDLE_TIMEOUT.as_millis() as u32, seat, &qh, ());
        shared.lock().unwrap().has_idle = true;
    }
    let manager: Option<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1> =
        globals.bind(&qh, 1..=3, ()).ok();
    shared.lock().unwrap().has_toplevels = manager.is_some();

    let mut state = State {
        shared: shared.clone(),
        pending: HashMap::new(),
        current: HashMap::new(),
    };
    std::thread::Builder::new()
        .name("an-wayland".into())
        .spawn(move || {
            // Keep the protocol objects alive for the thread's lifetime.
            let _keep = (seat, notifier, manager);
            loop {
                if let Err(e) = queue.blocking_dispatch(&mut state) {
                    tracing::warn!("wayland connection lost: {e}");
                    break;
                }
            }
        })
        .ok()?;
    Some(shared)
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_idle_notifier_v1::ExtIdleNotifierV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ext_idle_notifier_v1::ExtIdleNotifierV1,
        _: ext_idle_notifier_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut shared = state.shared.lock().unwrap();
        match event {
            ext_idle_notification_v1::Event::Idled => {
                shared.idle_since = Instant::now().checked_sub(IDLE_TIMEOUT);
            }
            ext_idle_notification_v1::Event::Resumed => shared.idle_since = None,
            _ => {}
        }
    }
}

impl Dispatch<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        _: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // New handles arrive through event_created_child below.
    }

    event_created_child!(State, zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE =>
            (zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event;
        let id = handle.id();
        let base = state.current.get(&id).cloned().unwrap_or_default();
        let pending = state.pending.entry(id.clone()).or_insert(base);
        match event {
            Event::Title { title } => pending.title = title,
            Event::AppId { app_id } => pending.app_id = app_id,
            Event::State { state: raw } => {
                // An array of u32 state values in native byte order.
                let states: Vec<u32> = raw
                    .chunks_exact(4)
                    .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                pending.activated = states.contains(&2);
                pending.fullscreen = states.contains(&3);
            }
            Event::Done => {
                if let Some(t) = state.pending.remove(&id) {
                    state.current.insert(id, t);
                }
                state.publish_focus();
            }
            Event::Closed => {
                state.pending.remove(&id);
                state.current.remove(&id);
                handle.destroy();
                state.publish_focus();
            }
            _ => {}
        }
    }
}
