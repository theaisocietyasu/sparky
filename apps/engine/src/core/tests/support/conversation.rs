//! Conversation store doubles: a recorder and a store that keeps ownership like the database.

use std::sync::Mutex;

use async_trait::async_trait;

use crate::core::traits::conversation::ConversationStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::conversation::{Stored, Visibility};
use crate::core::types::store::StoreError;

/// One stored message in a double: its position, and what a summary covers.
#[derive(Debug, Clone)]
pub struct Row {
    pub seq: i64,
    pub message: Message,
    pub covers: Option<i64>,
}

/// Appends message to rows at the next position.
pub fn push_row(rows: &mut Vec<Row>, message: Message, covers: Option<i64>) {
    let seq = rows.last().map_or(1, |r| r.seq + 1);
    rows.push(Row {
        seq,
        message,
        covers,
    });
}

/// History from the database: newest summary at its position, then newest messages, oldest first.
pub fn history_of(rows: &[Row], limit: usize) -> Vec<Stored> {
    let summary = rows.iter().rev().find(|r| r.message.role == Role::Summary);
    let from = summary.map_or(0, |s| s.covers.unwrap_or(s.seq));
    let after: Vec<&Row> = rows
        .iter()
        .filter(|r| r.message.role != Role::Summary && r.seq > from)
        .collect();
    let skip = after.len().saturating_sub(limit);
    summary
        .map(|s| Stored {
            position: from,
            message: s.message.clone(),
        })
        .into_iter()
        .chain(after.into_iter().skip(skip).map(|r| Stored {
            position: r.seq,
            message: r.message.clone(),
        }))
        .collect()
}

/// A conversation store that remembers what the loop asked it to keep.
#[derive(Default)]
pub struct Recording {
    turns: Mutex<Vec<Message>>,
}

impl Recording {
    pub fn appended(&self) -> Vec<Message> {
        self.turns.lock().map_or_default(|t| t.clone())
    }
}

#[async_trait]
impl ConversationStore for Recording {
    async fn ensure(&self, _ctx: &RequestContext, _channel_id: &str) -> Result<(), StoreError> {
        Ok(())
    }

    async fn owns(&self, _ctx: &RequestContext) -> Result<bool, StoreError> {
        Ok(true)
    }

    async fn load(&self, _ctx: &RequestContext, _limit: usize) -> Result<Vec<Stored>, StoreError> {
        Ok(Vec::new())
    }

    async fn append_summary(
        &self,
        _ctx: &RequestContext,
        summary: &Message,
        _covers: i64,
    ) -> Result<(), StoreError> {
        if let Ok(mut kept) = self.turns.lock() {
            kept.push(summary.clone());
        }
        Ok(())
    }

    async fn latest(
        &self,
        _ctx: &RequestContext,
        _channel_id: &str,
    ) -> Result<Option<uuid::Uuid>, StoreError> {
        Ok(None)
    }

    async fn end(&self, _ctx: &RequestContext, _channel_id: &str) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn append(&self, _ctx: &RequestContext, turns: &[Message]) -> Result<(), StoreError> {
        if let Ok(mut kept) = self.turns.lock() {
            kept.extend_from_slice(turns);
        }
        Ok(())
    }
}

/// One stored conversation in the Rooms double.
struct Room {
    id: uuid::Uuid,
    tenant: String,
    user: String,
    channel: String,
    visibility: Visibility,
    ended: bool,
    touched: u64,
    rows: Vec<Row>,
}

/// A conversation store that keeps ownership, channels, visibility, turns, ends like the database.
#[derive(Default)]
pub struct Rooms {
    state: Mutex<(u64, Vec<Room>)>,
}

impl Rooms {
    fn with<T>(&self, f: impl FnOnce(&mut (u64, Vec<Room>)) -> T) -> Result<T, StoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StoreError::Database("rooms poisoned".into()))?;
        Ok(f(&mut state))
    }
}

fn owned_by(room: &Room, ctx: &RequestContext) -> bool {
    room.tenant == ctx.tenant_id && room.user == ctx.user_id
}

#[async_trait]
impl ConversationStore for Rooms {
    async fn ensure(&self, ctx: &RequestContext, channel_id: &str) -> Result<(), StoreError> {
        self.with(|(clock, rooms)| {
            if let Some(room) = rooms.iter().find(|r| r.id == ctx.conversation_id) {
                let held = owned_by(room, ctx)
                    && room.channel == channel_id
                    && room.visibility == ctx.visibility;
                return if held {
                    Ok(())
                } else {
                    Err(StoreError::NotOwned)
                };
            }
            *clock += 1;
            rooms.push(Room {
                id: ctx.conversation_id,
                tenant: ctx.tenant_id.clone(),
                user: ctx.user_id.clone(),
                channel: channel_id.to_owned(),
                visibility: ctx.visibility,
                ended: false,
                touched: *clock,
                rows: Vec::new(),
            });
            Ok(())
        })?
    }

    async fn owns(&self, ctx: &RequestContext) -> Result<bool, StoreError> {
        self.with(|(_, rooms)| {
            rooms.iter().any(|r| {
                r.id == ctx.conversation_id && owned_by(r, ctx) && r.visibility == ctx.visibility
            })
        })
    }

    async fn load(&self, ctx: &RequestContext, limit: usize) -> Result<Vec<Stored>, StoreError> {
        self.with(|(_, rooms)| {
            rooms
                .iter()
                .find(|r| r.id == ctx.conversation_id && owned_by(r, ctx))
                .map_or_default(|r| history_of(&r.rows, limit))
        })
    }

    async fn append_summary(
        &self,
        ctx: &RequestContext,
        summary: &Message,
        covers: i64,
    ) -> Result<(), StoreError> {
        self.with(|(clock, rooms)| {
            let room = rooms
                .iter_mut()
                .find(|r| r.id == ctx.conversation_id && owned_by(r, ctx))
                .ok_or(StoreError::NotOwned)?;
            *clock += 1;
            room.touched = *clock;
            push_row(&mut room.rows, summary.clone(), Some(covers));
            Ok(())
        })?
    }

    async fn append(&self, ctx: &RequestContext, turns: &[Message]) -> Result<(), StoreError> {
        self.with(|(clock, rooms)| {
            let room = rooms
                .iter_mut()
                .find(|r| r.id == ctx.conversation_id && owned_by(r, ctx))
                .ok_or(StoreError::NotOwned)?;
            *clock += 1;
            room.touched = *clock;
            for turn in turns {
                push_row(&mut room.rows, turn.clone(), None);
            }
            Ok(())
        })?
    }

    async fn latest(
        &self,
        ctx: &RequestContext,
        channel_id: &str,
    ) -> Result<Option<uuid::Uuid>, StoreError> {
        self.with(|(_, rooms)| {
            rooms
                .iter()
                .filter(|r| {
                    owned_by(r, ctx)
                        && r.channel == channel_id
                        && r.visibility == ctx.visibility
                        && !r.ended
                })
                .max_by_key(|r| r.touched)
                .map(|r| r.id)
        })
    }

    async fn end(&self, ctx: &RequestContext, channel_id: &str) -> Result<u64, StoreError> {
        self.with(|(_, rooms)| {
            let mut ended = 0;
            for room in rooms
                .iter_mut()
                .filter(|r| owned_by(r, ctx) && r.channel == channel_id && !r.ended)
            {
                room.ended = true;
                ended += 1;
            }
            ended
        })
    }
}
