//! Reminders and notifications. The web lists, reads and cancels its own with the session; the bot
//! polls what is waiting with the service token, sends each as a direct message, and says so.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;

use crate::core::traits::nudge::NudgeStore;
use crate::core::traits::reminder::ReminderStore;
use crate::core::types::http::web::Session;
use crate::core::types::nudge::{NudgeFeedback, NudgeKind, NudgePrefs, Person};
use crate::core::types::reminder::{NotificationList, OutgoingList, ReminderList, SentRequest};
use crate::routes::auth::{Caller, authorized, caller};
use crate::routes::session::WebAuth;

/// What the reminder and notification routes read.
#[derive(Clone)]
pub struct NotificationState {
    /// Where reminders are kept. None when reminders are off.
    pub store: Option<Arc<dyn ReminderStore>>,
    /// Where nudges are kept. None when nudges are off.
    pub nudges: Option<Arc<dyn NudgeStore>>,
    /// Quiet time for a student who set nothing.
    pub nudge_defaults: NudgePrefs,
    /// Tenant a web session acts in.
    pub default_tenant: String,
    /// Most notifications a list or one poll returns.
    pub list_limit: usize,
    /// Bearer token the bot calls with.
    pub service_token: SecretString,
    /// Reads web sessions. None while the web sign-in is off.
    pub web: Option<WebAuth>,
}

impl NotificationState {
    /// The store and the session of a web request, or the response that refuses it.
    fn web_caller(
        &self,
        headers: &HeaderMap,
    ) -> Result<(Arc<dyn ReminderStore>, Session), Response> {
        let session = match caller(headers, &self.service_token, self.web.as_ref())? {
            Caller::Web(session) => session,
            Caller::Service => {
                return Err((StatusCode::FORBIDDEN, "web sessions only").into_response());
            }
        };
        Ok((self.store()?, session))
    }

    /// The nudge store and the student of a web request, or the response that refuses it.
    fn nudge_caller(&self, headers: &HeaderMap) -> Result<(Arc<dyn NudgeStore>, Person), Response> {
        let (_, session) = self.web_caller(headers)?;
        let store = self.nudges.clone().ok_or_else(|| {
            (StatusCode::SERVICE_UNAVAILABLE, "nudges are not enabled").into_response()
        })?;
        let who = Person {
            tenant: self.default_tenant.clone(),
            user: session.user_id,
        };
        Ok((store, who))
    }

    /// The store, or the answer when reminders are off.
    fn store(&self) -> Result<Arc<dyn ReminderStore>, Response> {
        self.store.clone().ok_or_else(|| {
            (StatusCode::SERVICE_UNAVAILABLE, "reminders are not enabled").into_response()
        })
    }
}

/// The 500 for a store failure, logged without the text.
fn failed(error: &dyn std::fmt::Display, what: &str) -> Response {
    tracing::error!(%error, "could not {what}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("could not {what}"),
    )
        .into_response()
}

/// GET /web/notifications: the student's newest notifications and how many are unread.
pub async fn list(State(state): State<NotificationState>, headers: HeaderMap) -> Response {
    let (store, session) = match state.web_caller(&headers) {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    match store
        .notifications(&state.default_tenant, &session.user_id, state.list_limit)
        .await
    {
        Ok(notifications) => {
            let unread = notifications.iter().filter(|n| !n.read).count();
            Json(NotificationList {
                notifications,
                unread,
            })
            .into_response()
        }
        Err(error) => failed(&error, "read notifications"),
    }
}

/// POST /web/notifications/read: marks every notification of the student read.
pub async fn read(State(state): State<NotificationState>, headers: HeaderMap) -> Response {
    let (store, session) = match state.web_caller(&headers) {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    match store
        .read_all(&state.default_tenant, &session.user_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(&error, "mark notifications read"),
    }
}

/// GET /web/reminders: the student's reminders, soonest first.
pub async fn reminders(State(state): State<NotificationState>, headers: HeaderMap) -> Response {
    let (store, session) = match state.web_caller(&headers) {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    match store.list(&state.default_tenant, &session.user_id).await {
        Ok(reminders) => Json(ReminderList { reminders }).into_response(),
        Err(error) => failed(&error, "read reminders"),
    }
}

/// DELETE /web/reminders/{id}: cancels one reminder of the student.
pub async fn cancel(
    State(state): State<NotificationState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, session) = match state.web_caller(&headers) {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    match store
        .cancel(&state.default_tenant, &session.user_id, &id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "no such reminder").into_response(),
        Err(error) => failed(&error, "cancel the reminder"),
    }
}

/// GET /notifications/outgoing: notifications the bot has not sent yet. Service token only.
pub async fn outgoing(State(state): State<NotificationState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.service_token) {
        return (StatusCode::UNAUTHORIZED, "missing or wrong bearer token").into_response();
    }
    let store = match state.store() {
        Ok(store) => store,
        Err(refused) => return refused,
    };
    match store.outgoing(state.list_limit).await {
        Ok(notifications) => Json(OutgoingList { notifications }).into_response(),
        Err(error) => failed(&error, "read outgoing notifications"),
    }
}

/// POST /notifications/sent: the bot delivered these. Service token only.
pub async fn sent(
    State(state): State<NotificationState>,
    headers: HeaderMap,
    Json(req): Json<SentRequest>,
) -> Response {
    if !authorized(&headers, &state.service_token) {
        return (StatusCode::UNAUTHORIZED, "missing or wrong bearer token").into_response();
    }
    let store = match state.store() {
        Ok(store) => store,
        Err(refused) => return refused,
    };
    match store.sent(&req.ids).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failed(&error, "mark notifications sent"),
    }
}

/// GET /web/nudges: the student's quiet time and the kinds they switched off.
pub async fn nudge_prefs(State(state): State<NotificationState>, headers: HeaderMap) -> Response {
    let (store, who) = match state.nudge_caller(&headers) {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    match store.prefs(&who).await {
        Ok(prefs) => Json(prefs.unwrap_or_else(|| state.nudge_defaults.clone())).into_response(),
        Err(error) => failed(&error, "read nudge settings"),
    }
}

/// PUT /web/nudges: keeps the student's quiet time and the kinds they switched off.
pub async fn set_nudge_prefs(
    State(state): State<NotificationState>,
    headers: HeaderMap,
    Json(prefs): Json<NudgePrefs>,
) -> Response {
    let (store, who) = match state.nudge_caller(&headers) {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    if prefs.quiet_start > 23 || prefs.quiet_end > 23 {
        return (StatusCode::UNPROCESSABLE_ENTITY, "hours are 0 to 23").into_response();
    }
    match store.set_prefs(&who, &prefs).await {
        Ok(()) => Json(prefs).into_response(),
        Err(error) => failed(&error, "keep nudge settings"),
    }
}

/// What the student turned off by saying a nudge was not useful.
#[derive(Debug, serde::Serialize)]
pub struct FeedbackResult {
    /// The kind switched off, when a nudge was marked not useful.
    pub turned_off: Option<NudgeKind>,
}

/// POST /web/notifications/{id}/feedback: whether a nudge was useful. Not useful switches its kind off.
pub async fn feedback(
    State(state): State<NotificationState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<NudgeFeedback>,
) -> Response {
    let (store, who) = match state.nudge_caller(&headers) {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    let kind = match store.feedback(&who, &id, req.helpful).await {
        Ok(kind) => kind,
        Err(error) => return failed(&error, "keep the feedback"),
    };
    let turned_off = match kind {
        Some(kind) if !req.helpful => {
            let mut prefs = match store.prefs(&who).await {
                Ok(prefs) => prefs.unwrap_or_else(|| state.nudge_defaults.clone()),
                Err(error) => return failed(&error, "read nudge settings"),
            };
            if !prefs.off.contains(&kind) {
                prefs.off.push(kind);
            }
            if let Err(error) = store.set_prefs(&who, &prefs).await {
                return failed(&error, "keep nudge settings");
            }
            Some(kind)
        }
        _ => None,
    };
    Json(FeedbackResult { turned_off }).into_response()
}
