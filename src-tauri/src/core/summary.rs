//! Today's numbers and the capacity strip, as pure functions of state.
//!
//! Lives in the core for the same reason the menu bar title does: "what Today
//! says after this sequence of blocks" is a product claim, and it must be
//! provable without a UI. The day boundary is *injected* — resolving local
//! midnight needs a timezone, which is a shell concern, not a domain rule.

use super::model::*;
use super::timer_machine::MachineState;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capacity {
    /// From settings: how much of the day the user has to give (SPEC §4.4).
    pub available_ms: Millis,
    /// What the queue will actually consume — a parked task counts its
    /// remainder, not a fresh allocation, so the strip never claims time a
    /// task no longer has (SPEC D10).
    pub allocated_ms: Millis,
    /// Signed: positive means room left, negative means over. Over-capacity is
    /// shown, never blocked (SPEC §7.3).
    pub unallocated_ms: Millis,
    pub over: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopTask {
    pub task_id: TaskId,
    pub title: String,
    pub ms: Millis,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Today {
    /// Work blocks only. A break is rest, not output (SPEC D7).
    pub worked_ms: Millis,
    pub break_ms: Millis,
    /// Time at unanswered checkpoints — neither work nor break (SPEC D13).
    pub away_ms: Millis,
    pub tasks_completed: usize,
    pub tasks_pending: usize,
    pub blocks_completed: usize,
    /// How often a block was set down mid-flight (SPEC D11).
    pub switched_early: u32,
    pub top: Vec<TopTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub today: Today,
    pub capacity: Capacity,
}

/// How many entries the Today list names before it stops being a list.
const TOP_N: usize = 3;

/// Time a block has consumed so far: its final figure once ended, otherwise the
/// live one. Both are already capped at the allocation by the reducer, so a
/// block reopened days later cannot report days of work (SPEC §6).
fn spent_ms(b: &TimeBlock, now: Millis) -> Millis {
    b.actual_ms.unwrap_or_else(|| b.active_ms(now).min(b.alloc_ms()))
}

/// Total time this block has spent waiting for a decision: what has been banked
/// on previous checkpoints, plus the one currently open.
fn away_of(b: &TimeBlock, current: Option<&BlockId>, awaiting: bool, now: Millis) -> Millis {
    let open = awaiting && Some(&b.id) == current;
    let live = if open { (now - b.end_at.unwrap_or(now)).max(0) } else { 0 };
    b.away_ms + live
}

/// What a task will actually get when it comes round: its remainder if it holds
/// a parked block, otherwise a full allocation.
fn queued_ms(state: &MachineState, task: &TaskId) -> Millis {
    match state.parked_for(task).and_then(|b| b.remaining_when_paused_ms) {
        Some(left) => left,
        None => state
            .tasks
            .iter()
            .find(|t| &t.id == task)
            .map_or(0, |t| t.block_duration_ms),
    }
}

pub fn summarize(
    state: &MachineState,
    day_start: Millis,
    now: Millis,
    available_ms: Millis,
) -> Summary {
    let today: Vec<&TimeBlock> = state
        .blocks
        .iter()
        .filter(|b| b.started_at.is_some_and(|t| t >= day_start))
        .collect();

    let awaiting = state.timer_state == TimerState::AwaitingDecision;
    let current = state.current_block_id.as_ref();
    let work = || today.iter().filter(|b| b.kind == BlockKind::Work);

    let mut by_task: Vec<(TaskId, Millis)> = Vec::new();
    for b in work() {
        let Some(t) = b.task_id.clone() else { continue };
        let ms = spent_ms(b, now);
        match by_task.iter_mut().find(|(id, _)| id == &t) {
            Some(e) => e.1 += ms,
            None => by_task.push((t, ms)),
        }
    }
    // Descending by time; ties keep first-worked order, so the list is stable
    // between ticks rather than shuffling on every refresh.
    by_task.sort_by_key(|(_, ms)| std::cmp::Reverse(*ms));
    let top = by_task
        .into_iter()
        .take(TOP_N)
        .filter_map(|(id, ms)| {
            state.tasks.iter().find(|t| t.id == id).map(|t| TopTask {
                task_id: id,
                title: t.title.clone(),
                ms,
            })
        })
        .collect();

    let today_summary = Today {
        worked_ms: work().map(|b| spent_ms(b, now)).sum(),
        break_ms: today
            .iter()
            .filter(|b| b.kind == BlockKind::Break)
            .map(|b| spent_ms(b, now))
            .sum(),
        away_ms: today.iter().map(|b| away_of(b, current, awaiting, now)).sum(),
        tasks_completed: state
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Done && t.completed_at.is_some_and(|c| c >= day_start))
            .count(),
        tasks_pending: state
            .tasks
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress))
            .count(),
        blocks_completed: work().filter(|b| b.status == BlockStatus::Completed).count(),
        switched_early: work().map(|b| b.interruptions).sum(),
        top,
    };

    let allocated_ms: Millis = state.queue.iter().map(|t| queued_ms(state, t)).sum();
    Summary {
        today: today_summary,
        capacity: Capacity {
            available_ms,
            allocated_ms,
            unallocated_ms: available_ms - allocated_ms,
            over: allocated_ms > available_ms,
        },
    }
}
