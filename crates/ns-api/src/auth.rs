//! Bearer-token auth and extension pairing.

use std::sync::{Arc, Mutex, RwLock};

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use ns_core::Timestamp;
use rand::Rng;

use crate::ApiState;

/// Settings key the token is stored under.
pub const TOKEN_KEY: &str = "api.token";
const PAIR_TTL_MS: i64 = 5 * 60_000;
const PAIR_MAX_ATTEMPTS: u32 = 5;

/// 256 random bits, URL-safe base64.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Debug)]
struct PairingCode {
    code: String,
    expires: Timestamp,
    attempts: u32,
}

/// The API token plus at most one live pairing code.
#[derive(Clone, Debug)]
pub struct Auth {
    token: Arc<RwLock<String>>,
    pairing: Arc<Mutex<Option<PairingCode>>>,
}

impl Auth {
    pub fn new(token: String) -> Self {
        Self {
            token: Arc::new(RwLock::new(token)),
            pairing: Arc::new(Mutex::new(None)),
        }
    }

    pub fn token(&self) -> String {
        self.token.read().unwrap().clone()
    }

    pub fn check(&self, presented: &str) -> bool {
        constant_time_eq(presented.as_bytes(), self.token.read().unwrap().as_bytes())
    }

    /// Creates a fresh 6-digit code, replacing any previous one.
    pub fn start_pairing(&self, now: Timestamp) -> (String, Timestamp) {
        let code = format!("{:06}", rand::rng().random_range(0..1_000_000));
        let expires = now + PAIR_TTL_MS;
        *self.pairing.lock().unwrap() = Some(PairingCode {
            code: code.clone(),
            expires,
            attempts: 0,
        });
        (code, expires)
    }

    /// Exchanges a valid code for the token. A code works once; too many
    /// wrong guesses burn it.
    pub fn pair(&self, code: &str, now: Timestamp) -> Option<String> {
        let mut slot = self.pairing.lock().unwrap();
        let p = slot.as_mut()?;
        if now > p.expires {
            *slot = None;
            return None;
        }
        if constant_time_eq(p.code.as_bytes(), code.trim().as_bytes()) {
            *slot = None;
            return Some(self.token());
        }
        p.attempts += 1;
        if p.attempts >= PAIR_MAX_ATTEMPTS {
            *slot = None;
        }
        None
    }
}

fn presented_token(req: &Request) -> Option<String> {
    if let Some(v) = req.headers().get(header::AUTHORIZATION)
        && let Ok(v) = v.to_str()
        && let Some(t) = v.strip_prefix("Bearer ")
    {
        return Some(t.trim().to_string());
    }
    // EventSource cannot set headers, so the SSE stream accepts ?token=.
    if req.uri().path().ends_with("/events") {
        return req.uri().query().and_then(|q| {
            q.split('&')
                .find_map(|kv| kv.strip_prefix("token="))
                .map(str::to_string)
        });
    }
    None
}

pub async fn require_token(State(state): State<ApiState>, req: Request, next: Next) -> Response {
    match presented_token(&req) {
        Some(t) if state.auth.check(&t) => next.run(req).await,
        _ => (StatusCode::UNAUTHORIZED, "missing or invalid token").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_codes_work_once_and_expire() {
        let auth = Auth::new(generate_token());
        assert_eq!(auth.token().len(), 43);
        assert!(auth.pair("123456", 0).is_none());
        let (code, expires) = auth.start_pairing(0);
        assert_eq!(code.len(), 6);
        assert_eq!(auth.pair(&code, 1).as_deref(), Some(auth.token().as_str()));
        assert!(auth.pair(&code, 2).is_none());
        let (code, _) = auth.start_pairing(0);
        assert!(auth.pair(&code, expires + 1).is_none());
        // Wrong guesses burn the code.
        let (code, _) = auth.start_pairing(0);
        let wrong = if code == "000000" { "000001" } else { "000000" };
        for _ in 0..PAIR_MAX_ATTEMPTS {
            assert!(auth.pair(wrong, 1).is_none());
        }
        assert!(auth.pair(&code, 1).is_none());
    }
}
