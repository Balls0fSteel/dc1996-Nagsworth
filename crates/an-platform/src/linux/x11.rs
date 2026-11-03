//! X11 (and XWayland) via x11rb: the active window from EWMH properties on
//! the root window, idle time from the MIT-SCREEN-SAVER extension.

use std::sync::Mutex;

use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as _;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, Window};
use x11rb::rust_connection::RustConnection;

struct Atoms {
    net_active_window: Atom,
    net_wm_name: Atom,
    utf8_string: Atom,
    net_wm_pid: Atom,
    net_wm_state: Atom,
    net_wm_state_fullscreen: Atom,
}

pub struct X11 {
    conn: Mutex<RustConnection>,
    root: Window,
    atoms: Atoms,
    has_screensaver: bool,
}

/// What the active window tells about itself.
pub struct ActiveWindow {
    pub window: u32,
    pub title: String,
    pub pid: Option<u32>,
    pub fullscreen: bool,
}

impl X11 {
    /// Connects to `$DISPLAY`; `None` without an X server.
    pub fn connect() -> Option<Self> {
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let atom = |name: &[u8]| -> Option<Atom> {
            Some(conn.intern_atom(false, name).ok()?.reply().ok()?.atom)
        };
        let atoms = Atoms {
            net_active_window: atom(b"_NET_ACTIVE_WINDOW")?,
            net_wm_name: atom(b"_NET_WM_NAME")?,
            utf8_string: atom(b"UTF8_STRING")?,
            net_wm_pid: atom(b"_NET_WM_PID")?,
            net_wm_state: atom(b"_NET_WM_STATE")?,
            net_wm_state_fullscreen: atom(b"_NET_WM_STATE_FULLSCREEN")?,
        };
        let has_screensaver = conn
            .screensaver_query_version(1, 1)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some();
        Some(Self {
            conn: Mutex::new(conn),
            root,
            atoms,
            has_screensaver,
        })
    }

    fn u32_prop(conn: &RustConnection, win: Window, prop: Atom, ty: impl Into<Atom>) -> Vec<u32> {
        conn.get_property(false, win, prop, ty, 0, 64)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default()
    }

    pub fn active_window(&self) -> Option<ActiveWindow> {
        let conn = self.conn.lock().unwrap();
        let a = &self.atoms;
        let window =
            *Self::u32_prop(&conn, self.root, a.net_active_window, AtomEnum::WINDOW).first()?;
        if window == 0 {
            return None;
        }
        let title = conn
            .get_property(false, window, a.net_wm_name, a.utf8_string, 0, 1024)
            .ok()
            .and_then(|c| c.reply().ok())
            .filter(|r| !r.value.is_empty())
            .or_else(|| {
                conn.get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)
                    .ok()
                    .and_then(|c| c.reply().ok())
            })
            .map(|r| String::from_utf8_lossy(&r.value).into_owned())
            .unwrap_or_default();
        let pid = Self::u32_prop(&conn, window, a.net_wm_pid, AtomEnum::CARDINAL)
            .first()
            .copied();
        let fullscreen = Self::u32_prop(&conn, window, a.net_wm_state, AtomEnum::ATOM)
            .contains(&a.net_wm_state_fullscreen);
        Some(ActiveWindow {
            window,
            title,
            pid,
            fullscreen,
        })
    }

    /// Milliseconds since the last input the X server saw.
    pub fn idle_ms(&self) -> Option<u64> {
        if !self.has_screensaver {
            return None;
        }
        let conn = self.conn.lock().unwrap();
        let info = conn.screensaver_query_info(self.root).ok()?.reply().ok()?;
        Some(info.ms_since_user_input as u64)
    }
}
