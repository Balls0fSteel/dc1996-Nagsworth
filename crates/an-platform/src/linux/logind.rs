//! systemd-logind over the system D-Bus: lock the session, and watch
//! Lock/Unlock on our session and PrepareForSleep on the manager.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use an_core::{SessionEvent, SessionState, Timestamp};
use zbus::blocking::{Connection, Proxy};

use crate::SessionSink;

const DEST: &str = "org.freedesktop.login1";
const SESSION_PATH: &str = "/org/freedesktop/login1/session/auto";
const SESSION_IFACE: &str = "org.freedesktop.login1.Session";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER_IFACE: &str = "org.freedesktop.login1.Manager";

fn now_ms() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as Timestamp)
        .unwrap_or(0)
}

pub fn lock_session() -> Result<(), String> {
    let conn = Connection::system().map_err(|e| e.to_string())?;
    let proxy = Proxy::new(&conn, DEST, SESSION_PATH, SESSION_IFACE).map_err(|e| e.to_string())?;
    proxy
        .call_method("Lock", &())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Encodes a [`SessionState`] for the shared atomic.
pub fn encode(s: SessionState) -> u8 {
    match s {
        SessionState::Active => 0,
        SessionState::Locked => 1,
        SessionState::Suspended => 2,
    }
}

pub fn decode(v: u8) -> SessionState {
    match v {
        1 => SessionState::Locked,
        2 => SessionState::Suspended,
        _ => SessionState::Active,
    }
}

/// One thread per signal; each blocks on its own iterator.
pub fn watch(sink: SessionSink, state: Arc<AtomicU8>) -> Result<(), String> {
    let conn = Connection::system().map_err(|e| e.to_string())?;
    let sink = Arc::new(sink);
    let emit = {
        let state = state.clone();
        move |s: SessionState, e: SessionEvent, sink: &SessionSink| {
            state.store(encode(s), Ordering::SeqCst);
            sink(e);
        }
    };

    for (signal, locked) in [("Lock", true), ("Unlock", false)] {
        let conn = conn.clone();
        let sink = sink.clone();
        let emit = emit.clone();
        std::thread::Builder::new()
            .name(format!("an-logind-{signal}"))
            .spawn(move || {
                let Ok(proxy) = Proxy::new(&conn, DEST, SESSION_PATH, SESSION_IFACE) else {
                    return;
                };
                let Ok(signals) = proxy.receive_signal(signal) else {
                    return;
                };
                for _ in signals {
                    let at = now_ms();
                    if locked {
                        emit(SessionState::Locked, SessionEvent::Locked { at }, &sink);
                    } else {
                        emit(SessionState::Active, SessionEvent::Unlocked { at }, &sink);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }

    std::thread::Builder::new()
        .name("an-logind-sleep".into())
        .spawn(move || {
            let Ok(proxy) = Proxy::new(&conn, DEST, MANAGER_PATH, MANAGER_IFACE) else {
                return;
            };
            let Ok(signals) = proxy.receive_signal("PrepareForSleep") else {
                return;
            };
            for msg in signals {
                let at = now_ms();
                match msg.body().deserialize::<bool>() {
                    Ok(true) => emit(
                        SessionState::Suspended,
                        SessionEvent::Suspending { at },
                        &sink,
                    ),
                    Ok(false) => emit(SessionState::Active, SessionEvent::Resumed { at }, &sink),
                    Err(_) => {}
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
