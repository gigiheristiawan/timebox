//! Persistence and recovery. These are the acceptance tests that a pure
//! reducer cannot prove: they require the state to make a round trip through
//! SQLite and come back correct.

use super::*;
use crate::core::model::{SeqIds, Task, TaskStatus, TimerState};
use crate::core::timer_machine::{Event, MachineState};
use crate::db::{repo, Db};

const MIN: Millis = 60_000;

fn seeded() -> MachineState {
    let mut s = MachineState::default();
    for (id, title, mins) in [("A", "Fix attendance bug", 30), ("B", "Payroll", 45), ("C", "Review", 30)] {
        s.tasks.push(Task::new(id, title, mins * MIN, 0));
        s.queue.push(id.to_string());
    }
    s
}

/// Persist a state, then reopen it as a *new* App at `later` — the same path a
/// quit and relaunch takes.
fn quit_and_relaunch(db: Db, state: &MachineState, saved_at: Millis, later: Millis) -> (Db, MachineState) {
    db.with_mut(|c| repo::save(c, state, saved_at)).unwrap();
    let reloaded = db.with(repo::load).unwrap();
    let mut ids = SeqIds::new("r");
    let (resolved, _) = crate::core::timer_machine::reduce(reloaded, Event::Tick, later, &mut ids);
    (db, resolved)
}

#[test]
fn round_trip_preserves_the_state_exactly() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    let before = app.snapshot();
    app.db.with_mut(|c| repo::save(c, &before, 0)).unwrap();

    let after = app.db.with(repo::load).unwrap();
    assert_eq!(after.tasks, before.tasks);
    assert_eq!(after.queue, before.queue, "queue order survives (SPEC D3)");
    assert_eq!(after.timer_state, before.timer_state);
}

#[test]
fn t7_restart_while_awaiting_a_decision_still_awaits() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();
    app.dispatch(Event::Tick, 30 * MIN).unwrap();
    assert_eq!(app.snapshot().timer_state, TimerState::AwaitingDecision);

    // Quit without deciding, relaunch two hours later.
    let state = app.snapshot();
    let db = Db::in_memory().unwrap();
    let (_, after) = quit_and_relaunch(db, &state, 30 * MIN, 150 * MIN);

    assert_eq!(after.timer_state, TimerState::AwaitingDecision, "the decision is still owed");
    assert_eq!(after.current_task().unwrap().id, "A");
    assert_ne!(after.tasks[0].status, TaskStatus::Done);
}

#[test]
fn t6_sleeping_past_expiry_resolves_to_a_checkpoint() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();

    // Mac sleeps at 10 minutes and wakes at 40 — past the 30-minute boundary.
    let state = app.snapshot();
    let db = Db::in_memory().unwrap();
    let (_, after) = quit_and_relaunch(db, &state, 10 * MIN, 40 * MIN);

    assert_eq!(after.timer_state, TimerState::AwaitingDecision, "must not show a running timer");
    assert_eq!(after.remaining_ms(40 * MIN), 0, "and must not have reset");
    assert_eq!(after.staleness_ms(40 * MIN), Some(10 * MIN), "expired 10 minutes ago");
}

#[test]
fn restart_before_expiry_resumes_running_with_the_correct_remainder() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();

    let state = app.snapshot();
    let db = Db::in_memory().unwrap();
    let (_, after) = quit_and_relaunch(db, &state, 5 * MIN, 12 * MIN);

    assert_eq!(after.timer_state, TimerState::Running);
    assert_eq!(after.remaining_ms(12 * MIN), 18 * MIN, "the clock ran while away");
}

#[test]
fn a_paused_block_is_restored_with_an_identical_remainder() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();
    app.dispatch(Event::Pause, 8 * MIN).unwrap();

    let state = app.snapshot();
    let db = Db::in_memory().unwrap();
    // Relaunched a day later: paused time is never spent.
    let (_, after) = quit_and_relaunch(db, &state, 8 * MIN, 1440 * MIN);

    assert_eq!(after.timer_state, TimerState::Paused);
    assert_eq!(after.remaining_ms(1440 * MIN), 22 * MIN);
}

#[test]
fn t35_parked_blocks_restore_with_exact_remainders() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();
    app.dispatch(Event::SwitchTo { task: "C".into() }, 5 * MIN, ).unwrap();

    let state = app.snapshot();
    let db = Db::in_memory().unwrap();
    let (_, after) = quit_and_relaunch(db, &state, 5 * MIN, 9 * MIN);

    // Only the CURRENT block re-evaluates expiry; the parked one just waits.
    let parked = after.parked_for(&"A".to_string()).expect("A's remainder survives a restart");
    assert_eq!(parked.remaining_when_paused_ms, Some(25 * MIN));
    assert_eq!(parked.interruptions, 1);
    assert_eq!(after.current_task().unwrap().id, "C");
    assert_eq!(after.remaining_ms(9 * MIN), 26 * MIN, "C's own block kept running");
}

#[test]
fn t22_a_break_survives_a_restart() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();
    app.dispatch(Event::Tick, 30 * MIN).unwrap();
    app.dispatch(Event::DecideBreak { ms: 5 * MIN, complete: false }, 30 * MIN).unwrap();

    let state = app.snapshot();
    let db = Db::in_memory().unwrap();
    // Slept through the break and then some.
    let (_, after) = quit_and_relaunch(db, &state, 31 * MIN, 45 * MIN);

    assert!(after.on_break(), "still the break's checkpoint, not a work one");
    assert_eq!(after.timer_state, TimerState::AwaitingDecision);
}

#[test]
fn hydrate_resolves_expiry_before_returning() {
    // The state is written as RUNNING and already past its end_at; hydrate must
    // never hand back a running timer for a block that is over.
    let db = Db::in_memory().unwrap();
    let mut s = seeded();
    let mut ids = SeqIds::new("h");
    let (s2, _) = crate::core::timer_machine::reduce(s.clone(), Event::SwitchTo { task: "A".into() }, 0, &mut ids);
    s = s2;
    db.with_mut(|c| repo::save(c, &s, 0)).unwrap();

    let app = App::hydrate(db, 90 * MIN).unwrap();
    assert_eq!(app.snapshot().timer_state, TimerState::AwaitingDecision);
}

#[test]
fn every_dispatch_is_durable_immediately() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();

    // Read straight from SQLite, bypassing the in-memory copy entirely.
    let persisted = app.db.with(repo::load).unwrap();
    assert_eq!(persisted.timer_state, TimerState::Running);
    assert_eq!(persisted.current_block().unwrap().task_id.as_deref(), Some("A"));
    assert_eq!(persisted.queue, vec!["A", "B", "C"]);
}

#[test]
fn the_parked_block_uniqueness_index_holds_through_persistence() {
    let db = Db::in_memory().unwrap();
    let app = App::hydrate(db, 0).unwrap();
    { *app.machine.lock() = seeded(); }
    app.dispatch(Event::SwitchTo { task: "A".into() }, 0).unwrap();
    for i in 1..=6 {
        app.dispatch(Event::SwitchTo { task: "C".into() }, i * MIN).unwrap();
        app.dispatch(Event::SwitchTo { task: "A".into() }, i * MIN).unwrap();
    }
    let n: i64 = app
        .db
        .with(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM time_blocks WHERE status='PAUSED' AND task_id='A'",
                [],
                |r| r.get(0),
            )
        })
        .unwrap();
    assert!(n <= 1, "the schema's partial unique index would have rejected more");
}
