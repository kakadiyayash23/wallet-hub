//! Signed cookie sessions. No server-side store: the cookie carries a small
//! JSON payload and an HMAC-SHA256 over it, keyed with SESSION_SECRET.
//! Token form: `hex(payload_json).hex(hmac)`.

use axum::http::header::{HeaderMap, COOKIE};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const COOKIE_NAME: &str = "wh_session";
const MAX_AGE_SECS: i64 = 60 * 60 * 24 * 7;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Session {
    /// The logged-in player, if any.
    pub pid: Option<Uuid>,
    /// Operator role, granted by the operator key.
    #[serde(default)]
    pub ops: bool,
    /// Unix expiry.
    pub exp: i64,
}

pub fn issue(secret: &[u8], session: &Session) -> String {
    let payload = serde_json::to_vec(session).expect("session serializes");
    let payload_hex = hex::encode(&payload);
    let sig = domain::sig::hmac_sha256_hex(secret, payload_hex.as_bytes());
    format!("{payload_hex}.{sig}")
}

pub fn verify(secret: &[u8], token: &str) -> Option<Session> {
    let (payload_hex, sig) = token.split_once('.')?;
    if !domain::sig::verify_hex(secret, payload_hex.as_bytes(), sig) {
        return None;
    }
    let payload = hex::decode(payload_hex).ok()?;
    let session: Session = serde_json::from_slice(&payload).ok()?;
    if session.exp < chrono::Utc::now().timestamp() {
        return None;
    }
    Some(session)
}

pub fn new_session(pid: Option<Uuid>, ops: bool) -> Session {
    Session {
        pid,
        ops,
        exp: chrono::Utc::now().timestamp() + MAX_AGE_SECS,
    }
}

/// Read the session from the request's Cookie header.
pub fn from_headers(secret: &[u8], headers: &HeaderMap) -> Option<Session> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    let token = raw.split(';').map(str::trim).find_map(|kv| {
        kv.strip_prefix(COOKIE_NAME)
            .and_then(|rest| rest.strip_prefix('='))
    })?;
    verify(secret, token)
}

/// `Set-Cookie` value for a session. HttpOnly, SameSite=Lax; Secure when served over HTTPS.
pub fn set_cookie(token: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={MAX_AGE_SECS}{secure}")
}

pub fn clear_cookie(secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper() {
        let s = new_session(Some(Uuid::new_v4()), true);
        let token = issue(b"secret", &s);
        let back = verify(b"secret", &token).unwrap();
        assert_eq!(back.pid, s.pid);
        assert!(back.ops);
        assert!(verify(b"other", &token).is_none());
        let mut tampered = token.clone();
        tampered.replace_range(0..2, "ff");
        assert!(verify(b"secret", &tampered).is_none());
    }

    #[test]
    fn expired_is_rejected() {
        let s = Session {
            pid: None,
            ops: false,
            exp: 1,
        };
        assert!(verify(b"k", &issue(b"k", &s)).is_none());
    }

    #[test]
    fn reads_cookie_header() {
        let s = new_session(None, true);
        let token = issue(b"k", &s);
        let mut h = HeaderMap::new();
        h.insert(
            COOKIE,
            format!("a=b; {COOKIE_NAME}={token}; c=d").parse().unwrap(),
        );
        assert!(from_headers(b"k", &h).unwrap().ops);
    }
}
