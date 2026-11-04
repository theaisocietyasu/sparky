//! Web sign-in: the sealed session, the Origin check, and the login and callback routes.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use tower::ServiceExt as _;

use crate::core::traits::identity::DiscordIdentity;
use crate::core::types::http::web::{
    DiscordUser, GuildAccess, IdentityError, MANAGE_GUILD, Session,
};
use crate::routes::session::WebAuth;
use crate::routes::web::WebState;

pub(crate) const SITE: &str = "https://sparky.example.org";
const GUILD: &str = "42";

pub(crate) fn auth() -> WebAuth {
    WebAuth::new(&[3; 32], "sparky_session", SITE)
}

pub(crate) fn session(user: &str, roles: &[&str], expires_in: i64) -> Session {
    Session {
        user_id: user.into(),
        name: "Sparky Fan".into(),
        roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        expires_at: chrono::Utc::now().timestamp() + expires_in,
    }
}

/// Headers of a browser request from origin carrying the sealed session.
pub(crate) fn browser(origin: &str, sealed: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(v) = origin.parse() {
        headers.insert(header::ORIGIN, v);
    }
    if let Ok(v) = format!("theme=dark; sparky_session={sealed}").parse() {
        headers.insert(header::COOKIE, v);
    }
    headers
}

#[test]
fn a_session_opens_only_with_its_key_untouched_and_unexpired() {
    let auth = auth();
    let mine = session("111", &[], 3600);
    let Some(sealed) = auth.seal(&mine) else {
        unreachable!("the session seals")
    };
    let now = chrono::Utc::now().timestamp();
    assert_eq!(auth.open(&sealed, now), Some(mine.clone()));
    assert_eq!(auth.open(&sealed, mine.expires_at + 1), None, "expired");
    let other = WebAuth::new(&[4; 32], "sparky_session", SITE);
    assert_eq!(other.open(&sealed, now), None, "another key");
    let mut tampered = sealed.clone().into_bytes();
    if let Some(last) = tampered.last_mut() {
        *last = if *last == b'A' { b'B' } else { b'A' };
    }
    assert_eq!(auth.open(&String::from_utf8_lossy(&tampered), now), None);
    assert_eq!(auth.open("", now), None);
}

#[test]
fn a_session_is_read_only_from_the_site_origin() {
    let auth = auth();
    let Some(sealed) = auth.seal(&session("111", &[], 3600)) else {
        unreachable!("the session seals")
    };
    assert!(auth.session(&browser(SITE, &sealed)).is_some());
    assert!(
        auth.session(&browser("https://evil.example.com", &sealed))
            .is_none()
    );
    let mut no_origin = browser(SITE, &sealed);
    no_origin.remove(header::ORIGIN);
    assert!(auth.session(&no_origin).is_none());
    let mut proxied = no_origin.clone();
    proxied.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
    assert!(
        auth.session(&proxied).is_some(),
        "same-origin GET via the site proxy"
    );
    let mut cross = no_origin;
    cross.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    assert!(auth.session(&cross).is_none());
    let mut lying = browser("https://evil.example.com", &sealed);
    lying.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
    assert!(auth.session(&lying).is_none(), "a wrong Origin wins");
}

#[tokio::test]
async fn the_state_cookie_path_follows_the_public_url_path() {
    let path_of = |headers: &HeaderMap| {
        headers
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split("; ").find_map(|p| p.strip_prefix("Path=")))
            .map(str::to_owned)
    };
    let (_, direct, _) = call(router(state(vec![])), "GET", "/web/login", HeaderMap::new()).await;
    assert_eq!(path_of(&direct).as_deref(), Some("/web"));
    let mut proxied = state(vec![]);
    proxied.public_url = "https://sparky.example.org/api/engine".into();
    let (_, headers, _) = call(router(proxied), "GET", "/web/login", HeaderMap::new()).await;
    assert_eq!(path_of(&headers).as_deref(), Some("/api/engine/web"));
    assert!(location(&headers).contains("api%2Fengine%2Fweb%2Fcallback"));
}

/// A Discord that signs in one account.
struct Fake(DiscordUser);

#[async_trait]
impl DiscordIdentity for Fake {
    async fn sign_in(&self, code: &str, _redirect: &str) -> Result<DiscordUser, IdentityError> {
        if code == "bad" {
            return Err(IdentityError::Refused(400));
        }
        Ok(self.0.clone())
    }
}

fn state(guilds: Vec<GuildAccess>) -> WebState {
    WebState {
        identity: Arc::new(Fake(DiscordUser {
            id: "111".into(),
            name: "Sparky Fan".into(),
            guilds,
        })),
        auth: auth(),
        client_id: "app".into(),
        authorize_url: "https://discord.com/oauth2/authorize".into(),
        public_url: "https://engine.example.org".into(),
        guild_id: GUILD.into(),
        capability: "MANAGE_GUILD".into(),
        session_secs: 3600,
    }
}

fn router(state: WebState) -> axum::Router {
    axum::Router::new()
        .route("/web/login", axum::routing::get(crate::routes::web::login))
        .route(
            "/web/callback",
            axum::routing::get(crate::routes::web::callback),
        )
        .route("/web/me", axum::routing::get(crate::routes::web::me))
        .route(
            "/web/logout",
            axum::routing::post(crate::routes::web::logout),
        )
        .with_state(state)
}

async fn call(
    router: axum::Router,
    method: &str,
    uri: &str,
    headers: HeaderMap,
) -> (StatusCode, HeaderMap, String) {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in &headers {
        request = request.header(name, value);
    }
    let Ok(request) = request.body(Body::empty()) else {
        unreachable!("the request builds")
    };
    let response = match router.oneshot(request).await {
        Ok(response) => response,
        Err(e) => match e {},
    };
    let status = response.status();
    let headers = response.headers().clone();
    let Ok(bytes) = axum::body::to_bytes(response.into_body(), 1 << 16).await else {
        unreachable!("the body reads")
    };
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn cookies(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|v| v.split(';').next())
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

fn location(headers: &HeaderMap) -> String {
    headers
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// Runs login, then the callback with code and the state login set.
async fn sign_in(state: WebState, code: &str) -> (HeaderMap, String) {
    let app = router(state);
    let (status, headers, _) = call(app.clone(), "GET", "/web/login", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let target = location(&headers);
    assert!(
        target.starts_with("https://discord.com/oauth2/authorize?"),
        "{target}"
    );
    assert!(
        target.contains("client_id=app") && target.contains("scope=identify+guilds"),
        "{target}"
    );
    let held = cookies(&headers)
        .remove("sparky_session_state")
        .unwrap_or_default();
    assert_ne!(held, String::new());
    let mut back = HeaderMap::new();
    if let Ok(v) = format!("sparky_session_state={held}").parse() {
        back.insert(header::COOKIE, v);
    }
    let uri = format!("/web/callback?code={code}&state={held}");
    let (status, headers, _) = call(app, "GET", &uri, back).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let target = location(&headers);
    (headers, target)
}

#[tokio::test]
async fn a_member_who_manages_the_guild_is_signed_in_with_the_capability() {
    let state = state(vec![GuildAccess {
        id: GUILD.into(),
        permissions: MANAGE_GUILD,
    }]);
    let (headers, target) = sign_in(state, "good").await;
    assert_eq!(target, format!("{SITE}/chat?signin=ok"));
    let jar = cookies(&headers);
    let sealed = jar.get("sparky_session").cloned().unwrap_or_default();
    let set = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("sparky_session="))
        .unwrap_or_default()
        .to_owned();
    assert!(set.contains("HttpOnly") && set.contains("Secure"), "{set}");
    let Some(session) = auth().open(&sealed, chrono::Utc::now().timestamp()) else {
        unreachable!("the callback sealed a session")
    };
    assert_eq!(session.user_id, "111");
    assert_eq!(session.roles, vec!["MANAGE_GUILD".to_owned()]);
}

#[tokio::test]
async fn a_member_without_permissions_gets_no_roles_and_a_stranger_is_refused() {
    let member = state(vec![GuildAccess {
        id: GUILD.into(),
        permissions: 0,
    }]);
    let (headers, _) = sign_in(member, "good").await;
    let sealed = cookies(&headers)
        .remove("sparky_session")
        .unwrap_or_default();
    let session = auth().open(&sealed, chrono::Utc::now().timestamp());
    assert!(session.is_some_and(|s| s.roles.is_empty()));

    let stranger = state(vec![GuildAccess {
        id: "7".into(),
        permissions: MANAGE_GUILD,
    }]);
    let (headers, target) = sign_in(stranger, "good").await;
    assert_eq!(target, format!("{SITE}/chat?signin=not_member"));
    assert!(!cookies(&headers).contains_key("sparky_session"));
}

#[tokio::test]
async fn a_callback_without_the_state_it_was_sent_is_refused() {
    let app = router(state(vec![GuildAccess {
        id: GUILD.into(),
        permissions: 0,
    }]));
    let mut forged = HeaderMap::new();
    if let Ok(v) = "sparky_session_state=abc".parse() {
        forged.insert(header::COOKIE, v);
    }
    let (_, headers, _) = call(
        app.clone(),
        "GET",
        "/web/callback?code=good&state=xyz",
        forged,
    )
    .await;
    assert_eq!(location(&headers), format!("{SITE}/chat?signin=expired"));
    assert!(!cookies(&headers).contains_key("sparky_session"));
    let (_, headers, _) = call(
        app,
        "GET",
        "/web/callback?code=good&state=",
        HeaderMap::new(),
    )
    .await;
    assert_eq!(location(&headers), format!("{SITE}/chat?signin=expired"));
}

#[tokio::test]
async fn me_reads_the_session_and_logout_needs_the_site_origin() {
    let app = router(state(Vec::new()));
    let Some(sealed) = auth().seal(&session("111", &[], 3600)) else {
        unreachable!("the session seals")
    };
    let (status, _, body) = call(app.clone(), "GET", "/web/me", browser(SITE, &sealed)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"user_id\":\"111\""), "{body}");
    let (status, _, _) = call(app.clone(), "GET", "/web/me", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = call(
        app.clone(),
        "POST",
        "/web/logout",
        browser("https://evil.example.com", &sealed),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, headers, _) = call(app, "POST", "/web/logout", browser(SITE, &sealed)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        cookies(&headers).get("sparky_session").map(String::as_str),
        Some("")
    );
}
