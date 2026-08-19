//! Application state: the domain core plus the things that make it durable.
//!
//! The core decides; this module persists the decision and drives the clock.
//! Nothing here re-implements a rule.

use crate::core::model::*;
use crate::core::timer_machine::{reduce, Effect, Event, MachineState};
use crate::db::{repo, Db};
use crate::error::AppResult;
use parking_lot::{Condvar, Mutex};
use std::sync::Arc;
use std::time::Duration;

pub fn now_ms() -> Millis {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as Millis)
        .unwrap_or(0)
}

struct UuidIds;

impl IdSource for UuidIds {
    fn next_id(&mut self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

pub struct App {
    pub db: Db,
    machine: Mutex<MachineState>,
    ids: Mutex<UuidIds>,
    ticker: Ticker,
}

impl App {
    /// Load persisted state and **resolve expiry before anyone can see it**.
    ///
    /// A block whose `end_at` passed while the app was not running must surface
    /// as a checkpoint, never as a running timer or a reset one. Feeding a
    /// single `Tick` at the current instant is what makes acceptance tests 6
    /// (Mac slept) and 7 (quit while awaiting a decision) hold, without either
    /// case needing its own code path.
    pub fn hydrate(db: Db, now: Millis) -> AppResult<Arc<Self>> {
        let loaded = db.with(repo::load)?;
        let mut ids = UuidIds;
        let (state, _fx) = reduce(loaded, Event::Tick, now, &mut ids);
        db.with_mut(|c| repo::save(c, &state, now))?;

        let running = state.timer_state == TimerState::Running;
        let app = Arc::new(Self {
            db,
            machine: Mutex::new(state),
            ids: Mutex::new(UuidIds),
            ticker: Ticker::new(),
        });
        app.ticker.set_running(running);
        Ok(app)
    }

    pub fn snapshot(&self) -> MachineState {
        self.machine.lock().clone()
    }

    /// Reduce, persist, and apply the ticking effects — in that order, so a
    /// crash can never leave the UI ahead of the database (SPEC §4.5).
    pub fn dispatch(&self, event: Event, now: Millis) -> AppResult<Vec<Effect>> {
        let mut guard = self.machine.lock();
        let (next, fx) = {
            let mut ids = self.ids.lock();
            reduce(guard.clone(), event, now, &mut *ids)
        };
        self.db.with_mut(|c| repo::save(c, &next, now))?;
        *guard = next;
        drop(guard);

        for e in &fx {
            match e {
                Effect::StartTicking => self.ticker.set_running(true),
                Effect::StopTicking => self.ticker.set_running(false),
                _ => {}
            }
        }
        Ok(fx)
    }

    /// Start the 1 Hz clock. It blocks entirely — zero wakeups — whenever the
    /// timer is not RUNNING, which is what keeps idle CPU at nil (task 3.7).
    ///
    /// Because the thread sleeps against wall time, a Mac that suspends and
    /// resumes simply produces a late tick, and the timestamp comparison in the
    /// reducer resolves the expiry correctly on that tick.
    pub fn start_ticking(self: &Arc<Self>, mut on_effects: impl FnMut(&[Effect]) + Send + 'static) {
        let app = Arc::clone(self);
        let ticker = self.ticker.clone_handle();
        std::thread::spawn(move || loop {
            if !ticker.wait_for_tick() {
                return;
            }
            match app.dispatch(Event::Tick, now_ms()) {
                Ok(fx) => on_effects(&fx),
                Err(e) => eprintln!("tick failed: {e}"),
            }
        });
    }
}

// -------------------------------------------------------------------- ticker

struct TickerInner {
    running: bool,
    alive: bool,
}

#[derive(Clone)]
pub struct TickerHandle(Arc<(Mutex<TickerInner>, Condvar)>);

impl TickerHandle {
    /// Blocks until it is time to tick. Returns false when the app is shutting
    /// down. While the timer is not running this parks indefinitely and costs
    /// nothing.
    fn wait_for_tick(&self) -> bool {
        let (lock, cv) = &*self.0;
        let mut g = lock.lock();
        while !g.running && g.alive {
            cv.wait(&mut g);
        }
        if !g.alive {
            return false;
        }
        cv.wait_for(&mut g, Duration::from_secs(1));
        g.alive
    }
}

struct Ticker(Arc<(Mutex<TickerInner>, Condvar)>);

impl Ticker {
    fn new() -> Self {
        Self(Arc::new((
            Mutex::new(TickerInner { running: false, alive: true }),
            Condvar::new(),
        )))
    }

    fn clone_handle(&self) -> TickerHandle {
        TickerHandle(Arc::clone(&self.0))
    }

    fn set_running(&self, on: bool) {
        let (lock, cv) = &*self.0;
        let mut g = lock.lock();
        if g.running != on {
            g.running = on;
            cv.notify_all();
        }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        let (lock, cv) = &*self.0;
        lock.lock().alive = false;
        cv.notify_all();
    }
}

#[cfg(test)]
mod tests;
