//! The weekly report (issue #6): seven days of already-recorded time, as a pure
//! function of `MachineState` plus the calendar.
//!
//! It adds no `Event`, no `Effect` and no column — nothing here can change the
//! timer, the queue or a task. Like `core::summary`, the calendar is *injected*:
//! resolving "which Monday" needs a timezone, which is a shell concern
//! (`state::week_start_ms`, `state::week_context`).

use super::model::*;
use super::summary::{day_figures, top_tasks, DayFigures, Iv, TopTask};
use super::timer_machine::MachineState;

/// One day, resolved by the shell. `state::week_context` builds seven.
#[derive(Debug, Clone, Copy)]
pub struct DayCtx {
    /// Local midnight.
    pub day_start: Millis,
    /// The next local midnight — not `+24h`, a DST day is 23 or 25 hours.
    pub day_end: Millis,
    /// `None` on a non-working weekday (IDLE_TIME D18).
    pub window: Option<Iv>,
    /// `settings.available_work_ms_per_day`.
    pub available_ms: Millis,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayReport {
    pub day_start: Millis,
    /// 0 = Monday … 6 = Sunday.
    pub weekday: u8,
    /// A label only. The day is scored either way — see `target_ms`.
    pub working_day: bool,
    /// The day's share of the plan: capacity on a working day, **0** otherwise.
    /// A real zero, not an absent target: work done on a day off is over
    /// target, which is the same stance `outside_hours_ms` takes on work
    /// outside the window (WEEKLY_REPORT D36).
    pub target_ms: Millis,
    pub worked_ms: Millis,
    pub break_ms: Millis,
    pub idle_ms: Millis,
    pub idle_awaiting_ms: Millis,
    pub idle_paused_ms: Millis,
    pub idle_untracked_ms: Millis,
    pub outside_hours_ms: Millis,
    pub tasks_completed: usize,
    pub blocks_completed: usize,
}

/// The week's figures.
///
/// Everything measured from spans or bucketed into exactly one day is the sum
/// of the seven days. `switched_early` is not: `interruptions` is a *lifetime*
/// counter on a block with no per-switch timestamp, and a block outlives a week
/// — so the week defines churn over the blocks it can attribute exactly once,
/// the ones that ended in it (D35).
///
/// `away_ms` is absent for the same reason and has no such repair:
/// `idle_awaiting_ms` measures the same waiting from `idle_spans`, is
/// interval-based, and is correctly week-scoped.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekTotals {
    pub worked_ms: Millis,
    pub break_ms: Millis,
    pub idle_ms: Millis,
    pub idle_awaiting_ms: Millis,
    pub idle_paused_ms: Millis,
    pub idle_untracked_ms: Millis,
    pub outside_hours_ms: Millis,
    pub tasks_completed: usize,
    pub blocks_completed: usize,
    /// Over the work blocks that *ended* within the week (D35).
    pub switched_early: u32,
    /// `available_ms` × working weekdays. The full week's, even mid-week (D36).
    pub target_ms: Millis,
    pub working_days: usize,
    /// Days with any worked time at all.
    pub days_worked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekReport {
    /// Monday, local midnight.
    pub week_start: Millis,
    /// The following Monday, exclusive.
    pub week_end: Millis,
    /// 0 = the week containing now, -1 = the week before.
    pub offset: i32,
    pub is_current_week: bool,
    /// Always seven, Monday first, zeros included (D39).
    pub days: Vec<DayReport>,
    pub totals: WeekTotals,
    /// Ranked over the whole week, not merged from the daily rankings (D35).
    pub top: Vec<TopTask>,
}

pub fn report(state: &MachineState, days: &[DayCtx], offset: i32, now: Millis) -> WeekReport {
    let week_start = days.first().map_or(0, |d| d.day_start);
    let week_end = days.last().map_or(0, |d| d.day_end);

    let figures: Vec<(DayCtx, DayFigures)> = days
        .iter()
        .map(|c| (*c, day_figures(state, c.day_start, c.day_end, now, c.window)))
        .collect();

    let mut by_task: Vec<(TaskId, Millis)> = Vec::new();
    for (_, d) in &figures {
        for (id, ms) in &d.by_task {
            match by_task.iter_mut().find(|(x, _)| x == id) {
                Some(e) => e.1 += *ms,
                None => by_task.push((id.clone(), *ms)),
            }
        }
    }
    by_task.sort_by_key(|(_, ms)| std::cmp::Reverse(*ms));

    let rows: Vec<DayReport> = figures
        .iter()
        .enumerate()
        .map(|(i, (c, d))| DayReport {
            day_start: c.day_start,
            weekday: i as u8,
            working_day: c.window.is_some(),
            target_ms: if c.window.is_some() { c.available_ms } else { 0 },
            worked_ms: d.worked_ms,
            break_ms: d.break_ms,
            idle_ms: d.idle_ms(),
            idle_awaiting_ms: d.idle_awaiting_ms,
            idle_paused_ms: d.idle_paused_ms,
            idle_untracked_ms: d.idle_untracked_ms,
            outside_hours_ms: d.outside_hours_ms,
            tasks_completed: d.tasks_completed,
            blocks_completed: d.blocks_completed,
        })
        .collect();

    // The one figure that is not a sum of the rows (D35). Bucketed on the day
    // the block *ended*, exactly as `blocks_completed` is, so a block parked
    // across three weeks reports its switches once — in the week it finished —
    // rather than into whichever week is being looked at.
    let switched_early: u32 = state
        .blocks
        .iter()
        .filter(|b| b.kind == BlockKind::Work)
        .filter(|b| b.ended_at.is_some_and(|e| e >= week_start && e < week_end))
        .map(|b| b.interruptions)
        .sum();

    let sum = |f: fn(&DayReport) -> Millis| rows.iter().map(f).sum::<Millis>();
    let totals = WeekTotals {
        worked_ms: sum(|r| r.worked_ms),
        break_ms: sum(|r| r.break_ms),
        idle_ms: sum(|r| r.idle_ms),
        idle_awaiting_ms: sum(|r| r.idle_awaiting_ms),
        idle_paused_ms: sum(|r| r.idle_paused_ms),
        idle_untracked_ms: sum(|r| r.idle_untracked_ms),
        outside_hours_ms: sum(|r| r.outside_hours_ms),
        tasks_completed: rows.iter().map(|r| r.tasks_completed).sum(),
        blocks_completed: rows.iter().map(|r| r.blocks_completed).sum(),
        switched_early,
        target_ms: rows.iter().map(|r| r.target_ms).sum(),
        working_days: rows.iter().filter(|r| r.working_day).count(),
        days_worked: rows.iter().filter(|r| r.worked_ms > 0).count(),
    };

    WeekReport {
        week_start,
        week_end,
        offset,
        is_current_week: offset == 0,
        days: rows,
        totals,
        top: top_tasks(state, by_task),
    }
}
