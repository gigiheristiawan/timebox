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
    /// A sub-view of `idle_awaiting_ms`, kept because it is per block and is
    /// what the checkpoint's staleness line reads.
    pub away_ms: Millis,
    /// Working-window time that no running block covered (IDLE_TIME §3).
    /// A set difference, not `window - worked - break`.
    pub idle_ms: Millis,
    /// The three causes. They sum to `idle_ms` exactly (acceptance test 32).
    pub idle_awaiting_ms: Millis,
    pub idle_paused_ms: Millis,
    pub idle_untracked_ms: Millis,
    /// Work done outside the window — a signal in its own right, never
    /// subtracted from anything (IDLE_TIME §3.2).
    pub outside_hours_ms: Millis,
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

// ------------------------------------------------------------------ intervals
//
// Idle is defined as `|window \ covered|` over *intervals*, not as arithmetic
// on durations (IDLE_TIME §3). `window - worked - break` is the tempting
// one-liner and is wrong whenever a block runs past `work_end` or a paused
// block spans a gap — and it can go negative, which a duration cannot.

/// A half-open wall-clock interval `[start, end)`.
type Iv = (Millis, Millis);

fn union(mut ivs: Vec<Iv>) -> Vec<Iv> {
    ivs.retain(|(a, b)| b > a);
    ivs.sort_unstable();
    let mut out: Vec<Iv> = Vec::with_capacity(ivs.len());
    for (a, b) in ivs {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// `a ∩ b`. Both sides must already be unioned (sorted, disjoint).
fn intersect(a: &[Iv], b: &[Iv]) -> Vec<Iv> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let lo = a[i].0.max(b[j].0);
        let hi = a[i].1.min(b[j].1);
        if hi > lo {
            out.push((lo, hi));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// `a \ b`. Both sides must already be unioned.
fn subtract(a: &[Iv], b: &[Iv]) -> Vec<Iv> {
    let mut out = Vec::new();
    for &(mut lo, hi) in a {
        for &(bl, bh) in b {
            if bh <= lo {
                continue;
            }
            if bl >= hi {
                break;
            }
            if bl > lo {
                out.push((lo, bl));
            }
            lo = lo.max(bh);
            if lo >= hi {
                break;
            }
        }
        if hi > lo {
            out.push((lo, hi));
        }
    }
    out
}

fn total(ivs: &[Iv]) -> Millis {
    ivs.iter().map(|(a, b)| b - a).sum()
}

/// Every interval the timer was not running, as of `now`, split by cause.
///
/// The region before anything had ever started carries no span — nothing had
/// happened yet to leave one — so it is folded into `Untracked` here. After
/// that instant the spans are contiguous: a span opens on every departure from
/// RUNNING and closes on every return, so their complement *is* `covered`.
fn idle_intervals(state: &MachineState, now: Millis) -> [(IdleReason, Vec<Iv>); 3] {
    let anchor = state.blocks.iter().filter_map(|b| b.started_at).min();
    let mut by_reason = [
        (IdleReason::Awaiting, Vec::new()),
        (IdleReason::Paused, Vec::new()),
        (IdleReason::Untracked, Vec::new()),
    ];
    for span in state.idle_spans.iter().chain(state.open_idle.iter()) {
        let iv = (span.started_at, span.ended_at.unwrap_or(now).min(now));
        if let Some(slot) = by_reason.iter_mut().find(|(r, _)| *r == span.reason) {
            slot.1.push(iv);
        }
    }
    // Millis::MIN would overflow the subtraction in `total`; the day clip that
    // every caller applies makes any sufficiently early start equivalent.
    let dawn = anchor.unwrap_or(now);
    by_reason[2].1.push((dawn - 400 * 24 * 3_600_000, dawn));
    for slot in by_reason.iter_mut() {
        slot.1 = union(std::mem::take(&mut slot.1));
    }
    by_reason
}

/// Wall-clock intervals during which a *break* block was running. Used to keep
/// rest out of `outside_hours_ms`, which is about work.
fn break_intervals(today: &[&TimeBlock], now: Millis) -> Vec<Iv> {
    union(
        today
            .iter()
            .filter(|b| b.kind == BlockKind::Break)
            .filter_map(|b| Some((b.started_at?, b.ended_at.unwrap_or(now).min(now))))
            .collect(),
    )
}

pub fn summarize(
    state: &MachineState,
    day_start: Millis,
    now: Millis,
    available_ms: Millis,
    window: Option<Iv>,
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

    // D19: a day on which nothing was ever started produces no idle at all.
    // Without it every Saturday, holiday and sick day reads as a wasted window.
    // The accepted cost is that a genuinely wasted working day is
    // indistinguishable from a day off — the app should not guess which.
    let day = vec![(day_start, now.max(day_start))];
    let idle = idle_intervals(state, now);
    let window_iv: Vec<Iv> = match window {
        Some(w) if !today.is_empty() => intersect(&day, &union(vec![w])),
        _ => Vec::new(),
    };
    let idle_of = |r: IdleReason| {
        let ivs = &idle.iter().find(|(x, _)| *x == r).expect("all three reasons present").1;
        total(&intersect(ivs, &window_iv))
    };
    let idle_awaiting_ms = idle_of(IdleReason::Awaiting);
    let idle_paused_ms = idle_of(IdleReason::Paused);
    let idle_untracked_ms = idle_of(IdleReason::Untracked);

    // D17/D18: work outside the window is recorded and reported; idle outside
    // it is not, because outside the window no claim of presence was made.
    let not_running = union(idle.iter().flat_map(|(_, ivs)| ivs.iter().copied()).collect());
    let work_covered = subtract(&subtract(&day, &not_running), &break_intervals(&today, now));
    let outside_hours_ms = total(&intersect(
        &work_covered,
        &subtract(&day, &union(window.into_iter().collect())),
    ));

    let today_summary = Today {
        worked_ms: work().map(|b| spent_ms(b, now)).sum(),
        break_ms: today
            .iter()
            .filter(|b| b.kind == BlockKind::Break)
            .map(|b| spent_ms(b, now))
            .sum(),
        away_ms: today.iter().map(|b| away_of(b, current, awaiting, now)).sum(),
        idle_ms: idle_awaiting_ms + idle_paused_ms + idle_untracked_ms,
        idle_awaiting_ms,
        idle_paused_ms,
        idle_untracked_ms,
        outside_hours_ms,
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
