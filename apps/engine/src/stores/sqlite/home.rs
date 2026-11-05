//! The home conversation of each student: the reminders and nudges Sparky sends land there as its messages.

use chrono::{DateTime, Utc};
use sqlx::{Sqlite, Transaction};

use super::{database, stamp};
use crate::core::types::conversation::message::{Message, Notice};
use crate::core::types::conversation::{Visibility, WEB_CHANNEL, home_id};
use crate::core::types::store::StoreError;

/// Writes text as a message of Sparky in the home conversation of the student, made when missing.
/// Leaves updated_at as it was.
pub(super) async fn deliver(
    tx: &mut Transaction<'_, Sqlite>,
    tenant: &str,
    user: &str,
    notice: Notice,
    text: &str,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let id = home_id(tenant, user).to_string();
    let at = stamp(now);
    sqlx::query(
        "INSERT INTO conversations \
         (id, tenant_id, user_id, channel_id, visibility, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (id) DO NOTHING",
    )
    .bind(&id)
    .bind(tenant)
    .bind(user)
    .bind(WEB_CHANNEL)
    .bind(Visibility::Private.as_str())
    .bind(&at)
    .bind(&at)
    .execute(&mut **tx)
    .await
    .map_err(database)?;
    let body = serde_json::to_string(&Message::notice(text, notice))
        .map_err(|e| StoreError::Database(e.to_string()))?;
    sqlx::query(
        "INSERT INTO messages (conversation_id, role, content, covers, created_at) \
         VALUES (?, 'assistant', ?, NULL, ?)",
    )
    .bind(&id)
    .bind(body)
    .bind(&at)
    .execute(&mut **tx)
    .await
    .map_err(database)?;
    Ok(())
}
