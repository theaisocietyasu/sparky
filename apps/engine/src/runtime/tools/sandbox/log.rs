//! CommandLog: the commands a sandbox is running and the ones it ran, for the operator view.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::core::types::tools::sandbox::SandboxCommand;

/// The guard of a lock, taken back after a panic.
pub(super) fn held<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Running and ended commands. Clones share one log.
#[derive(Debug, Clone, Default)]
pub(super) struct CommandLog {
    /// The commands that ran, newest last, capped at keep.
    recent: Arc<Mutex<VecDeque<SandboxCommand>>>,
    /// Commands running right now, by the ticket they took.
    running: Arc<Mutex<Vec<(u64, SandboxCommand)>>>,
    /// Hands out a ticket per command, so one can be found again when it ends.
    ticket: Arc<AtomicU64>,
    /// Ended commands kept.
    keep: usize,
}

impl CommandLog {
    /// An empty log that keeps the last keep commands.
    pub(super) fn new(keep: usize) -> Self {
        Self {
            keep,
            ..Self::default()
        }
    }

    /// Records a command as started. The guard takes it out again however the call ends.
    pub(super) fn started(&self, mut command: SandboxCommand) -> Running {
        let ticket = self.ticket.fetch_add(1, Ordering::Relaxed);
        command.id = ticket;
        held(&self.running).push((ticket, command));
        Running {
            log: self.clone(),
            ticket,
            started: Instant::now(),
        }
    }

    /// Running commands, then ended ones, newest first.
    pub(super) fn commands(&self) -> Vec<SandboxCommand> {
        let mut out: Vec<SandboxCommand> =
            held(&self.running).iter().map(|(_, c)| c.clone()).collect();
        out.extend(held(&self.recent).iter().rev().cloned());
        out.sort_by_key(|c| std::cmp::Reverse(c.id));
        out
    }

    /// Takes the command of ticket out of the running list, if it is still there.
    fn take_running(&self, ticket: u64) -> Option<SandboxCommand> {
        let mut running = held(&self.running);
        let at = running.iter().position(|(t, _)| *t == ticket)?;
        Some(running.remove(at).1)
    }

    /// Puts an ended command in the log, dropping the oldest once the log is full.
    fn push(&self, command: SandboxCommand) {
        let mut recent = held(&self.recent);
        while recent.len() >= self.keep.max(1) {
            recent.pop_front();
        }
        recent.push_back(command);
    }
}

/// Holds one command in the running list until the call that started it is dropped.
pub(super) struct Running {
    log: CommandLog,
    ticket: u64,
    started: Instant,
}

impl Running {
    /// Moves the finished command into the log and ends the guard.
    pub(super) fn ended(self, exit_code: Option<i32>, duration_ms: u64) {
        let Some(mut command) = self.log.take_running(self.ticket) else {
            return;
        };
        command.exit_code = exit_code;
        command.duration_ms = Some(duration_ms);
        self.log.push(command);
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // A cancelled call ends here. It is recorded as ended with no exit status.
        if let Some(mut command) = self.log.take_running(self.ticket) {
            command.duration_ms = Some(
                self.started
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            );
            self.log.push(command);
        }
    }
}
