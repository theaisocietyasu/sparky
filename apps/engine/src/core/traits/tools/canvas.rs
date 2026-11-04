//! Canvas calendar feed: where each student's feed link is kept, and how a feed is read.

use async_trait::async_trait;
use secrecy::SecretString;

use crate::core::types::store::StoreError;
use crate::core::types::tools::canvas::CanvasError;

/// Each student's Canvas calendar feed link, kept sealed.
#[async_trait]
pub trait CanvasFeeds: Send + Sync {
    /// Keeps the link for user, replacing any earlier one.
    async fn save(&self, user: &str, link: &str) -> Result<(), StoreError>;
    /// The link of user, if one is kept.
    async fn load(&self, user: &str) -> Result<Option<SecretString>, StoreError>;
    /// Removes the link of user. Returns whether one was there.
    async fn delete(&self, user: &str) -> Result<bool, StoreError>;
}

/// Reads a calendar feed over the network.
#[async_trait]
pub trait CanvasCalendar: Send + Sync {
    /// The iCalendar text the link serves.
    async fn fetch(&self, link: &SecretString) -> Result<String, CanvasError>;
}
