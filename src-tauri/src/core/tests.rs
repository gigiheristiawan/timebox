//! Acceptance tests from SPEC §12, as pure reducer tests with an injected clock.
//! Test numbers match the spec exactly so a failure names its requirement.

use super::model::*;
use super::timer_machine::*;

const MIN: Millis = 60_000;

/// A day with tasks A(30m) B(45m) C(30m) D(45m), queued in that order.
fn day() -> (MachineState, SeqIds) {
    let now = 0;
    let mut s = MachineState::default();
    for (id, title, mins) in [
        ("A", "Fix attendance bug", 30),
        ("B", "Payroll calculation", 45),
        ("C", "Code review", 30),
        ("D", "Client proposal", 45),
    ] {
        s.tasks.push(Task::new(id, title, mins * MIN, now));
        s.queue.push(id.to_string());
    }
    (s, SeqIds::new("b"))
}

fn fire(s: MachineState, e: Event, now: Millis, ids: &mut SeqIds) -> MachineState {
    reduce(s, e, now, ids).0
}

fn fire_fx(s: MachineState, e: Event, now: Millis, ids: &mut SeqIds) -> (MachineState, Vec<Effect>) {
    reduce(s, e, now, ids)
}

fn status_of(s: &MachineState, id: &str) -> TaskStatus {
    s.tasks.iter().find(|t| t.id == id).unwrap().status
}

fn blocks_for<'a>(s: &'a MachineState, task: &str) -> Vec<&'a TimeBlock> {
    s.blocks.iter().filter(|b| b.task_id.as_deref() == Some(task)).collect()
}

fn start_a(now: Millis) -> (MachineState, SeqIds) {
    let (s, mut ids) = day();
    let s = fire(s, Event::SwitchTo { task: "A".into() }, now, &mut ids);
    (s, ids)
}

// ---------------------------------------------------------------- Test 1–4

#[test]
fn t1_expiration_halts_at_a_decision() {
    let (s, mut ids) = start_a(0);
    let (s, fx) = fire_fx(s, Event::Tick, 30 * MIN, &mut ids);

    assert_eq!(s.timer_state, TimerState::AwaitingDecision);
    assert_eq!(status_of(&s, "A"), TaskStatus::InProgress, "A must not be done");
    assert!(fx.iter().any(|f| matches!(f, Effect::EnterCheckpoint { .. })));
    // B must NOT have started on its own — that is the whole product.
    assert_eq!(s.current_task().unwrap().id, "A");
    assert!(blocks_for(&s, "B").is_empty());
}

#[test]
fn t2_complete_and_start_next() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideComplete, 30 * MIN, &mut ids);

    assert_eq!(status_of(&s, "A"), TaskStatus::Done);
    assert_eq!(s.timer_state, TimerState::Running);
    assert_eq!(s.current_task().unwrap().id, "B");
    assert!(!s.queue.contains(&"A".to_string()));
}

#[test]
fn t3_keep_pending_and_start_next() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecidePending, 30 * MIN, &mut ids);

    assert_eq!(status_of(&s, "A"), TaskStatus::InProgress, "pending is NOT done");
    assert_eq!(s.current_task().unwrap().id, "B");
    assert_eq!(s.queue, vec!["B", "C", "D", "A"], "A rotates to the tail");
}

#[test]
fn t4_extend() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideExtend { ms: 10 * MIN }, 30 * MIN, &mut ids);

    assert_eq!(s.timer_state, TimerState::Running);
    assert_eq!(s.remaining_ms(30 * MIN), 10 * MIN);
    assert_eq!(s.current_block().unwrap().extension_ms, 10 * MIN);
    assert_eq!(status_of(&s, "A"), TaskStatus::InProgress, "extending never completes");
}

// ------------------------------------------------------------- Test 8–9

#[test]
fn t8_manual_completion_is_independent_of_the_timer() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::CompleteCurrentTask, 10 * MIN, &mut ids);

    assert_eq!(status_of(&s, "A"), TaskStatus::Done);
    let b = blocks_for(&s, "A")[0];
    assert_eq!(b.status, BlockStatus::Completed);
    assert_eq!(b.actual_ms, Some(10 * MIN), "records 10m, not the 30m allocation");
}

#[test]
fn t9_block_completion_is_not_task_completion() {
    // The single most important guarantee in the product.
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecidePending, 30 * MIN, &mut ids);

    let b = blocks_for(&s, "A")[0];
    assert_eq!(b.status, BlockStatus::Completed, "the BLOCK completed");
    assert_ne!(status_of(&s, "A"), TaskStatus::Done, "the TASK did not");
}

// ------------------------------------------------------------ Test 10–12

#[test]
fn t10_pausing_does_not_consume_the_allocation() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Pause, 5 * MIN, &mut ids);
    assert_eq!(s.remaining_ms(5 * MIN), 25 * MIN);

    // Ten minutes pass while paused.
    assert_eq!(s.remaining_ms(15 * MIN), 25 * MIN, "paused time is not spent");

    let s = fire(s, Event::Resume, 15 * MIN, &mut ids);
    assert_eq!(s.remaining_ms(15 * MIN), 25 * MIN);
    assert_eq!(s.current_block().unwrap().end_at, Some(40 * MIN));
}

#[test]
fn t11_skip_records_elapsed_and_rotates() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Skip, 7 * MIN, &mut ids);

    let b = blocks_for(&s, "A")[0];
    assert_eq!(b.status, BlockStatus::Skipped);
    assert_eq!(b.actual_ms, Some(7 * MIN));
    assert_eq!(status_of(&s, "A"), TaskStatus::InProgress);
    assert_eq!(s.queue, vec!["B", "C", "D", "A"]);
    assert_eq!(s.current_task().unwrap().id, "B");
}

#[test]
fn t12_requeued_block_equals_the_task_duration_ignoring_extensions() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideExtend { ms: 10 * MIN }, 30 * MIN, &mut ids);
    let s = fire(s, Event::Tick, 40 * MIN, &mut ids);
    let s = fire(s, Event::DecidePending, 40 * MIN, &mut ids); // A -> tail
    // Run the queue back around to A.
    let mut s = s;
    for (i, _) in ["B", "C", "D"].iter().enumerate() {
        let t = 40 * MIN + (i as i64 + 1) * 100 * MIN;
        s = fire(s, Event::Tick, t, &mut ids);
        s = fire(s, Event::DecidePending, t, &mut ids);
    }
    assert_eq!(s.current_task().unwrap().id, "A");
    assert_eq!(
        s.current_block().unwrap().planned_ms,
        30 * MIN,
        "a fresh block is the task's duration; the earlier +10m does not carry"
    );
    assert_eq!(s.current_block().unwrap().extension_ms, 0);
}

// ------------------------------------------------------------ Test 13–16

#[test]
fn t13_mid_block_switch_parks_the_block() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::SwitchTo { task: "C".into() }, 5 * MIN, &mut ids);

    let parked = s.parked_for(&"A".to_string()).expect("A keeps a parked block");
    assert_eq!(parked.status, BlockStatus::Paused);
    assert_eq!(parked.remaining_when_paused_ms, Some(25 * MIN));
    assert_eq!(parked.interruptions, 1);
    assert_eq!(status_of(&s, "A"), TaskStatus::InProgress);
    assert_eq!(s.queue, vec!["C", "B", "D", "A"]);
    assert_eq!(s.current_task().unwrap().id, "C");
    assert_eq!(s.remaining_ms(5 * MIN), 30 * MIN, "C gets a fresh block");
}

#[test]
fn t14_return_resumes_the_remainder() {
    // The anti-loophole test.
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::SwitchTo { task: "C".into() }, 5 * MIN, &mut ids);
    let s = fire(s, Event::SwitchTo { task: "A".into() }, 8 * MIN, &mut ids);

    assert_eq!(s.current_task().unwrap().id, "A");
    assert_eq!(s.remaining_ms(8 * MIN), 25 * MIN, "25 minutes left, NOT a fresh 30");
    assert_eq!(blocks_for(&s, "A").len(), 1, "one block for A, never two");
    assert_eq!(s.current_block().unwrap().end_at, Some(33 * MIN));
}

#[test]
fn t15_switching_cannot_farm_time() {
    let (s, mut ids) = start_a(0);
    let mut s = s;
    let mut t = 0;
    // Bounce away and back ten times, one minute of work each visit.
    for _ in 0..10 {
        t += MIN;
        s = fire(s, Event::SwitchTo { task: "C".into() }, t, &mut ids);
        s = fire(s, Event::SwitchTo { task: "A".into() }, t, &mut ids);
    }
    assert_eq!(s.current_task().unwrap().id, "A");
    assert_eq!(blocks_for(&s, "A").len(), 1, "still exactly one block for A");
    assert_eq!(
        s.remaining_ms(t),
        20 * MIN,
        "ten minutes were worked, so ten are gone — the 30m never resets"
    );
    assert_eq!(blocks_for(&s, "A")[0].interruptions, 10);

    // And it still reaches a checkpoint at the original 30-minute boundary.
    let s = fire(s, Event::Tick, t + 20 * MIN, &mut ids);
    assert_eq!(s.timer_state, TimerState::AwaitingDecision);
}

#[test]
fn t16_switch_is_not_a_skip() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::SwitchTo { task: "C".into() }, 5 * MIN, &mut ids);
    let s = fire(s, Event::Skip, 9 * MIN, &mut ids);

    let a = blocks_for(&s, "A")[0];
    let c = blocks_for(&s, "C")[0];
    assert_eq!(a.status, BlockStatus::Paused, "A was parked, not skipped");
    assert_eq!(a.interruptions, 1);
    assert_eq!(c.status, BlockStatus::Skipped, "C was skipped, not parked");
    assert_eq!(c.interruptions, 0);
}

// ------------------------------------------------------------ Test 17–19

#[test]
fn t17_keep_pending_and_break() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideBreak { ms: 10 * MIN, complete: false }, 30 * MIN, &mut ids);

    assert!(s.on_break());
    assert_eq!(s.remaining_ms(30 * MIN), 10 * MIN);
    assert_ne!(status_of(&s, "A"), TaskStatus::Done);
    assert_eq!(s.queue, vec!["B", "C", "D", "A"]);
    assert_eq!(blocks_for(&s, "A")[0].status, BlockStatus::Completed);
}

#[test]
fn t17b_complete_and_break() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideBreak { ms: 15 * MIN, complete: true }, 30 * MIN, &mut ids);

    assert!(s.on_break());
    assert_eq!(status_of(&s, "A"), TaskStatus::Done);
    assert!(!s.queue.contains(&"A".to_string()));
    assert_eq!(s.queue.first().unwrap(), "B", "next up is B, not the finished A");
}

#[test]
fn t18_break_does_not_auto_advance() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideBreak { ms: 10 * MIN, complete: false }, 30 * MIN, &mut ids);
    let s = fire(s, Event::Tick, 40 * MIN, &mut ids);

    assert_eq!(s.timer_state, TimerState::AwaitingDecision);
    assert!(s.on_break(), "the checkpoint belongs to the break block");
    assert!(blocks_for(&s, "B").is_empty(), "B must not have started by itself");

    let s = fire(s, Event::EndBreak, 45 * MIN, &mut ids);
    assert_eq!(s.current_task().unwrap().id, "B");
    assert_eq!(s.timer_state, TimerState::Running);
}

#[test]
fn t19_break_accounting_is_separate() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideBreak { ms: 10 * MIN, complete: false }, 30 * MIN, &mut ids);
    let s = fire(s, Event::EndBreak, 40 * MIN, &mut ids);

    let worked: Millis = s.blocks.iter()
        .filter(|b| b.kind == BlockKind::Work)
        .filter_map(|b| b.actual_ms).sum();
    let rested: Millis = s.blocks.iter()
        .filter(|b| b.kind == BlockKind::Break)
        .filter_map(|b| b.actual_ms).sum();

    assert_eq!(worked, 30 * MIN, "the break is not work");
    assert_eq!(rested, 10 * MIN);
    assert!(blocks_for(&s, "A").iter().all(|b| b.kind == BlockKind::Work),
            "no break block is ever attributed to a task");
}

// ----------------------------------------------------- Test 20 (staleness)

#[test]
fn t20_staleness_is_measured_from_expiry() {
    let (s, mut ids) = start_a(0);
    assert_eq!(s.staleness_ms(10 * MIN), None, "not at a checkpoint");

    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    assert_eq!(s.staleness_ms(30 * MIN), Some(0));
    assert_eq!(s.staleness_ms(30 * MIN + 3 * MIN), Some(3 * MIN));

    // Answering it credits the task with the allocation, never the idle gap.
    let s = fire(s, Event::DecidePending, 30 * MIN + 134 * MIN, &mut ids);
    assert_eq!(blocks_for(&s, "A")[0].actual_ms, Some(30 * MIN));
}

// ------------------------------------------------------- invariants & edges

#[test]
fn checkpoint_has_no_side_doors() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);

    for e in [
        Event::SwitchTo { task: "C".into() },
        Event::Pause,
        Event::Resume,
    ] {
        let (after, _) = fire_fx(s.clone(), e, 31 * MIN, &mut ids);
        assert_eq!(after.timer_state, TimerState::AwaitingDecision,
                   "nothing but a decision may leave the checkpoint");
        assert_eq!(after.current_task().unwrap().id, "A");
    }
}

#[test]
fn switching_during_a_break_ends_it_without_recording_an_interruption() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideBreak { ms: 10 * MIN, complete: false }, 30 * MIN, &mut ids);
    let s = fire(s, Event::SwitchTo { task: "C".into() }, 33 * MIN, &mut ids);

    assert_eq!(s.current_task().unwrap().id, "C");
    let brk = s.blocks.iter().find(|b| b.kind == BlockKind::Break).unwrap();
    assert_eq!(brk.status, BlockStatus::Completed);
    assert_eq!(brk.interruptions, 0);
}

#[test]
fn resuming_a_nearly_spent_block_goes_straight_to_the_checkpoint() {
    let (s, mut ids) = start_a(0);
    // Switch away with 20 seconds left — under the resume floor.
    let s = fire(s, Event::SwitchTo { task: "C".into() }, 30 * MIN - 20_000, &mut ids);
    let s = fire(s, Event::SwitchTo { task: "A".into() }, 40 * MIN, &mut ids);

    assert_eq!(s.timer_state, TimerState::AwaitingDecision);
    assert_eq!(s.current_task().unwrap().id, "A");
}

#[test]
fn emptying_the_queue_lands_in_idle_not_a_restart() {
    let mut s = MachineState::default();
    s.tasks.push(Task::new("A", "only task", 30 * MIN, 0));
    s.queue.push("A".into());
    let mut ids = SeqIds::new("b");

    let s = fire(s, Event::SwitchTo { task: "A".into() }, 0, &mut ids);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideComplete, 30 * MIN, &mut ids);

    assert_eq!(s.timer_state, TimerState::Idle);
    assert!(s.current_block_id.is_none());
    assert!(s.queue.is_empty());
}

#[test]
fn at_most_one_parked_block_per_task_ever_exists() {
    let (s, mut ids) = start_a(0);
    let mut s = s;
    for i in 1..=5 {
        s = fire(s, Event::SwitchTo { task: "C".into() }, i * MIN, &mut ids);
        s = fire(s, Event::SwitchTo { task: "A".into() }, i * MIN, &mut ids);
    }
    for t in ["A", "B", "C", "D"] {
        let parked = s.blocks.iter()
            .filter(|b| b.task_id.as_deref() == Some(t) && b.is_parked(s.current_block_id.as_ref()))
            .count();
        assert!(parked <= 1, "{t} has {parked} parked blocks");
    }
}

#[test]
fn removing_the_running_task_cancels_its_block_and_moves_on() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::RemoveTask { task: "A".into() }, 5 * MIN, &mut ids);

    assert_eq!(status_of(&s, "A"), TaskStatus::Cancelled);
    assert_eq!(blocks_for(&s, "A")[0].status, BlockStatus::Cancelled);
    assert!(!s.queue.contains(&"A".to_string()));
    assert_eq!(s.current_task().unwrap().id, "B");
}

#[test]
fn removing_a_task_cancels_its_parked_block() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::SwitchTo { task: "C".into() }, 5 * MIN, &mut ids);
    let s = fire(s, Event::RemoveTask { task: "A".into() }, 6 * MIN, &mut ids);

    assert!(s.parked_for(&"A".to_string()).is_none(), "no orphaned remainder");
    assert_eq!(blocks_for(&s, "A")[0].status, BlockStatus::Cancelled);
    assert_eq!(s.current_task().unwrap().id, "C", "C keeps running");
}

#[test]
fn a_clock_moved_backwards_never_yields_negative_work() {
    let (s, mut ids) = start_a(10 * MIN);
    let s = fire(s, Event::Skip, 5 * MIN, &mut ids); // "now" earlier than the start
    assert_eq!(blocks_for(&s, "A")[0].actual_ms, Some(0));
}

#[test]
fn expiry_emits_the_full_alert_set_once() {
    let (s, mut ids) = start_a(0);
    let (_, fx) = fire_fx(s, Event::Tick, 30 * MIN, &mut ids);

    assert_eq!(fx.iter().filter(|f| matches!(f, Effect::EnterCheckpoint { .. })).count(), 1);
    assert_eq!(fx.iter().filter(|f| matches!(f, Effect::PlayExpirySound)).count(), 1);
    assert!(fx.iter().any(|f| matches!(f, Effect::Notify { .. })));
    assert!(fx.iter().any(|f| matches!(f, Effect::StopTicking)));
    assert!(fx.iter().any(|f| matches!(f, Effect::Persist)), "every transition persists");
}

#[test]
fn ticking_before_expiry_changes_nothing() {
    let (s, mut ids) = start_a(0);
    let before = s.clone();
    let s = fire(s, Event::Tick, 29 * MIN, &mut ids);
    assert_eq!(s, before, "a tick is not a state change");
}

// ------------------------------------------------------------ queue editing

#[test]
fn adding_a_task_queues_it_without_starting_anything() {
    let (s, mut ids) = day();
    let s = fire(s, Event::AddTask {
        title: "  Write the release notes  ".into(),
        block_ms: 20 * MIN,
        priority: Priority::Low,
    }, 0, &mut ids);

    assert_eq!(s.timer_state, TimerState::Idle, "adding never starts work");
    assert_eq!(s.tasks.len(), 5);
    assert_eq!(s.tasks[4].title, "Write the release notes", "title is trimmed");
    assert_eq!(s.queue.last().unwrap(), &s.tasks[4].id, "queued at the back");
}

#[test]
fn a_blank_or_zero_length_task_is_rejected() {
    let (s, mut ids) = day();
    let s = fire(s, Event::AddTask { title: "   ".into(), block_ms: 30 * MIN, priority: Priority::Medium }, 0, &mut ids);
    let s = fire(s, Event::AddTask { title: "ok".into(), block_ms: 0, priority: Priority::Medium }, 0, &mut ids);
    assert_eq!(s.tasks.len(), 4, "neither was added");
}

#[test]
fn adding_while_running_does_not_disturb_the_current_block() {
    let (s, mut ids) = start_a(0);
    let before = s.current_block().unwrap().clone();
    let s = fire(s, Event::AddTask { title: "later".into(), block_ms: 15 * MIN, priority: Priority::Medium }, 5 * MIN, &mut ids);

    assert_eq!(s.current_block().unwrap(), &before);
    assert_eq!(s.remaining_ms(5 * MIN), 25 * MIN);
    assert_eq!(s.queue.first().unwrap(), "A", "the running task stays at the head");
}

#[test]
fn reordering_moves_a_task_before_another() {
    let (s, mut ids) = day();
    let s = fire(s, Event::Reorder { moved: "D".into(), before: "B".into() }, 0, &mut ids);
    assert_eq!(s.queue, vec!["A", "D", "B", "C"]);

    // Self-move and unknown targets are no-ops, not corruption.
    let s = fire(s, Event::Reorder { moved: "D".into(), before: "D".into() }, 0, &mut ids);
    assert_eq!(s.queue, vec!["A", "D", "B", "C"]);
    let s = fire(s, Event::Reorder { moved: "Z".into(), before: "B".into() }, 0, &mut ids);
    assert_eq!(s.queue, vec!["A", "D", "B", "C"]);
}

// ------------------------------------------- Phase 7: away time and summary
//
// Test 22 in SPEC §12 and D13. The point of these is that a gap at an
// unanswered checkpoint is *surfaced*, never guessed at and never quietly
// credited to a task as work.

#[test]
fn time_at_an_unanswered_checkpoint_is_away_and_not_worked() {
    let (s, mut ids) = start_a(0);
    // The block expires at 30m; the decision comes two hours later.
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    assert_eq!(s.timer_state, TimerState::AwaitingDecision);
    assert_eq!(s.staleness_ms(150 * MIN), Some(120 * MIN));

    let s = fire(s, Event::DecidePending, 150 * MIN, &mut ids);
    let a = blocks_for(&s, "A")[0];
    assert_eq!(a.away_ms, 120 * MIN, "the gap is banked on the block");
    assert_eq!(
        a.actual_ms,
        Some(30 * MIN),
        "the two hours away are not worked time — the block is capped at its allocation"
    );
}

#[test]
fn extending_after_a_gap_keeps_both_gaps() {
    // Extending re-arms the block, so it can reach the checkpoint more than
    // once. Assigning rather than accumulating would silently lose the first
    // wait, and Today's Away line would understate the day.
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::DecideExtend { ms: 10 * MIN }, 40 * MIN, &mut ids); // 10m away
    let s = fire(s, Event::Tick, 50 * MIN, &mut ids);
    let s = fire(s, Event::DecidePending, 55 * MIN, &mut ids); // 5m more

    assert_eq!(blocks_for(&s, "A")[0].away_ms, 15 * MIN);
}

#[test]
fn a_parked_block_accrues_no_away_time() {
    // A parked block keeps a past `end_at`, so deriving Away from timestamps
    // after the fact would count every set-down task as time spent waiting at
    // a checkpoint that was never open.
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::SwitchTo { task: "B".into() }, 10 * MIN, &mut ids);
    let s = fire(s, Event::Tick, 300 * MIN, &mut ids);

    assert_eq!(blocks_for(&s, "A")[0].away_ms, 0);
}

#[test]
fn a_rejected_event_at_a_checkpoint_banks_nothing() {
    // SwitchTo is refused at a work checkpoint. If it banked the gap anyway,
    // the wait would be counted twice once the real decision arrived.
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let s = fire(s, Event::SwitchTo { task: "B".into() }, 60 * MIN, &mut ids);
    assert_eq!(s.timer_state, TimerState::AwaitingDecision);
    assert_eq!(blocks_for(&s, "A")[0].away_ms, 0);

    let s = fire(s, Event::DecidePending, 90 * MIN, &mut ids);
    assert_eq!(blocks_for(&s, "A")[0].away_ms, 60 * MIN, "counted once, not twice");
}

mod summary {
    use super::*;
    use crate::core::summary::summarize;

    const DAY: Millis = 0;
    const AVAILABLE: Millis = 420 * MIN;

    #[test]
    fn a_break_is_rest_and_never_counts_as_worked() {
        // The invariant the whole product rests on: break time is not output,
        // and it does not consume the day's work capacity (SPEC D7).
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
        let s = fire(s, Event::DecideBreak { ms: 10 * MIN, complete: true }, 30 * MIN, &mut ids);

        let sum = summarize(&s, DAY, 35 * MIN, AVAILABLE);
        assert_eq!(sum.today.worked_ms, 30 * MIN);
        assert_eq!(sum.today.break_ms, 5 * MIN, "the running break counts only what it has spent");
        assert_eq!(sum.today.tasks_completed, 1);
        assert!(
            !sum.today.top.iter().any(|t| t.title == "Break"),
            "a break has no task and cannot appear in the top list"
        );
    }

    #[test]
    fn switching_early_is_counted_so_the_day_shows_its_own_churn() {
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::SwitchTo { task: "B".into() }, 10 * MIN, &mut ids);
        let s = fire(s, Event::SwitchTo { task: "C".into() }, 20 * MIN, &mut ids);

        assert_eq!(summarize(&s, DAY, 25 * MIN, AVAILABLE).today.switched_early, 2);
    }

    #[test]
    fn capacity_counts_a_parked_task_at_its_remainder() {
        // Otherwise the strip would promise time the task no longer has, and
        // the day would look emptier than it is (SPEC D10).
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::SwitchTo { task: "B".into() }, 10 * MIN, &mut ids);

        // Queue is now B(45, running) A(20 parked) C(30) D(45).
        let cap = summarize(&s, DAY, 10 * MIN, AVAILABLE).capacity;
        assert_eq!(cap.allocated_ms, (45 + 20 + 30 + 45) * MIN);
        assert!(!cap.over);
        assert_eq!(cap.unallocated_ms, AVAILABLE - cap.allocated_ms);
    }

    #[test]
    fn over_capacity_is_reported_but_never_blocked() {
        let (s, _ids) = day(); // 150m queued
        let cap = summarize(&s, DAY, 0, 60 * MIN).capacity;
        assert!(cap.over);
        assert_eq!(cap.unallocated_ms, -90 * MIN, "the overrun is signed, not clamped");
    }

    #[test]
    fn only_todays_blocks_are_counted() {
        // The app is long-running; without the day boundary Today would be
        // "since install".
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
        let s = fire(s, Event::DecidePending, 30 * MIN, &mut ids);

        let tomorrow = 24 * 60 * MIN;
        assert_eq!(summarize(&s, tomorrow, tomorrow, AVAILABLE).today.worked_ms, 0);
        assert_eq!(summarize(&s, DAY, 30 * MIN, AVAILABLE).today.worked_ms, 30 * MIN);
    }

    #[test]
    fn away_includes_the_checkpoint_still_open() {
        // A gap only becomes visible once it is answered unless the open one is
        // added live — which is exactly the case D13 is about.
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::Tick, 30 * MIN, &mut ids);

        assert_eq!(summarize(&s, DAY, 90 * MIN, AVAILABLE).today.away_ms, 60 * MIN);
    }

    #[test]
    fn the_top_list_ranks_by_time_and_stops_at_three() {
        let (s, mut ids) = day();
        let mut s = s;
        for (task, at) in [("A", 0), ("B", 30 * MIN), ("C", 40 * MIN), ("D", 45 * MIN)] {
            s = fire(s, Event::SwitchTo { task: task.into() }, at, &mut ids);
        }
        let top = summarize(&s, DAY, 50 * MIN, AVAILABLE).today.top;
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].ms, 30 * MIN); // A
        assert_eq!(top[1].ms, 10 * MIN); // B
        assert!(top[0].ms >= top[1].ms && top[1].ms >= top[2].ms);
    }
}
