//! Local HTTP API on `127.0.0.1:47821/v1`.
//!
//! Every route except `/health` and `/pair` needs `Authorization: Bearer
//! <token>`. The dashboard gets the token from the app; the browser extension
//! gets it by pairing with a 6-digit code shown in the dashboard.

pub mod auth;
mod routes;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};

use an_core::{Activity, Clock, Config, EventBus, LocalDay, Timestamp};
use an_rules::EngineSnapshot;
use an_storage::StorageHandle;
use an_tracker::ExtEvent;
use axum::Router;
use axum::http::{HeaderValue, Method, header};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tower_http::cors::{AllowOrigin, CorsLayer};

pub use auth::{Auth, generate_token};

/// Reads and writes the config file (which holds the rules). Implemented by
/// the app, which also validates and hot-reloads.
pub trait ConfigStore: Send + Sync {
    fn read(&self) -> String;
    /// Validates, saves and applies a new config file text.
    fn write(&self, text: &str) -> Result<(), String>;
}

#[derive(Clone)]
pub struct ApiState {
    pub storage: StorageHandle,
    pub bus: EventBus,
    pub clock: Arc<dyn Clock>,
    pub config: Arc<RwLock<Config>>,
    pub config_store: Arc<dyn ConfigStore>,
    pub auth: Auth,
    pub engine: watch::Receiver<EngineSnapshot>,
    pub activity: watch::Receiver<Activity>,
    pub ext_tx: mpsc::Sender<ExtEvent>,
    /// Last heartbeat per extension client id.
    pub ext_clients: Arc<Mutex<HashMap<String, ExtClient>>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ExtClient {
    pub browser: String,
    pub version: Option<String>,
    pub last_seen: Timestamp,
}

impl ApiState {
    pub fn today(&self) -> LocalDay {
        let now = self.clock.now();
        let day_start = self.config.read().unwrap().general.day_start;
        LocalDay::from_ts(now, self.clock.offset_s(now), day_start)
    }
}

fn is_allowed_origin(origin: &HeaderValue) -> bool {
    let Ok(o) = origin.to_str() else { return false };
    o.starts_with("chrome-extension://")
        || o.starts_with("moz-extension://")
        || o.starts_with("tauri://localhost")
        || o == "http://tauri.localhost"
        || o == "https://tauri.localhost"
        || o.starts_with("http://localhost:")
        || o.starts_with("http://127.0.0.1:")
}

pub fn router(state: ApiState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|o, _| is_allowed_origin(o)))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    Router::new()
        .nest("/v1", routes::routes(state.clone()))
        .layer(cors)
        .with_state(state)
}

/// Ports tried in order; the extension probes the same list.
pub fn candidate_ports(preferred: u16) -> Vec<u16> {
    (0..4).map(|i| preferred.saturating_add(i)).collect()
}

/// Binds the first free candidate port and serves until the runtime stops.
pub async fn serve(state: ApiState, bind: &str, port: u16) -> std::io::Result<SocketAddr> {
    let mut last_err = None;
    for p in candidate_ports(port) {
        match TcpListener::bind((bind, p)).await {
            Ok(listener) => {
                let addr = listener.local_addr()?;
                let app = router(state);
                tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, app).await {
                        tracing::error!("api server stopped: {e}");
                    }
                });
                tracing::info!("api listening on http://{addr}/v1");
                return Ok(addr);
            }
            Err(e) => {
                tracing::warn!("cannot bind {bind}:{p}: {e}");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("no port to bind")))
}

#[cfg(test)]
mod tests;
