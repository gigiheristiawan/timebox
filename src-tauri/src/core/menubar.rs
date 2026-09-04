//! The menu bar title, as a pure function of state (SPEC §7.1).
//!
//! Lives in the core rather than in `platform/tray.rs` so that "what the menu
//! bar says in state X" is testable without a running app — the tray module
//! only pushes the resulting string at macOS.

use super::model::{Millis, TimerState};
use super::timer_machine::MachineState;

/// The title beside the tray icon. An empty string means *icon only*, which is
/// both the IDLE presentation and what `menuBarShowTimer = false` produces.
pub fn title(state: &MachineState, now: Millis, show_timer: bool) -> String {
    if !show_timer {
        return String::new();
    }
    match state.timer_state {
        TimerState::Idle => String::new(),
        TimerState::Paused => "◉ PAUSED".to_string(),
        TimerState::AwaitingDecision => "⚠ TIME'S UP".to_string(),
        TimerState::AwaitingPomodoro => "☕ BREAK?".to_string(),
        TimerState::Running => {
            let clock = clock_str(state.remaining_ms(now));
            if state.on_break() {
                format!("◔ BREAK {clock}")
            } else {
                format!("◉ {clock}")
            }
        }
    }
}

/// `mm:ss`, or `h:mm:ss` past an hour. Rounds up so the last second is shown as
/// `00:01` rather than `00:00` — matching `clockStr` in `src/core/format.ts`.
fn clock_str(ms: Millis) -> String {
    let total = (ms.max(0) + 999) / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::*;
    use crate::core::timer_machine::{reduce, Event};

    /// A state with one task, started at `now`, so the title reflects a real
    /// running block rather than a hand-built struct.
    fn running(block_ms: Millis, now: Millis) -> (MachineState, SeqIds) {
        let mut ids = SeqIds::new("b");
        let s = MachineState {
            tasks: vec![Task::new("t1", "Write the spec", block_ms, now)],
            queue: vec!["t1".into()],
            ..Default::default()
        };
        let (s, _) = reduce(s, Event::SwitchTo { task: "t1".into() }, now, 0, &mut ids);
        (s, ids)
    }

    #[test]
    fn idle_shows_the_icon_alone() {
        assert_eq!(title(&MachineState::default(), 0, true), "");
    }

    #[test]
    fn running_work_counts_down_in_mm_ss() {
        let (s, _) = running(30 * 60_000, 0);
        assert_eq!(title(&s, 0, true), "◉ 30:00");
        assert_eq!(title(&s, 5 * 60_000 + 43_000, true), "◉ 24:17");
    }

    #[test]
    fn past_an_hour_the_title_grows_an_hours_field() {
        let (s, _) = running(90 * 60_000, 0);
        assert_eq!(title(&s, 0, true), "◉ 1:30:00");
    }

    #[test]
    fn a_break_is_labelled_so_the_glance_never_reads_as_work() {
        let (s, mut ids) = running(60_000, 0);
        let (s, _) = reduce(s, Event::Tick, 60_000, 0, &mut ids);
        let (s, _) = reduce(s, Event::DecideBreak { ms: 300_000, complete: false }, 60_000, 0, &mut ids);
        // SPEC §7.1 writes this example as `4:12`; minutes are padded here so the
        // title cannot change width mid-countdown and shove the menu bar around.
        assert_eq!(title(&s, 60_000 + 48_000, true), "◔ BREAK 04:12");
    }

    #[test]
    fn paused_and_awaiting_state_themselves_rather_than_showing_a_frozen_clock() {
        let (s, mut ids) = running(30 * 60_000, 0);
        let (paused, _) = reduce(s.clone(), Event::Pause, 60_000, 0, &mut ids);
        assert_eq!(title(&paused, 60_000, true), "◉ PAUSED");

        let (expired, _) = reduce(s, Event::Tick, 30 * 60_000, 0, &mut ids);
        assert_eq!(title(&expired, 30 * 60_000, true), "⚠ TIME'S UP");
    }

    /// The setting removes the timer, not the icon — the tray item must stay
    /// clickable, since the popover is the only other way in (SPEC §7.1).
    #[test]
    fn menu_bar_show_timer_off_yields_icon_only_in_every_state() {
        let (s, mut ids) = running(30 * 60_000, 0);
        let (expired, _) = reduce(s.clone(), Event::Tick, 30 * 60_000, 0, &mut ids);
        for st in [&s, &expired] {
            assert_eq!(title(st, 30 * 60_000, false), "");
        }
    }
}
