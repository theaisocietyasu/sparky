//! Where reminders and their notifications are kept.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::core::types::reminder::{NewReminder, Notification, Outgoing, Reminder};
use crate::core::types::store::StoreError;

/// Reminders per student, and the notifications due reminders become.
#[async_trait]
pub trait ReminderStore: Send + Sync {
    /// Keeps a reminder for the user, or returns None when they already have the most allowed.
    async fn add(
        &self,
        tenant: &str,
        user: &str,
        reminder: &NewReminder,
    ) -> Result<Option<Reminder>, StoreError>;
    /// The user's reminders, soonest first.
    async fn list(&self, tenant: &str, user: &str) -> Result<Vec<Reminder>, StoreError>;
    /// Removes one reminder of the user. Returns whether it was there.
    async fn cancel(&self, tenant: &str, user: &str, id: &str) -> Result<bool, StoreError>;
    /// Turns every reminder due by now into a notification and moves repeats on. Returns how many fired.
    async fn fire(&self, now: DateTime<Utc>) -> Result<usize, StoreError>;
    /// The user's newest notifications.
    async fn notifications(
        &self,
        tenant: &str,
        user: &str,
        limit: usize,
    ) -> Result<Vec<Notification>, StoreError>;
    /// Marks every notification of the user read.
    async fn read_all(&self, tenant: &str, user: &str) -> Result<(), StoreError>;
    /// Notifications not yet sent as a direct message, oldest first.
    async fn outgoing(&self, limit: usize) -> Result<Vec<Outgoing>, StoreError>;
    /// Marks notifications sent.
    async fn sent(&self, ids: &[String]) -> Result<(), StoreError>;
}
