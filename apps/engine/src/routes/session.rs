//! Web sessions: a sealed cookie that names a signed-in student, and the checks on a request that carries it.

use axum::http::{HeaderMap, header};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use crate::core::types::http::web::Session;

/// Length of an XChaCha20 nonce in bytes.
const NONCE_BYTES: usize = 24;

/// Header a browser sets to say where a request comes from.
const SEC_FETCH_SITE: &str = "sec-fetch-site";

pub use crate::core::types::conversation::WEB_CHANNEL;

/// What a route needs to read a web session.
#[derive(Clone)]
pub struct WebAuth {
    cipher: XChaCha20Poly1305,
    /// Name of the session cookie.
    pub cookie_name: String,
    /// The only origin a request with the cookie may come from.
    pub site: String,
}

impl WebAuth {
    /// A reader over the 32-byte session key, the cookie name, and the site origin.
    pub fn new(key: &[u8; 32], cookie_name: &str, site: &str) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(&Key::from(*key)),
            cookie_name: cookie_name.to_owned(),
            site: site.trim_end_matches('/').to_owned(),
        }
    }

    /// The cookie value for session, or None if the cipher refused.
    pub fn seal(&self, session: &Session) -> Option<String> {
        let plain = serde_json::to_vec(session).ok()?;
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let sealed = self.cipher.encrypt(&nonce, plain.as_slice()).ok()?;
        let mut bytes = nonce.to_vec();
        bytes.extend_from_slice(&sealed);
        Some(URL_SAFE_NO_PAD.encode(bytes))
    }

    /// The session in a cookie value, or None when it was not sealed here or has expired.
    pub fn open(&self, value: &str, now: i64) -> Option<Session> {
        let bytes = URL_SAFE_NO_PAD.decode(value.trim()).ok()?;
        if bytes.len() <= NONCE_BYTES {
            return None;
        }
        let (nonce, sealed) = bytes.split_at(NONCE_BYTES);
        let nonce: [u8; NONCE_BYTES] = nonce.try_into().ok()?;
        let plain = self.cipher.decrypt(&XNonce::from(nonce), sealed).ok()?;
        let session: Session = serde_json::from_slice(&plain).ok()?;
        (session.expires_at > now).then_some(session)
    }

    /// True when the request comes from the site: its Origin is the site, or, with no Origin, the browser marks it same-origin behind the site proxy.
    pub fn is_from_site(&self, headers: &HeaderMap) -> bool {
        match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            Some(origin) => origin.trim_end_matches('/') == self.site,
            None => headers
                .get(SEC_FETCH_SITE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == "same-origin"),
        }
    }

    /// The session of a request: a valid cookie, sent from the site.
    pub fn session(&self, headers: &HeaderMap) -> Option<Session> {
        if !self.is_from_site(headers) {
            return None;
        }
        let value = cookie(headers, &self.cookie_name)?;
        self.open(&value, chrono::Utc::now().timestamp())
    }
}

/// The value of the cookie named name in the request, if sent.
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
}

/// A Set-Cookie value: HttpOnly and Secure, sent on cross-site requests, for max_age seconds.
pub fn set_cookie(name: &str, value: &str, path: &str, max_age: i64) -> String {
    format!("{name}={value}; Path={path}; Max-Age={max_age}; HttpOnly; Secure; SameSite=None")
}
