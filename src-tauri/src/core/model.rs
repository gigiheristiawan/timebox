//! Domain types. Instants and durations are milliseconds so that repeated
//! pause/resume cycles cannot accumulate rounding drift; the persistence layer
//! (Phase 3) is responsible for mapping to the schema's second-granularity
//! columns.

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch. Supplied to the reducer, never read from
/// a global clock, so every rule is testable at an arbitrary instant.
pub type Millis = i64;

pub type TaskId = String;
pub type BlockId = String;

/// Resuming a parked block with less than this left goes straight to the
/// checkpoint rather than flashing a token countdown (SPEC §11).
pub const RESUME_FLOOR_MS: Millis = 30_000;

/// Ids are supplied rather than generated, keeping the reducer deterministic.
pub trait IdSource {
    fn next_id(&mut self) -> String;
}

/// Deterministic ids for tests: b1, b2, b3…
pub struct SeqIds {
    prefix: &'static str,
    n: u32,
}

impl SeqIds {
    pub fn new(prefix: &'static str) -> Self {
        Self { prefix, n: 0 }
    }
}

impl IdSource for SeqIds {
    fn next_id(&mut self) -> String {
        self.n += 1;
        format!("{}{}", self.prefix, self.n)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerState {
    Idle,
    Running,
    Paused,
    AwaitingDecision,
}

/// Why the timer was not running. Every in-window instant no block covered
/// falls in exactly one of these, which is what makes the three sub-buckets
/// sum to `idle_ms` (IDLE_TIME §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdleReason {
    /// At an open checkpoint — the existing `away_ms` (SPEC D13).
    Awaiting,
    /// Held in PAUSED, by the Pause control or by quitting (IDLE_TIME D16).
    Paused,
    /// No block at all: before the first, between blocks, after the last.
    Untracked,
}

impl IdleReason {
    /// The reason a span opened in `state` carries. `None` for `Running`,
    /// which is the one state that is *not* idle.
    pub fn of(state: TimerState) -> Option<Self> {
        match state {
            TimerState::Running => None,
            TimerState::Idle => Some(IdleReason::Untracked),
            TimerState::Paused => Some(IdleReason::Paused),
            TimerState::AwaitingDecision => Some(IdleReason::Awaiting),
        }
    }
}

/// One interval the timer was not running. Banked on the transition rather
/// than derived afterwards, for the same reason `away_ms` is (SPEC D13):
/// after the fact a past `end_at` cannot tell a parked block from an
/// unanswered one. `ended_at` is `None` while the span is still open — and it
/// stays open across a quit, because the app not running is exactly what idle
/// measures (IDLE_TIME D15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleSpan {
    pub id: String,
    pub started_at: Millis,
    pub ended_at: Option<Millis>,
    pub reason: IdleReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    Work,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockStatus {
    Planned,
    Running,
    /// Also the state of a *parked* block — one set down by a switch, holding
    /// its remainder until the task is picked back up (SPEC D10).
    Paused,
    AwaitingDecision,
    Completed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
    pub priority: Priority,
    pub block_duration_ms: Millis,
    pub created_at: Millis,
    pub completed_at: Option<Millis>,
}

impl Task {
    pub fn new(id: impl Into<TaskId>, title: impl Into<String>, block_ms: Millis, now: Millis) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: TaskStatus::Todo,
            priority: Priority::Medium,
            block_duration_ms: block_ms,
            created_at: now,
            completed_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeBlock {
    pub id: BlockId,
    pub kind: BlockKind,
    /// `None` exactly when `kind == Break` (SPEC D7).
    pub task_id: Option<TaskId>,

    pub planned_ms: Millis,
    /// Extensions granted to THIS block. Never carried to a later block.
    pub extension_ms: Millis,
    /// Times this block was set down mid-flight (SPEC D11).
    pub interruptions: u32,
    pub actual_ms: Option<Millis>,

    pub status: BlockStatus,

    pub started_at: Option<Millis>,
    pub ended_at: Option<Millis>,
    /// Absolute instant of expiry. Recomputed on resume and extend; never
    /// decremented (SPEC §6).
    pub end_at: Option<Millis>,
    pub paused_at: Option<Millis>,
    pub remaining_when_paused_ms: Option<Millis>,
    pub accumulated_active_ms: Millis,
    pub last_resume_at: Option<Millis>,
    /// Time this block spent at an unanswered checkpoint, banked each time the
    /// checkpoint is answered (SPEC D13). Not derivable after the fact: a
    /// parked block also carries a past `end_at`, and an extended block can
    /// reach the checkpoint more than once. The *open* checkpoint's gap is
    /// still live — see `MachineState::staleness_ms`.
    pub away_ms: Millis,
}

impl TimeBlock {
    /// Total time this block was granted, extensions included.
    pub fn alloc_ms(&self) -> Millis {
        self.planned_ms + self.extension_ms
    }

    /// Time left. Constant while paused or parked; derived from `end_at` while
    /// running. Never negative.
    pub fn remaining_ms(&self, now: Millis) -> Millis {
        match self.status {
            BlockStatus::Paused => self.remaining_when_paused_ms.unwrap_or(0),
            BlockStatus::Running => (self.end_at.unwrap_or(now) - now).max(0),
            _ => 0,
        }
    }

    /// Wall time actually worked, excluding pauses. A clock moved backwards
    /// contributes zero rather than a negative (SPEC §11).
    pub fn active_ms(&self, now: Millis) -> Millis {
        let live = match (self.status, self.last_resume_at) {
            (BlockStatus::Running, Some(since)) => (now - since).max(0),
            _ => 0,
        };
        self.accumulated_active_ms + live
    }

    pub fn is_parked(&self, current: Option<&BlockId>) -> bool {
        self.status == BlockStatus::Paused && Some(&self.id) != current
    }
}

// ------------------------------------------------------------ wire encoding
//
// Enum names as stored in SQLite. Kept beside the types so a new variant
// cannot be added without a matching encoding.

macro_rules! str_enum {
    ($t:ty { $($v:ident => $s:literal),+ $(,)? }) => {
        impl $t {
            pub fn as_str(&self) -> &'static str {
                match self { $(<$t>::$v => $s),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some(<$t>::$v),)+ _ => None }
            }
        }
    };
}

str_enum!(TaskStatus {
    Todo => "TODO", InProgress => "IN_PROGRESS", Done => "DONE", Cancelled => "CANCELLED",
});
str_enum!(Priority { Low => "LOW", Medium => "MEDIUM", High => "HIGH" });
str_enum!(TimerState {
    Idle => "IDLE", Running => "RUNNING", Paused => "PAUSED",
    AwaitingDecision => "AWAITING_DECISION",
});
str_enum!(BlockKind { Work => "WORK", Break => "BREAK" });
str_enum!(IdleReason {
    Awaiting => "AWAITING", Paused => "PAUSED", Untracked => "UNTRACKED",
});
str_enum!(BlockStatus {
    Planned => "PLANNED", Running => "RUNNING", Paused => "PAUSED",
    AwaitingDecision => "AWAITING_DECISION", Completed => "COMPLETED",
    Skipped => "SKIPPED", Cancelled => "CANCELLED",
});

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn every_variant_round_trips() {
        for v in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Done, TaskStatus::Cancelled] {
            assert_eq!(TaskStatus::parse(v.as_str()), Some(v));
        }
        for v in [BlockStatus::Planned, BlockStatus::Running, BlockStatus::Paused,
                  BlockStatus::AwaitingDecision, BlockStatus::Completed,
                  BlockStatus::Skipped, BlockStatus::Cancelled] {
            assert_eq!(BlockStatus::parse(v.as_str()), Some(v));
        }
        for v in [TimerState::Idle, TimerState::Running, TimerState::Paused, TimerState::AwaitingDecision] {
            assert_eq!(TimerState::parse(v.as_str()), Some(v));
        }
        for v in [BlockKind::Work, BlockKind::Break] {
            assert_eq!(BlockKind::parse(v.as_str()), Some(v));
        }
        for v in [Priority::Low, Priority::Medium, Priority::High] {
            assert_eq!(Priority::parse(v.as_str()), Some(v));
        }
        for v in [IdleReason::Awaiting, IdleReason::Paused, IdleReason::Untracked] {
            assert_eq!(IdleReason::parse(v.as_str()), Some(v));
        }
    }

    #[test]
    fn unknown_strings_are_rejected_rather_than_defaulted() {
        assert_eq!(BlockStatus::parse("INTERRUPTED"), None);
        assert_eq!(TaskStatus::parse(""), None);
    }
}
