use std::convert::Infallible;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router, middleware};
use ns_core::config::StoreUrl;
use ns_core::{Activity, CategoryKind, ControlCommand, Event, LocalDay, SegmentKind, Timestamp};
use ns_rules::{EngineSnapshot, RuleSet};
use ns_storage::StorageError;
use ns_tracker::{ExtEvent, ExtEventKind};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::require_token;
use crate::{ApiState, ExtClient};

pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> Self {
        let status = match e {
            StorageError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError(status, e.to_string())
    }
}

fn bad_request(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn routes(state: ApiState) -> Router<ApiState> {
    let protected = Router::new()
        .route("/pair/start", post(pair_start))
        .route("/ext/events", post(ext_events))
        .route("/ext/heartbeat", post(ext_heartbeat))
        .route("/state", get(get_state))
        .route("/stats/apps", get(stats_apps))
        .route("/stats/categories", get(stats_categories))
        .route("/stats/daily", get(stats_daily))
        .route("/stats/kinds", get(stats_kinds))
        .route("/stats/timeline", get(stats_timeline))
        .route("/stats/web", get(stats_web))
        .route("/apps", get(list_apps))
        .route("/apps/{id}", patch(update_app))
        .route("/categories", get(list_categories).post(create_category))
        .route("/rules", get(get_rules).put(put_rules))
        .route("/nags", get(list_nags))
        .route("/export", get(export))
        .route("/backup", post(backup))
        .route("/events", get(events))
        .route("/control/snooze", post(control_snooze))
        .route("/control/skip", post(control_skip))
        .route("/control/break/start", post(control_break))
        .route("/control/pause", post(control_pause))
        .route("/control/resume", post(control_resume))
        .route("/control/reading-mode", post(control_reading_mode))
        .route_layer(middleware::from_fn_with_state(state, require_token));
    Router::new()
        .route("/health", get(health))
        .route("/pair", post(pair))
        .merge(protected)
}

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "name": "nagsworth",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ---- pairing ----

#[derive(Deserialize)]
struct PairRequest {
    code: String,
}

async fn pair(State(s): State<ApiState>, Json(req): Json<PairRequest>) -> Response {
    match s.auth.pair(&req.code, s.clock.now()) {
        Some(token) => Json(json!({ "token": token })).into_response(),
        None => ApiError(StatusCode::FORBIDDEN, "invalid or expired code".into()).into_response(),
    }
}

async fn pair_start(State(s): State<ApiState>) -> Json<Value> {
    let (code, expires_at) = s.auth.start_pairing(s.clock.now());
    Json(json!({ "code": code, "expires_at": expires_at }))
}

// ---- extension ----

#[derive(Deserialize)]
struct ExtBatch {
    client_id: String,
    events: Vec<ExtEvent>,
}

#[derive(Serialize)]
struct ExtBatchResult {
    accepted: usize,
    duplicates: usize,
    dropped: usize,
}

fn sanitize(mut e: ExtEvent, store_url: StoreUrl) -> ExtEvent {
    if store_url == StoreUrl::Host {
        // Keep only the origin; the web segmenter derives the host from it.
        e.url = e.url.map(|u| match u.find("://") {
            Some(i) => {
                let rest = &u[i + 3..];
                let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
                format!("{}{}/", &u[..i + 3], &rest[..end])
            }
            None => u,
        });
    }
    e
}

async fn ext_events(
    State(s): State<ApiState>,
    Json(batch): Json<ExtBatch>,
) -> ApiResult<ExtBatchResult> {
    if batch.client_id.is_empty() || batch.client_id.len() > 128 {
        return Err(bad_request("client_id must be 1-128 characters"));
    }
    let (store_url, include_incognito) = {
        let c = s.config.read().unwrap();
        (c.web.store_url, c.web.include_incognito)
    };
    let mut result = ExtBatchResult {
        accepted: 0,
        duplicates: 0,
        dropped: 0,
    };
    for e in batch.events {
        if e.incognito && !include_incognito {
            result.dropped += 1;
            continue;
        }
        let (client, ts, tab) = (batch.client_id.clone(), e.ts, e.tab_id);
        let fresh = s
            .storage
            .call(move |db| db.mark_ext_event(&client, ts, tab))
            .await?;
        if !fresh {
            result.duplicates += 1;
            continue;
        }
        if s.ext_tx.send(sanitize(e, store_url)).await.is_err() {
            return Err(ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "web tracking is not running".into(),
            ));
        }
        result.accepted += 1;
    }
    Ok(Json(result))
}

#[derive(Deserialize)]
struct Heartbeat {
    client_id: String,
    browser: String,
    #[serde(default)]
    version: Option<String>,
    /// The tab that is active right now, if a browser window has focus.
    #[serde(default)]
    active: Option<ExtEvent>,
}

async fn ext_heartbeat(State(s): State<ApiState>, Json(hb): Json<Heartbeat>) -> Json<Value> {
    let now = s.clock.now();
    s.ext_clients.lock().unwrap().insert(
        hb.client_id.clone(),
        ExtClient {
            browser: hb.browser.clone(),
            version: hb.version.clone(),
            last_seen: now,
        },
    );
    let (store_url, include_incognito) = {
        let c = s.config.read().unwrap();
        (c.web.store_url, c.web.include_incognito)
    };
    if let Some(mut tab) = hb.active
        && (!tab.incognito || include_incognito)
    {
        tab.kind = ExtEventKind::Heartbeat;
        let _ = s.ext_tx.send(sanitize(tab, store_url)).await;
    }
    Json(json!({ "ok": true, "server_time": now }))
}

// ---- state ----

#[derive(Serialize)]
struct StateResponse {
    now: Timestamp,
    today: LocalDay,
    activity: Activity,
    tracking_paused: bool,
    engine: EngineSnapshot,
    extensions: Vec<(String, ExtClient)>,
}

async fn get_state(State(s): State<ApiState>) -> Json<StateResponse> {
    let activity = s.activity.borrow().clone();
    Json(StateResponse {
        now: s.clock.now(),
        today: s.today(),
        tracking_paused: activity.kind == Some(SegmentKind::Paused),
        activity,
        engine: s.engine.borrow().clone(),
        extensions: s
            .ext_clients
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    })
}

// ---- stats ----

#[derive(Deserialize)]
struct Range {
    from: Option<LocalDay>,
    to: Option<LocalDay>,
}

impl Range {
    fn resolve(&self, s: &ApiState) -> Result<(LocalDay, LocalDay), ApiError> {
        let today = s.today();
        let to = self.to.unwrap_or(today);
        let from = self.from.unwrap_or(to);
        if from > to {
            return Err(bad_request("from must not be after to"));
        }
        Ok((from, to))
    }
}

async fn stats_apps(State(s): State<ApiState>, Query(r): Query<Range>) -> ApiResult<Value> {
    let (from, to) = r.resolve(&s)?;
    let rows = s.storage.call(move |db| db.usage_by_app(from, to)).await?;
    Ok(Json(json!({ "from": from, "to": to, "apps": rows })))
}

async fn stats_categories(State(s): State<ApiState>, Query(r): Query<Range>) -> ApiResult<Value> {
    let (from, to) = r.resolve(&s)?;
    let rows = s
        .storage
        .call(move |db| db.usage_by_category(from, to))
        .await?;
    Ok(Json(json!({ "from": from, "to": to, "categories": rows })))
}

async fn stats_daily(State(s): State<ApiState>, Query(r): Query<Range>) -> ApiResult<Value> {
    let (from, to) = r.resolve(&s)?;
    let rows = s.storage.call(move |db| db.daily_usage(from, to)).await?;
    Ok(Json(json!({ "from": from, "to": to, "days": rows })))
}

async fn stats_kinds(State(s): State<ApiState>, Query(r): Query<Range>) -> ApiResult<Value> {
    let (from, to) = r.resolve(&s)?;
    let rows = s.storage.call(move |db| db.kind_totals(from, to)).await?;
    Ok(Json(json!({ "from": from, "to": to, "kinds": rows })))
}

#[derive(Deserialize)]
struct DayQuery {
    day: Option<LocalDay>,
}

async fn stats_timeline(State(s): State<ApiState>, Query(q): Query<DayQuery>) -> ApiResult<Value> {
    let day = q.day.unwrap_or_else(|| s.today());
    let rows = s.storage.call(move |db| db.timeline(day)).await?;
    Ok(Json(json!({ "day": day, "segments": rows })))
}

async fn stats_web(State(s): State<ApiState>, Query(r): Query<Range>) -> ApiResult<Value> {
    let (from, to) = r.resolve(&s)?;
    let rows = s.storage.call(move |db| db.usage_by_host(from, to)).await?;
    Ok(Json(json!({ "from": from, "to": to, "hosts": rows })))
}

// ---- apps and categories ----

async fn list_apps(State(s): State<ApiState>) -> ApiResult<Value> {
    let apps = s.storage.call(|db| db.apps()).await?;
    Ok(Json(json!({ "apps": apps })))
}

#[derive(Deserialize)]
struct AppPatch {
    category_id: Option<i64>,
    is_game: Option<bool>,
    /// `null` clears the display name; absent leaves it alone.
    #[serde(default, deserialize_with = "double_option")]
    display_name: Option<Option<String>>,
}

fn double_option<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

async fn update_app(
    State(s): State<ApiState>,
    Path(id): Path<i64>,
    Json(p): Json<AppPatch>,
) -> ApiResult<Value> {
    let app = s
        .storage
        .call(move |db| db.update_app(id, p.category_id, p.is_game, p.display_name))
        .await?;
    Ok(Json(json!({ "app": app })))
}

async fn list_categories(State(s): State<ApiState>) -> ApiResult<Value> {
    let cats = s.storage.call(|db| db.categories()).await?;
    Ok(Json(json!({ "categories": cats })))
}

#[derive(Deserialize)]
struct NewCategory {
    name: String,
    kind: CategoryKind,
    color: Option<String>,
}

async fn create_category(
    State(s): State<ApiState>,
    Json(c): Json<NewCategory>,
) -> ApiResult<Value> {
    let name = c.name.trim().to_string();
    if name.is_empty() {
        return Err(bad_request("name must not be empty"));
    }
    let cat = s
        .storage
        .call(move |db| db.create_category(&name, c.kind, c.color.as_deref()))
        .await?;
    Ok(Json(json!({ "category": cat })))
}

// ---- rules ----

async fn get_rules(State(s): State<ApiState>) -> ApiResult<Value> {
    let text = s.config_store.read();
    let rules = RuleSet::from_toml(&text).map_err(|e| e.to_string());
    Ok(Json(match rules {
        Ok(r) => json!({ "toml": text, "rules": r.rules }),
        Err(e) => json!({ "toml": text, "rules": [], "error": e }),
    }))
}

async fn put_rules(State(s): State<ApiState>, body: String) -> ApiResult<Value> {
    s.config_store.write(&body).map_err(bad_request)?;
    get_rules(State(s)).await
}

// ---- export and backup ----

#[derive(Deserialize)]
struct ExportQuery {
    from: Option<LocalDay>,
    to: Option<LocalDay>,
    #[serde(default)]
    format: ExportFormat,
}

#[derive(Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ExportFormat {
    #[default]
    Json,
    Csv,
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Segments (and web visits for JSON) of a day range, as a download.
async fn export(
    State(s): State<ApiState>,
    Query(q): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let (from, to) = Range {
        from: q.from,
        to: q.to,
    }
    .resolve(&s)?;
    let segments = s
        .storage
        .call(move |db| db.export_segments(from, to))
        .await?;
    let disposition = |ext: &str| format!("attachment; filename=\"activity-{from}-{to}.{ext}\"");
    if q.format == ExportFormat::Csv {
        let mut out = String::from("day,start,end,kind,exe,display_name,category,title\n");
        for r in &segments {
            let cols = [
                r.day.to_string(),
                r.start.to_string(),
                r.end.to_string(),
                r.kind.to_string(),
                r.exe_name.clone().unwrap_or_default(),
                r.display_name.clone().unwrap_or_default(),
                r.category.clone().unwrap_or_default(),
                r.title.clone().unwrap_or_default(),
            ];
            let line: Vec<String> = cols.iter().map(|c| csv_field(c)).collect();
            out.push_str(&line.join(","));
            out.push('\n');
        }
        return Ok((
            [
                (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
                (header::CONTENT_DISPOSITION, disposition("csv")),
            ],
            out,
        )
            .into_response());
    }
    let visits = s.storage.call(move |db| db.export_visits(from, to)).await?;
    let body = json!({ "from": from, "to": to, "segments": segments, "web_visits": visits });
    Ok((
        [(header::CONTENT_DISPOSITION, disposition("json"))],
        Json(body),
    )
        .into_response())
}

/// Writes a consistent copy of the database into the backup folder.
async fn backup(State(s): State<ApiState>) -> ApiResult<Value> {
    let name = format!("activity-{}-{}.db", s.today(), s.clock.now());
    let path = s.backup_dir.join(name);
    let p = path.clone();
    s.storage.call(move |db| db.backup_to(&p)).await?;
    Ok(Json(json!({ "path": path.display().to_string() })))
}

// ---- nags ----

#[derive(Deserialize)]
struct NagQuery {
    since: Option<Timestamp>,
    limit: Option<u32>,
}

async fn list_nags(State(s): State<ApiState>, Query(q): Query<NagQuery>) -> ApiResult<Value> {
    let since = q.since.unwrap_or(0);
    let limit = q.limit.unwrap_or(100).min(1000);
    let nags = s.storage.call(move |db| db.nags(since, limit)).await?;
    Ok(Json(json!({ "nags": nags })))
}

// ---- live events ----

async fn events(
    State(s): State<ApiState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(s.bus.subscribe()).filter_map(|e| {
        let e = e.ok()?;
        let data = serde_json::to_string(&e).ok()?;
        Some(Ok(SseEvent::default().data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ---- control ----

fn control(s: &ApiState, cmd: ControlCommand) -> Json<Value> {
    s.bus.publish(Event::Control(cmd));
    Json(json!({ "ok": true }))
}

#[derive(Deserialize)]
struct RuleRef {
    rule_id: String,
}

#[derive(Deserialize, Default)]
struct OptRuleRef {
    rule_id: Option<String>,
}

async fn control_snooze(State(s): State<ApiState>, Json(r): Json<RuleRef>) -> Json<Value> {
    control(&s, ControlCommand::Snooze { rule_id: r.rule_id })
}

async fn control_skip(State(s): State<ApiState>, Json(r): Json<RuleRef>) -> Json<Value> {
    control(&s, ControlCommand::SkipBreak { rule_id: r.rule_id })
}

/// An optional JSON body: empty means the default, whatever the content type.
fn optional_json<T: DeserializeOwned + Default>(body: &Bytes) -> Result<T, ApiError> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }
    serde_json::from_slice(body).map_err(|e| bad_request(e.to_string()))
}

async fn control_break(State(s): State<ApiState>, body: Bytes) -> ApiResult<Value> {
    let r: OptRuleRef = optional_json(&body)?;
    Ok(control(
        &s,
        ControlCommand::StartBreakNow { rule_id: r.rule_id },
    ))
}

#[derive(Deserialize, Default)]
struct PauseRequest {
    minutes: Option<u64>,
}

async fn control_pause(State(s): State<ApiState>, body: Bytes) -> ApiResult<Value> {
    let p: PauseRequest = optional_json(&body)?;
    Ok(control(
        &s,
        ControlCommand::PauseTracking {
            for_ms: p.minutes.map(|m| m * 60_000),
        },
    ))
}

async fn control_resume(State(s): State<ApiState>) -> Json<Value> {
    control(&s, ControlCommand::ResumeTracking)
}

#[derive(Deserialize)]
struct ReadingMode {
    on: bool,
}

async fn control_reading_mode(
    State(s): State<ApiState>,
    Json(r): Json<ReadingMode>,
) -> Json<Value> {
    control(&s, ControlCommand::SetReadingMode { on: r.on })
}
