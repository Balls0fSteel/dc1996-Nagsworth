use std::sync::{Arc, Mutex, RwLock};

use an_core::{Activity, Config, ControlCommand, Event, EventBus, MockClock};
use an_rules::{EngineSnapshot, RuleSet};
use an_storage::StorageHandle;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use tower::ServiceExt;

use super::*;

struct MemConfig(Mutex<String>);

impl ConfigStore for MemConfig {
    fn read(&self) -> String {
        self.0.lock().unwrap().clone()
    }

    fn write(&self, text: &str) -> Result<(), String> {
        RuleSet::from_toml(text).map_err(|e| e.to_string())?;
        *self.0.lock().unwrap() = text.to_string();
        Ok(())
    }
}

struct Fixture {
    app: Router,
    token: String,
    bus: EventBus,
    ext_rx: mpsc::Receiver<ExtEvent>,
    _engine: watch::Sender<EngineSnapshot>,
    _activity: watch::Sender<Activity>,
}

fn fixture() -> Fixture {
    let token = generate_token();
    let bus = EventBus::default();
    let (ext_tx, ext_rx) = mpsc::channel(16);
    let (etx, erx) = watch::channel(EngineSnapshot::default());
    let (atx, arx) = watch::channel(Activity::default());
    let state = ApiState {
        storage: StorageHandle::open_in_memory().unwrap(),
        bus: bus.clone(),
        clock: Arc::new(MockClock::new(1_800_000_000_000)),
        config: Arc::new(RwLock::new(Config::default())),
        config_store: Arc::new(MemConfig(Mutex::new(
            an_core::config::DEFAULT_CONFIG_TOML.to_string(),
        ))),
        auth: Auth::new(token.clone()),
        engine: erx,
        activity: arx,
        ext_tx,
        ext_clients: Arc::default(),
        backup_dir: std::env::temp_dir().join(format!("an-api-backup-{}", std::process::id())),
    };
    Fixture {
        app: router(state),
        token,
        bus,
        ext_rx,
        _engine: etx,
        _activity: atx,
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, body)
}

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::get(path);
    if let Some(t) = token {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

fn post(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut b = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = token {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn health_is_public_state_is_not() {
    let f = fixture();
    let (s, body) = send(&f.app, get("/v1/health", None)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(
        send(&f.app, get("/v1/state", None)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&f.app, get("/v1/state", Some("wrong"))).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (s, body) = send(&f.app, get("/v1/state", Some(&f.token))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["tracking_paused"], false);
}

#[tokio::test]
async fn pairing_flow() {
    let f = fixture();
    let (s, _) = send(&f.app, post("/v1/pair", None, json!({ "code": "123456" }))).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, started) = send(&f.app, post("/v1/pair/start", Some(&f.token), json!({}))).await;
    let code = started["code"].as_str().unwrap().to_string();
    let (s, body) = send(&f.app, post("/v1/pair", None, json!({ "code": code }))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["token"], f.token.as_str());
}

#[tokio::test]
async fn ext_events_are_idempotent_and_drop_incognito() {
    let mut f = fixture();
    let ev = |ts: i64, incognito: bool| {
        json!({
            "browser": "chrome", "ts": ts, "kind": "tab_activated", "tab_id": 7,
            "url": "https://example.com/secret/path?q=1", "title": "Ex", "incognito": incognito
        })
    };
    let batch = json!({ "client_id": "c1", "events": [ev(1, false), ev(2, true)] });
    let (s, body) = send(
        &f.app,
        post("/v1/ext/events", Some(&f.token), batch.clone()),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "accepted": 1, "duplicates": 0, "dropped": 1 })
    );
    let (_, body) = send(&f.app, post("/v1/ext/events", Some(&f.token), batch)).await;
    assert_eq!(body["duplicates"], 1);
    let forwarded = f.ext_rx.recv().await.unwrap();
    // store_url = host keeps only the origin.
    assert_eq!(forwarded.url.as_deref(), Some("https://example.com/"));
    assert!(f.ext_rx.try_recv().is_err());
}

#[tokio::test]
async fn control_publishes_on_the_bus() {
    let f = fixture();
    let mut rx = f.bus.subscribe();
    let (s, _) = send(
        &f.app,
        post(
            "/v1/control/pause",
            Some(&f.token),
            json!({ "minutes": 30 }),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        rx.recv().await.unwrap(),
        Event::Control(ControlCommand::PauseTracking {
            for_ms: Some(1_800_000)
        })
    );
    let req = Request::post("/v1/control/break/start")
        .header(header::AUTHORIZATION, format!("Bearer {}", f.token))
        // What PowerShell sends for a bodyless POST.
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&f.app, req).await.0, StatusCode::OK);
    assert_eq!(
        rx.recv().await.unwrap(),
        Event::Control(ControlCommand::StartBreakNow { rule_id: None })
    );
}

#[tokio::test]
async fn rules_validate_on_put() {
    let f = fixture();
    let (_, body) = send(&f.app, get("/v1/rules", Some(&f.token))).await;
    assert_eq!(body["rules"].as_array().unwrap().len(), 4);
    let bad = Request::put("/v1/rules")
        .header(header::AUTHORIZATION, format!("Bearer {}", f.token))
        .body(Body::from("[[rules]]\nid = \"x\"\n"))
        .unwrap();
    assert_eq!(send(&f.app, bad).await.0, StatusCode::BAD_REQUEST);
    let good = Request::put("/v1/rules")
        .header(header::AUTHORIZATION, format!("Bearer {}", f.token))
        .body(Body::from(
            "[[rules]]\nid = \"x\"\ntrigger = { kind = \"interval\", every = \"1m\", break_for = \"5s\" }\n",
        ))
        .unwrap();
    let (s, body) = send(&f.app, good).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["rules"][0]["id"], "x");
}

#[tokio::test]
async fn stats_and_apps_answer() {
    let f = fixture();
    for path in [
        "/v1/stats/apps",
        "/v1/stats/categories",
        "/v1/stats/daily?from=2026-11-01&to=2026-11-03",
        "/v1/stats/kinds",
        "/v1/stats/timeline?day=2026-11-02",
        "/v1/stats/web",
        "/v1/apps",
        "/v1/categories",
        "/v1/nags",
        "/v1/export",
        "/v1/export?format=csv&from=2026-11-01&to=2026-11-03",
    ] {
        let (s, body) = send(&f.app, get(path, Some(&f.token))).await;
        assert_eq!(s, StatusCode::OK, "{path}: {body}");
    }
    let (s, _) = send(
        &f.app,
        get(
            "/v1/stats/apps?from=2026-11-03&to=2026-11-01",
            Some(&f.token),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = send(
        &f.app,
        Request::patch("/v1/apps/999")
            .header(header::AUTHORIZATION, format!("Bearer {}", f.token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"is_game\":true}"))
            .unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cors_allows_extensions_only() {
    let f = fixture();
    let preflight = |origin: &str| {
        Request::builder()
            .method("OPTIONS")
            .uri("/v1/state")
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .unwrap()
    };
    let res = f
        .app
        .clone()
        .oneshot(preflight("chrome-extension://abc"))
        .await
        .unwrap();
    assert!(
        res.headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    let res = f
        .app
        .clone()
        .oneshot(preflight("https://evil.example"))
        .await
        .unwrap();
    assert!(
        !res.headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
}

#[tokio::test]
async fn backup_writes_a_database_copy() {
    let f = fixture();
    let (s, body) = send(&f.app, post("/v1/backup", Some(&f.token), json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let path = std::path::PathBuf::from(body["path"].as_str().unwrap());
    assert!(path.exists());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
