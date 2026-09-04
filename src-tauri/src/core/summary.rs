//! Today's numbers and the capacity strip, as pure functions of state.
//!
//! Lives in the core for the same reason the menu bar title does: "what Today
//! says after this sequence of blocks" is a product claim, and it must be
//! provable without a UI. The day boundary is *injected* — resolving local
//! midnight needs a timezone, which is a shell concern, not a domain rule.

use super::model::*;
use super::timer_machine::MachineState;
use std::collections::HashMap;

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
///
/// This is the block's whole life, not one day of it — only the fallback in
/// `spent_in_day` still reads it.
fn spent_ms(b: &TimeBlock, now: Millis) -> Millis {
    b.actual_ms.unwrap_or_else(|| b.active_ms(now).min(b.alloc_ms()))
}

/// The part of a block's running time that falls inside `day` (issue #11).
///
/// A block is not an interval: it can be parked at 23:50 and resumed at 09:00,
/// and an extension can keep it alive for days. Attributing it whole to the day
/// it *started* — which is what this did before — reported nothing at all for
/// the task a whole day had actually been spent on.
///
/// Blocks written before `work_spans` existed carry no spans, and there is no
/// way to recover their shape after the fact; those keep the old attribution so
/// that past days read as they always did rather than dropping to zero.
///
/// `day_end` is passed beside `day` because `day.1` is clipped at `now`, and the
/// fallback needs the day's real upper edge. Bounding it at both ends is
/// vacuous for today — nothing starts in the future — and load-bearing for any
/// earlier day, which the weekly report asks about: with the lower bound alone
/// a span-less block started this morning lands on *every* past day
/// (WEEKLY_REPORT D44).
fn spent_in_day(spans: &SpansByBlock, b: &TimeBlock, day: Iv, day_end: Millis, now: Millis) -> Millis {
    match spans.get(&b.id) {
        Some(ivs) => total(&intersect(ivs, &[day])).min(b.alloc_ms()),
        None if started_in(b, day.0, day_end) => spent_ms(b, now),
        None => 0,
    }
}

/// The span-less fallback's day test, in one place because the block filter and
/// `spent_in_day` must agree exactly (D44).
fn started_in(b: &TimeBlock, day_start: Millis, day_end: Millis) -> bool {
    b.started_at.is_some_and(|t| t >= day_start && t < day_end)
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
pub type Iv = (Millis, Millis);

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

/// Every block's running intervals, clipped at `now`. The open span is folded
/// in so a block still running reports the time it has spent so far.
type SpansByBlock = HashMap<BlockId, Vec<Iv>>;

fn spans_by_block(state: &MachineState, now: Millis) -> SpansByBlock {
    let mut out: SpansByBlock = HashMap::new();
    for sp in state.work_spans.iter().chain(state.open_work.iter()) {
        let iv = (sp.started_at, sp.ended_at.unwrap_or(now).min(now));
        out.entry(sp.block_id.clone()).or_default().push(iv);
    }
    for ivs in out.values_mut() {
        *ivs = union(std::mem::take(ivs));
    }
    out
}

/// Wall-clock intervals during which a *break* block was running. Used to keep
/// rest out of `outside_hours_ms`, which is about work.
fn break_intervals(spans: &SpansByBlock, today: &[&TimeBlock], now: Millis) -> Vec<Iv> {
    union(
        today
            .iter()
            .filter(|b| b.kind == BlockKind::Break)
            .flat_map(|b| match spans.get(&b.id) {
                Some(ivs) => ivs.clone(),
                // Pre-`work_spans` blocks: a break cannot be paused, so its
                // whole life is one interval.
                None => b
                    .started_at
                    .map(|st| vec![(st, b.ended_at.unwrap_or(now).min(now))])
                    .unwrap_or_default(),
            })
            .collect(),
    )
}

/// Everything one day contributes, computed in one place so that the Today
/// strip and the weekly report cannot drift apart on what a day *is*
/// (WEEKLY_REPORT D34). Issues #11 and #16 were each one subtle rule about the
/// boundary; a second implementation of them would show up as two screens in
/// the same app disagreeing about yesterday.
///
/// `by_task` is the whole map rather than the top three: a week's ranking is
/// not a merge of daily rankings, and a task placed fourth every day can be
/// second over the week (D35).
pub struct DayFigures {
    pub worked_ms: Millis,
    pub break_ms: Millis,
    pub away_ms: Millis,
    pub idle_awaiting_ms: Millis,
    pub idle_paused_ms: Millis,
    pub idle_untracked_ms: Millis,
    pub outside_hours_ms: Millis,
    pub tasks_completed: usize,
    pub blocks_completed: usize,
    /// This day's blocks only. Not summable across days — see `core::report`,
    /// which defines the week's churn over blocks that *ended* in it (D35).
    pub switched_early: u32,
    /// Descending by time; ties keep first-worked order, so the list is stable
    /// between ticks rather than shuffling on every refresh.
    pub by_task: Vec<(TaskId, Millis)>,
}

impl DayFigures {
    pub fn idle_ms(&self) -> Millis {
        self.idle_awaiting_ms + self.idle_paused_ms + self.idle_untracked_ms
    }
}

/// The day beginning at `day_start`, as of `now`. `window` is the working
/// window for that day — `None` on a non-working weekday, which is the whole of
/// IDLE_TIME D18.
pub fn day_figures(
    state: &MachineState,
    day_start: Millis,
    day_end: Millis,
    now: Millis,
    window: Option<Iv>,
) -> DayFigures {
    // Clipped at both ends: `now` is inside the day whenever the app asks about
    // today, but a block still running at the boundary must not spill the day
    // it is being asked about into the next one (issue #11). For a day already
    // past, the clamp yields the whole day; for one still to come, nothing.
    let day_iv: Iv = (day_start, now.clamp(day_start, day_end));
    let spans = spans_by_block(state, now);
    // What ran *during* the day, not what merely began in it (issue #11): a
    // block started at 23:32 and worked until 13:12 the next day belongs to
    // both days, for the part of it each day actually holds.
    let today: Vec<&TimeBlock> = state
        .blocks
        .iter()
        .filter(|b| match spans.get(&b.id) {
            Some(ivs) => !intersect(ivs, &[day_iv]).is_empty(),
            None => started_in(b, day_start, day_end),
        })
        .collect();

    let awaiting = state.timer_state == TimerState::AwaitingDecision;
    let current = state.current_block_id.as_ref();
    let work = || today.iter().filter(|b| b.kind == BlockKind::Work);
    let spent = |b: &TimeBlock| spent_in_day(&spans, b, day_iv, day_end, now);

    let mut by_task: Vec<(TaskId, Millis)> = Vec::new();
    for b in work() {
        let Some(t) = b.task_id.clone() else { continue };
        let ms = spent(b);
        match by_task.iter_mut().find(|(id, _)| id == &t) {
            Some(e) => e.1 += ms,
            None => by_task.push((t, ms)),
        }
    }
    by_task.sort_by_key(|(_, ms)| std::cmp::Reverse(*ms));

    // D19: a day on which nothing was ever started produces no idle at all.
    // Without it every Saturday, holiday and sick day reads as a wasted window.
    // The accepted cost is that a genuinely wasted working day is
    // indistinguishable from a day off — the app should not guess which.
    let day = vec![day_iv];
    let idle = idle_intervals(state, now);
    let window_iv: Vec<Iv> = match window {
        Some(w) if !today.is_empty() => intersect(&day, &union(vec![w])),
        _ => Vec::new(),
    };
    let idle_of = |r: IdleReason| {
        let ivs = &idle.iter().find(|(x, _)| *x == r).expect("all three reasons present").1;
        total(&intersect(ivs, &window_iv))
    };

    // D17/D18: work outside the window is recorded and reported; idle outside
    // it is not, because outside the window no claim of presence was made.
    let not_running = union(idle.iter().flat_map(|(_, ivs)| ivs.iter().copied()).collect());
    let work_covered = subtract(&subtract(&day, &not_running), &break_intervals(&spans, &today, now));
    let outside_hours_ms = total(&intersect(
        &work_covered,
        &subtract(&day, &union(window.into_iter().collect())),
    ));

    DayFigures {
        worked_ms: work().map(|b| spent(b)).sum(),
        break_ms: today
            .iter()
            .filter(|b| b.kind == BlockKind::Break)
            .map(|b| spent(b))
            .sum(),
        away_ms: today.iter().map(|b| away_of(b, current, awaiting, now)).sum(),
        idle_awaiting_ms: idle_of(IdleReason::Awaiting),
        idle_paused_ms: idle_of(IdleReason::Paused),
        idle_untracked_ms: idle_of(IdleReason::Untracked),
        outside_hours_ms,
        // A daily counts here exactly like any other completion (issue #16),
        // even though it stays `Todo` and stays in the queue — ticking one off
        // is a thing you did today. Bounded above by `day_end` (D44): for today
        // that is vacuous, for an earlier day it is what stops every later
        // completion from being counted on it as well.
        tasks_completed: state
            .tasks
            .iter()
            .filter(|t| {
                (t.status == TaskStatus::Done || t.daily)
                    && t.completed_at.is_some_and(|c| c >= day_start && c < day_end)
            })
            .count(),
        // Counted on the day the block *finished*, so one that ran across
        // midnight is not counted twice (issue #11).
        blocks_completed: work()
            .filter(|b| b.status == BlockStatus::Completed)
            .filter(|b| b.ended_at.is_some_and(|e| e >= day_start && e < day_end))
            .count(),
        switched_early: work().map(|b| b.interruptions).sum(),
        by_task,
    }
}

/// The `TOP_N` largest of a per-task map, resolved to titles. Shared by the day
/// and the week so both name tasks the same way.
pub fn top_tasks(state: &MachineState, by_task: Vec<(TaskId, Millis)>) -> Vec<TopTask> {
    by_task
        .into_iter()
        .take(TOP_N)
        .filter_map(|(id, ms)| {
            state.tasks.iter().find(|t| t.id == id).map(|t| TopTask {
                task_id: id,
                title: t.title.clone(),
                ms,
            })
        })
        .collect()
}

pub fn summarize(
    state: &MachineState,
    day_start: Millis,
    day_end: Millis,
    now: Millis,
    available_ms: Millis,
    window: Option<Iv>,
) -> Summary {
    let d = day_figures(state, day_start, day_end, now, window);

    let today_summary = Today {
        worked_ms: d.worked_ms,
        break_ms: d.break_ms,
        away_ms: d.away_ms,
        idle_ms: d.idle_ms(),
        idle_awaiting_ms: d.idle_awaiting_ms,
        idle_paused_ms: d.idle_paused_ms,
        idle_untracked_ms: d.idle_untracked_ms,
        outside_hours_ms: d.outside_hours_ms,
        tasks_completed: d.tasks_completed,
        // …and correspondingly stops being outstanding until tomorrow, or it
        // would be counted in both columns at once.
        tasks_pending: state
            .tasks
            .iter()
            .filter(|t| {
                matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress)
                    && !t.done_today(day_start)
            })
            .count(),
        blocks_completed: d.blocks_completed,
        switched_early: d.switched_early,
        top: top_tasks(state, d.by_task),
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
