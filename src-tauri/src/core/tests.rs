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
fn editing_a_task_renames_and_reprioritises_it_without_touching_its_block() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::EditTask {
        task: "A".into(),
        title: "  Draft the release notes  ".into(),
        priority: Priority::High,
    }, 5 * MIN, &mut ids);

    let t = s.tasks.iter().find(|t| t.id == "A").unwrap();
    assert_eq!(t.title, "Draft the release notes", "title is trimmed, as on add");
    assert_eq!(t.priority, Priority::High);
    // An edit is not an allocation: renaming the running task mid-block must
    // not hand it a fresh 30 minutes, which is the D10 anti-gaming rule.
    assert_eq!(s.remaining_ms(5 * MIN), 25 * MIN);
    assert_eq!(s.timer_state, TimerState::Running);
}

#[test]
fn an_edit_cannot_blank_a_title() {
    let (s, mut ids) = day();
    let s = fire(s, Event::EditTask { task: "A".into(), title: "   ".into(), priority: Priority::High }, 0, &mut ids);
    let t = s.tasks.iter().find(|t| t.id == "A").unwrap();
    assert_eq!(t.title, "Fix attendance bug", "the blank edit was refused whole");
    assert_eq!(t.priority, Priority::Medium, "including the priority that rode with it");

    // An unknown task is a no-op, not a panic.
    let s = fire(s, Event::EditTask { task: "Z".into(), title: "x".into(), priority: Priority::Low }, 0, &mut ids);
    assert_eq!(s.tasks.len(), 4);
}

#[test]
fn adding_time_grows_the_running_block_without_resetting_what_was_burnt() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::AddTime { task: "A".into(), ms: 10 * MIN }, 20 * MIN, &mut ids);

    // 10 of the 30 were left; the grant adds to that, it does not restart it.
    assert_eq!(s.remaining_ms(20 * MIN), 20 * MIN);
    assert_eq!(s.current_block().unwrap().extension_ms, 10 * MIN);
    // The task's own allocation grows too, so its *next* block is 40m as well.
    let t = s.tasks.iter().find(|t| t.id == "A").unwrap();
    assert_eq!(t.block_duration_ms, 40 * MIN);
}

#[test]
fn adding_time_to_a_parked_task_grows_the_remainder_it_will_resume_with() {
    let (s, mut ids) = start_a(0);
    // Switch away at 25m: A parks holding 5m.
    let s = fire(s, Event::SwitchTo { task: "B".into() }, 25 * MIN, &mut ids);
    assert_eq!(s.parked_for(&"A".to_string()).unwrap().remaining_when_paused_ms, Some(5 * MIN));

    let s = fire(s, Event::AddTime { task: "A".into(), ms: 15 * MIN }, 26 * MIN, &mut ids);
    assert_eq!(
        s.parked_for(&"A".to_string()).unwrap().remaining_when_paused_ms,
        Some(20 * MIN),
        "the grant lands on the remainder the task will resume with",
    );
    let s = fire(s, Event::SwitchTo { task: "A".into() }, 30 * MIN, &mut ids);
    assert_eq!(s.remaining_ms(30 * MIN), 20 * MIN, "and is what the resumed block runs on");
}

#[test]
fn adding_time_is_refused_at_a_checkpoint_and_never_shortens_anything() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    assert_eq!(s.timer_state, TimerState::AwaitingDecision);

    // Extend is the checkpoint's own grant and it costs a decision. Handing out
    // time from the editor instead would be a way around answering.
    let s = fire(s, Event::AddTime { task: "A".into(), ms: 10 * MIN }, 31 * MIN, &mut ids);
    assert_eq!(s.timer_state, TimerState::AwaitingDecision, "still owed a decision");
    assert_eq!(s.current_block().unwrap().extension_ms, 0, "the block is untouched");

    // Nothing subtracts. A zero or negative grant is a no-op, not a trim.
    let (s2, mut ids2) = start_a(0);
    let before = s2.clone();
    let s2 = fire(s2, Event::AddTime { task: "A".into(), ms: -10 * MIN }, 5 * MIN, &mut ids2);
    assert_eq!(s2, before, "a negative grant changes nothing");
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

    // Dragging *downwards* has to move the row too. Deriving the insert point
    // after lifting the row out would land it back where it started — the drag
    // would look broken rather than wrong.
    let s = fire(s, Event::Reorder { moved: "A".into(), before: "D".into() }, 0, &mut ids);
    assert_eq!(s.queue, vec!["D", "A", "B", "C"]);
    let s = fire(s, Event::Reorder { moved: "D".into(), before: "C".into() }, 0, &mut ids);
    assert_eq!(s.queue, vec!["A", "B", "C", "D"], "a drag onto the last row lands last");
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

// ------------------------------------------- Tests 42–47: deliberate breaks
//
// IDLE_TIME D22. Without them every rest taken between checkpoints is recorded
// as idle, which makes the measure punish the honest user.

#[test]
fn t42_start_break_parks_the_block_and_keeps_the_task_at_the_head() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::StartBreak { ms: 45 * MIN }, 10 * MIN, &mut ids);

    assert!(s.on_break(), "a break is running");
    assert_eq!(s.timer_state, TimerState::Running);
    let parked = s.parked_for(&"A".to_string()).expect("A holds its remainder");
    assert_eq!(parked.remaining_when_paused_ms, Some(20 * MIN));
    // Unlike a switch, which rotates to the tail: a break is a return to the
    // same work, so the break checkpoint offers A again and not B.
    assert_eq!(s.queue.first().unwrap(), "A");

    let s = fire(s, Event::Tick, 55 * MIN, &mut ids);
    let s = fire(s, Event::EndBreak, 55 * MIN, &mut ids);
    assert_eq!(s.current_task().unwrap().id, "A", "the break returned to the same task");
    assert_eq!(s.remaining_ms(55 * MIN), 20 * MIN, "and to its remainder, not a fresh block");
}

#[test]
fn t43_a_deliberate_break_is_not_an_interruption() {
    // D11 counts churn *between tasks*. Tinting Today's "Switched early"
    // warning because someone took lunch would contradict D9's claim about rest.
    let (s, mut ids) = start_a(0);
    let broke = fire(s.clone(), Event::StartBreak { ms: 10 * MIN }, 10 * MIN, &mut ids);
    let switched = fire(s, Event::SwitchTo { task: "B".into() }, 10 * MIN, &mut ids);

    assert_eq!(blocks_for(&broke, "A")[0].interruptions, 0);
    assert_eq!(blocks_for(&switched, "A")[0].interruptions, 1, "a switch still counts");
}

#[test]
fn t44_start_break_is_a_no_op_at_a_work_checkpoint() {
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
    let after = fire(s.clone(), Event::StartBreak { ms: 10 * MIN }, 31 * MIN, &mut ids);

    assert_eq!(after.timer_state, TimerState::AwaitingDecision);
    assert!(!after.on_break(), "the checkpoint has no side doors");
    assert_eq!(after.blocks.len(), s.blocks.len());
}

#[test]
fn t45_start_break_during_a_break_is_a_no_op() {
    // ExtendBreak is the operation during one; a second break block would
    // orphan the first and double-count the rest.
    let (s, mut ids) = start_a(0);
    let s = fire(s, Event::StartBreak { ms: 10 * MIN }, 5 * MIN, &mut ids);
    let after = fire(s.clone(), Event::StartBreak { ms: 30 * MIN }, 6 * MIN, &mut ids);

    assert_eq!(after.blocks.len(), s.blocks.len());
    assert_eq!(after.remaining_ms(6 * MIN), 9 * MIN, "still the original break");
}

#[test]
fn a_deliberate_break_can_start_from_idle_and_from_paused() {
    let (s, mut ids) = day();
    let idle = fire(s.clone(), Event::StartBreak { ms: 10 * MIN }, 0, &mut ids);
    assert!(idle.on_break());

    let s = fire(s, Event::SwitchTo { task: "A".into() }, 0, &mut ids);
    let s = fire(s, Event::Pause, 5 * MIN, &mut ids);
    let s = fire(s, Event::StartBreak { ms: 10 * MIN }, 6 * MIN, &mut ids);
    assert!(s.on_break());
    assert_eq!(
        s.parked_for(&"A".to_string()).unwrap().remaining_when_paused_ms,
        Some(25 * MIN),
        "the pause held the remainder; the break must not spend it"
    );
}

mod idle {
    //! Tests 23–33 (IDLE_TIME §7). Idle is *inferred*: window time that no
    //! running block covered. Every test here fixes a claim the daily report
    //! would otherwise make falsely.

    use super::*;
    use crate::core::summary::{summarize, Summary};

    const HOUR: Millis = 60 * MIN;
    const DAY: Millis = 0;
    const AVAILABLE: Millis = 420 * MIN;
    /// 09:00–18:00 on the day beginning at `DAY`.
    const WINDOW: (Millis, Millis) = (9 * HOUR, 18 * HOUR);

    fn at(h: i64, m: i64) -> Millis {
        h * HOUR + m * MIN
    }

    fn sum_at(s: &MachineState, now: Millis, window: Option<(Millis, Millis)>) -> Summary {
        summarize(s, DAY, now, AVAILABLE, window)
    }

    fn today(s: &MachineState, now: Millis) -> crate::core::summary::Today {
        sum_at(s, now, Some(WINDOW)).today
    }

    /// The three sub-buckets must always sum to the total. Asserted in every
    /// test rather than only in the property test, because a bucket that leaks
    /// is invisible in any single number.
    fn assert_sums(t: &crate::core::summary::Today) {
        assert_eq!(
            t.idle_ms,
            t.idle_awaiting_ms + t.idle_paused_ms + t.idle_untracked_ms,
            "the causes must partition idle exactly (test 32)"
        );
    }

    #[test]
    fn t23_a_late_start_is_untracked_idle() {
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(10, 30), &mut ids);

        let t = today(&s, at(10, 45));
        assert_eq!(t.idle_untracked_ms, 90 * MIN, "09:00 to 10:30 was never claimed");
        assert_eq!(t.idle_ms, 90 * MIN);
        assert_sums(&t);
    }

    #[test]
    fn t24_a_pause_is_idle_and_is_not_worked() {
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(9, 0), &mut ids);
        let s = fire(s, Event::Pause, at(11, 0), &mut ids);
        let s = fire(s, Event::Resume, at(11, 20), &mut ids);

        let t = today(&s, at(11, 30));
        assert_eq!(t.idle_paused_ms, 20 * MIN);
        assert_eq!(t.idle_untracked_ms, 0, "nothing before 09:00 counts");
        // D4: the pause was never worked either. The 30-minute block ran
        // 09:00–11:00 in wall time but its allocation caps what it can report.
        assert_eq!(t.worked_ms, 30 * MIN);
        assert_sums(&t);
    }

    #[test]
    fn t25_time_at_a_checkpoint_is_idle_and_matches_away() {
        // The same interval seen two ways — per block (`away_ms`, D13) and per
        // interval (the span). They are two views of one fact and must agree;
        // if they ever drift, one of the two writers has been changed alone.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(13, 30), &mut ids);
        let s = fire(s, Event::Tick, at(14, 0), &mut ids);
        let s = fire(s, Event::DecideComplete, at(14, 25), &mut ids);

        let t = today(&s, at(14, 30));
        assert_eq!(t.idle_awaiting_ms, 25 * MIN);
        assert_eq!(t.away_ms, 25 * MIN, "away_ms and the AWAITING span record one gap");
        assert_sums(&t);
    }

    #[test]
    fn t26_quitting_parks_the_block_and_the_gap_is_idle_not_work() {
        // The measure this whole feature exists for. Quitting is a pause
        // (D16): an interval cannot be idle and worked at once.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(14, 50), &mut ids);
        // 15:00 — the quit path dispatches the existing Pause.
        let s = fire(s, Event::Pause, at(15, 0), &mut ids);
        let worked_at_quit = today(&s, at(15, 0)).worked_ms;

        // Reopened an hour later. Nothing happened in between, and the open
        // span kept accruing while the app was not running.
        let t = today(&s, at(16, 0));
        assert_eq!(t.idle_paused_ms, 60 * MIN);
        assert_eq!(t.worked_ms, worked_at_quit, "the clock stopped when the app did");
        let held = s.current_block().unwrap();
        assert_eq!(held.status, BlockStatus::Paused);
        assert_eq!(held.remaining_when_paused_ms, Some(20 * MIN), "held, never re-granted");
        assert_sums(&t);
    }

    #[test]
    fn t27_idle_is_sealed_at_work_end_not_at_the_next_launch() {
        // Without the seal, a block left parked over a long weekend would
        // report days of idle on the next hydrate.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(17, 0), &mut ids);
        let s = fire(s, Event::Pause, at(17, 30), &mut ids);

        let next_morning = at(33, 0); // 09:00 tomorrow, still the same open span
        let t = today(&s, next_morning);
        assert_eq!(t.idle_paused_ms, 30 * MIN, "17:30 to 18:00 only");
        assert!(t.idle_ms <= 9 * HOUR, "a day's idle can never exceed its window");
        assert_sums(&t);
    }

    #[test]
    fn t28_work_after_work_end_is_recorded_and_adds_no_idle() {
        // D17: the day's idle is sealed at work_end; the day's work is not.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(9, 0), &mut ids);
        let s = fire(s, Event::Tick, at(9, 30), &mut ids);
        let s = fire(s, Event::DecideComplete, at(9, 30), &mut ids); // B starts
        let s = fire(s, Event::Pause, at(9, 45), &mut ids);
        let sealed = today(&s, at(18, 0));

        let s = fire(s, Event::Resume, at(20, 0), &mut ids);
        let t = today(&s, at(20, 30));
        assert!(t.outside_hours_ms > 0, "evening work is a signal, not an error");
        assert_eq!(t.outside_hours_ms, 30 * MIN);
        assert_eq!(t.idle_ms, sealed.idle_ms, "and it changes no idle figure");
        assert_sums(&t);
    }

    #[test]
    fn t29_a_non_working_day_records_work_and_no_idle() {
        // D18: a weekend is a day whose window is empty, so nothing about it
        // can be idle — no claim of presence was ever made.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(11, 0), &mut ids);

        let t = sum_at(&s, at(11, 20), None).today;
        assert_eq!(t.idle_ms, 0);
        assert_eq!(t.worked_ms, 20 * MIN);
        assert_eq!(t.outside_hours_ms, 20 * MIN);
        assert_sums(&t);
    }

    #[test]
    fn t30_a_day_with_no_blocks_reports_no_idle() {
        // D19, the holiday and sick-day answer. The accepted cost is that a
        // genuinely wasted working day is indistinguishable from a day off —
        // the app must not guess which it was.
        let (s, _ids) = day(); // tasks queued, nothing ever started
        let t = today(&s, at(17, 0));
        assert_eq!(t.idle_ms, 0);
        assert_eq!(t.idle_untracked_ms, 0);
        assert_sums(&t);
    }

    #[test]
    fn t31_an_open_span_survives_an_unclean_exit_and_keeps_accruing() {
        // D15: the app need not be running for idle to accrue. A span left open
        // by a crash describes a machine state that is still true — paused is
        // still paused — so it is not closed at the last write, which would
        // erase a real gap. See the D20 note in IDLE_TIME.md.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(15, 40), &mut ids);
        let s = fire(s, Event::Pause, at(16, 10), &mut ids); // the last state write

        let reloaded = s.clone(); // whatever comes back from SQLite is identical
        assert!(reloaded.open_idle.is_some(), "the span is still open");
        let t = today(&reloaded, at(17, 10));
        assert_eq!(t.idle_paused_ms, 60 * MIN);
        assert_sums(&t);
    }

    #[test]
    fn t32_the_causes_partition_idle_for_any_sequence_of_events() {
        // A property test over a long, deliberately messy day: every kind of
        // departure from RUNNING, in an order no single test would produce.
        let (mut s, mut ids) = day();
        let script: &[(Millis, Event)] = &[
            (at(9, 30), Event::SwitchTo { task: "A".into() }),
            (at(9, 40), Event::Pause),
            (at(10, 0), Event::Resume),
            (at(10, 20), Event::SwitchTo { task: "B".into() }),
            (at(11, 5), Event::Tick),
            (at(11, 30), Event::DecidePending),
            (at(11, 45), Event::StartBreak { ms: 15 * MIN }),
            (at(12, 0), Event::Tick),
            (at(12, 20), Event::EndBreak),
            (at(12, 50), Event::Skip),
            (at(13, 10), Event::CompleteCurrentTask),
            (at(13, 40), Event::Pause),
            (at(14, 0), Event::Resume),
            (at(15, 0), Event::Tick),
            (at(15, 30), Event::DecideExtend { ms: 10 * MIN }),
            (at(15, 40), Event::Tick),
            (at(16, 0), Event::DecideComplete),
        ];
        for (now, e) in script {
            s = fire(s, e.clone(), *now, &mut ids);
            for probe in [*now, now + 7 * MIN] {
                let t = today(&s, probe);
                assert_sums(&t);
                assert!(t.idle_ms >= 0 && t.idle_ms <= 9 * HOUR, "idle stays inside the window");
            }
        }
    }

    #[test]
    fn t33_idle_is_a_set_difference_and_not_a_subtraction() {
        // A block spanning work_end. `window - worked - break` would subtract
        // the whole 30 minutes from a window that only contains ten of them,
        // and can go negative — which a duration cannot.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(9, 0), &mut ids);
        let s = fire(s, Event::Tick, at(9, 30), &mut ids);
        let s = fire(s, Event::DecidePending, at(9, 30), &mut ids); // B, 45m
        let s = fire(s, Event::Pause, at(9, 31), &mut ids);
        let s = fire(s, Event::Resume, at(17, 50), &mut ids);

        let t = today(&s, at(18, 20));
        // 09:31–17:50 is genuinely idle; 17:50–18:00 is covered and 18:00
        // onwards is outside the window entirely.
        assert_eq!(t.idle_paused_ms, at(17, 50) - at(9, 31));
        assert_eq!(t.idle_ms, at(17, 50) - at(9, 31));
        assert!(t.idle_ms > 0, "a set difference is never negative");
        assert_sums(&t);
    }

    #[test]
    fn t46_a_deliberate_break_is_break_and_not_idle() {
        // The point of D22. Without it, lunch is indistinguishable from drift.
        let (s, mut ids) = day();
        let s = fire(s, Event::SwitchTo { task: "A".into() }, at(11, 0), &mut ids);
        let s = fire(s, Event::StartBreak { ms: 45 * MIN }, at(12, 30), &mut ids);

        let t = today(&s, at(13, 15));
        assert_eq!(t.break_ms, 45 * MIN);
        assert_eq!(t.idle_ms, 2 * HOUR, "09:00–11:00 only; the break is not idle");
        assert_sums(&t);
    }

    #[test]
    fn t47_a_deliberate_break_does_not_consume_capacity() {
        // D9 unchanged: rest is not spent from the day's working time.
        let (s, mut ids) = day();
        let before = sum_at(&s, at(11, 0), Some(WINDOW)).capacity.allocated_ms;
        let s = fire(s, Event::StartBreak { ms: 45 * MIN }, at(11, 0), &mut ids);

        let cap = sum_at(&s, at(11, 30), Some(WINDOW)).capacity;
        assert_eq!(cap.allocated_ms, before, "a break is not queued work");
        assert_eq!(today(&s, at(11, 30)).worked_ms, 0, "and it is not output");
    }
}

mod summary {
    use super::*;
    use crate::core::summary::summarize;

    const DAY: Millis = 0;
    const AVAILABLE: Millis = 420 * MIN;
    const HOUR: Millis = 60 * MIN;

    /// A working day with an ordinary 09:00-18:00 window. These tests predate
    /// the window and are about the other numbers; `idle` has its own module.
    fn sum(s: &MachineState, day: Millis, now: Millis, available: Millis) -> crate::core::summary::Summary {
        summarize(s, day, now, available, Some((day + 9 * HOUR, day + 18 * HOUR)))
    }

    #[test]
    fn a_break_is_rest_and_never_counts_as_worked() {
        // The invariant the whole product rests on: break time is not output,
        // and it does not consume the day's work capacity (SPEC D7).
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::Tick, 30 * MIN, &mut ids);
        let s = fire(s, Event::DecideBreak { ms: 10 * MIN, complete: true }, 30 * MIN, &mut ids);

        let sum = sum(&s, DAY, 35 * MIN, AVAILABLE);
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

        assert_eq!(sum(&s, DAY, 25 * MIN, AVAILABLE).today.switched_early, 2);
    }

    #[test]
    fn capacity_counts_a_parked_task_at_its_remainder() {
        // Otherwise the strip would promise time the task no longer has, and
        // the day would look emptier than it is (SPEC D10).
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::SwitchTo { task: "B".into() }, 10 * MIN, &mut ids);

        // Queue is now B(45, running) A(20 parked) C(30) D(45).
        let cap = sum(&s, DAY, 10 * MIN, AVAILABLE).capacity;
        assert_eq!(cap.allocated_ms, (45 + 20 + 30 + 45) * MIN);
        assert!(!cap.over);
        assert_eq!(cap.unallocated_ms, AVAILABLE - cap.allocated_ms);
    }

    #[test]
    fn over_capacity_is_reported_but_never_blocked() {
        let (s, _ids) = day(); // 150m queued
        let cap = sum(&s, DAY, 0, 60 * MIN).capacity;
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
        assert_eq!(sum(&s, tomorrow, tomorrow, AVAILABLE).today.worked_ms, 0);
        assert_eq!(sum(&s, DAY, 30 * MIN, AVAILABLE).today.worked_ms, 30 * MIN);
    }

    #[test]
    fn away_includes_the_checkpoint_still_open() {
        // A gap only becomes visible once it is answered unless the open one is
        // added live — which is exactly the case D13 is about.
        let (s, mut ids) = start_a(0);
        let s = fire(s, Event::Tick, 30 * MIN, &mut ids);

        assert_eq!(sum(&s, DAY, 90 * MIN, AVAILABLE).today.away_ms, 60 * MIN);
    }

    #[test]
    fn the_top_list_ranks_by_time_and_stops_at_three() {
        let (s, mut ids) = day();
        let mut s = s;
        for (task, at) in [("A", 0), ("B", 30 * MIN), ("C", 40 * MIN), ("D", 45 * MIN)] {
            s = fire(s, Event::SwitchTo { task: task.into() }, at, &mut ids);
        }
        let top = sum(&s, DAY, 50 * MIN, AVAILABLE).today.top;
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].ms, 30 * MIN); // A
        assert_eq!(top[1].ms, 10 * MIN); // B
        assert!(top[0].ms >= top[1].ms && top[1].ms >= top[2].ms);
    }
}
