//! Where nudges are kept, and the background run that writes the ones that need a search.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::core::types::agent::context::RequestContext;
use crate::core::types::nudge::{Nudge, NudgeKind, NudgePrefs, Person};
use crate::core::types::store::StoreError;

/// What was said to each student, what they set, and the notifications nudges become.
#[async_trait]
pub trait NudgeStore: Send + Sync {
    /// Students with a private conversation updated since, each once, at their latest tenant.
    async fn people(&self, since: DateTime<Utc>) -> Result<Vec<Person>, StoreError>;
    /// What the student set, or None when they set nothing.
    async fn prefs(&self, who: &Person) -> Result<Option<NudgePrefs>, StoreError>;
    /// Keeps what the student set.
    async fn set_prefs(&self, who: &Person, prefs: &NudgePrefs) -> Result<(), StoreError>;
    /// The keys among keys that were already sent to the student.
    async fn sent_keys(&self, who: &Person, keys: &[String]) -> Result<Vec<String>, StoreError>;
    /// Marks keys sent without sending anything, so later changes stand out.
    async fn mark(&self, who: &Person, kind: NudgeKind, keys: &[String]) -> Result<(), StoreError>;
    /// How many nudges the student was sent since.
    async fn count_since(&self, who: &Person, since: DateTime<Utc>) -> Result<usize, StoreError>;
    /// The text of the latest nudges of a kind, newest first.
    async fn recent(
        &self,
        who: &Person,
        kind: NudgeKind,
        limit: usize,
    ) -> Result<Vec<String>, StoreError>;
    /// Turns a nudge into a notification and marks its keys sent.
    async fn send(&self, who: &Person, nudge: &Nudge) -> Result<(), StoreError>;
    /// Records whether one notification was useful. Returns its kind when it was a nudge.
    async fn feedback(
        &self,
        who: &Person,
        id: &str,
        helpful: bool,
    ) -> Result<Option<NudgeKind>, StoreError>;
    /// Forgets sent keys older than before.
    async fn prune(&self, before: DateTime<Utc>) -> Result<u64, StoreError>;
}

/// Runs the agent in the background for one student and returns what it wrote.
#[async_trait]
pub trait Background: Send + Sync {
    /// The answer to instruction, read with the student's memories and the read-only tools.
    async fn ask(&self, ctx: &RequestContext, instruction: &str) -> Result<String, String>;
}
