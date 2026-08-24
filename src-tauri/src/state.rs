//! Application state: the domain core plus the things that make it durable.
//!
//! The core decides; this module persists the decision and drives the clock.
//! Nothing here re-implements a rule.

use crate::core::model::*;
use crate::core::timer_machine::{reduce, Effect, Event, MachineState};
use crate::db::settings::Settings;
use crate::db::{repo, settings, Db};
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

/// Local midnight preceding `now`. The boundary for everything Today counts.
///
/// A timezone is a shell concern — the domain core takes the boundary as an
/// argument so its arithmetic stays pure and testable (see `core::summary`).
pub fn day_start_ms(now: Millis) -> Millis {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(now)
        .single()
        .and_then(|dt| dt.date_naive().and_hms_opt(0, 0, 0))
        .and_then(|midnight| Local.from_local_datetime(&midnight).earliest())
        .map(|dt| dt.timestamp_millis())
        // A DST spring-forward can leave local midnight nonexistent. Falling
        // back to a UTC-day boundary keeps Today counting rather than empty.
        .unwrap_or(now - now.rem_euclid(86_400_000))
}

/// The working window on the day beginning at `day_start`, as a pair of
/// absolute instants — or `None` when that weekday is not a working day, which
/// is the whole of IDLE_TIME D18: a weekend is a day whose window is empty.
///
/// Resolved here rather than in the core for the same reason `day_start_ms` is:
/// a weekday and a local wall-clock time need a timezone, which is a shell
/// concern. `core::summary` takes the answer as an argument.
pub fn window_for(day_start: Millis, s: &Settings) -> Option<(Millis, Millis)> {
    use chrono::{Datelike, Duration, Local, TimeZone};

    let date = Local.timestamp_millis_opt(day_start).single()?.date_naive();
    if s.working_weekdays & (1 << date.weekday().num_days_from_monday()) == 0 {
        return None;
    }
    // Built as a local wall-clock time rather than `day_start + offset`, so a
    // DST day's window still starts at 09:00 by the clock on the wall.
    let at = |ms: Millis| -> Millis {
        date.and_hms_opt(0, 0, 0)
            .and_then(|midnight| Local.from_local_datetime(&(midnight + Duration::milliseconds(ms))).earliest())
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(day_start + ms)
    };
    let (start, end) = (at(s.work_start_ms), at(s.work_end_ms));
    (start < end).then_some((start, end))
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
    /// Cached so the tick loop and every checkpoint effect can read settings
    /// without touching the database once a second.
    settings: Mutex<Settings>,
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
        let settings = db.with(settings::load)?;
        let mut ids = UuidIds;
        let (state, _fx) = reduce(loaded, Event::Tick, now, &mut ids);
        db.with_mut(|c| repo::save(c, &state, now))?;

        let running = state.timer_state == TimerState::Running;
        let app = Arc::new(Self {
            db,
            machine: Mutex::new(state),
            settings: Mutex::new(settings),
            ids: Mutex::new(UuidIds),
            ticker: Ticker::new(),
        });
        app.ticker.set_running(running);
        Ok(app)
    }

    pub fn snapshot(&self) -> MachineState {
        self.machine.lock().clone()
    }

    pub fn settings(&self) -> Settings {
        *self.settings.lock()
    }

    /// Persist first, then cache — the same ordering as `dispatch`, so a failed
    /// write can never leave the app running on settings the database rejected.
    /// Returns the stored values, which may be clamped.
    pub fn set_settings(&self, next: &Settings) -> AppResult<Settings> {
        let stored = self.db.with(|c| settings::save(c, next))?;
        *self.settings.lock() = stored;
        Ok(stored)
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
