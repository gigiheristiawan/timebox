//! Settings (SPEC §4.4). One row, read whole and written whole — the same
//! reasoning as `repo::save`: there is no diff to get wrong.
//!
//! Nothing here decides anything about the timer. Settings choose defaults and
//! presentation; they never change whether a checkpoint appears.

use crate::core::model::Millis;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::System => "SYSTEM",
            Theme::Light => "LIGHT",
            Theme::Dark => "DARK",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "LIGHT" => Theme::Light,
            "DARK" => Theme::Dark,
            _ => Theme::System,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub launch_at_login: bool,
    pub theme: Theme,
    pub default_block_duration_ms: Millis,
    pub default_break_duration_ms: Millis,
    pub expiration_sound: bool,
    pub system_notification: bool,
    pub available_work_ms_per_day: Millis,
    pub menu_bar_show_timer: bool,
    /// The one-time panel pointing at the menu bar has been dismissed (D12).
    pub first_run_done: bool,

    /// The working window: when the user asserts they are at the desk
    /// (IDLE_TIME §2). Milliseconds from *local* midnight, not instants — the
    /// day they belong to is resolved by `state::window_for`.
    ///
    /// A different quantity from `available_work_ms_per_day`, which is how much
    /// of the day the user intends to give. 09:00–18:00 with 7h of capacity is
    /// a normal configuration, not a contradiction.
    pub work_start_ms: Millis,
    pub work_end_ms: Millis,
    /// Which weekdays the window applies to. Bitmask, Monday = bit 0.
    pub working_weekdays: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            theme: Theme::System,
            default_block_duration_ms: 30 * 60_000,
            default_break_duration_ms: 10 * 60_000,
            expiration_sound: true,
            system_notification: true,
            available_work_ms_per_day: 420 * 60_000,
            menu_bar_show_timer: true,
            first_run_done: false,
            work_start_ms: 9 * 3_600_000,
            work_end_ms: 18 * 3_600_000,
            working_weekdays: 0b001_1111,
        }
    }
}

impl Settings {
    /// Clamp the values a malformed row or a hand-edited database could carry.
    /// A zero-length default block would create blocks that expire instantly.
    fn sanitized(mut self) -> Self {
        self.default_block_duration_ms = self.default_block_duration_ms.clamp(60_000, 8 * 3_600_000);
        self.default_break_duration_ms = self.default_break_duration_ms.clamp(60_000, 4 * 3_600_000);
        self.available_work_ms_per_day = self.available_work_ms_per_day.clamp(60_000, 16 * 3_600_000);
        self.work_start_ms = self.work_start_ms.clamp(0, DAY_MS - MINUTE_MS);
        self.work_end_ms = self.work_end_ms.clamp(MINUTE_MS, DAY_MS);
        self.working_weekdays &= 0b111_1111;
        // Overnight windows are out of scope (IDLE_TIME §8) and `update_settings`
        // refuses them. A row that carries one anyway — hand-edited, or written
        // by a future version — would make every day report a negative window,
        // so it falls back to the default rather than being half-supported.
        if self.work_start_ms >= self.work_end_ms {
            let d = Settings::default();
            self.work_start_ms = d.work_start_ms;
            self.work_end_ms = d.work_end_ms;
        }
        self
    }

    /// The reason `update_settings` refuses, or `None` if the values are usable.
    pub fn rejection(&self) -> Option<String> {
        (self.work_start_ms >= self.work_end_ms).then(|| {
            "Working hours must start before they end; an overnight window is not supported."
                .to_string()
        })
    }
}

const MINUTES_PER_DAY_COL: Millis = 60_000;
const MINUTE_MS: Millis = 60_000;
const DAY_MS: Millis = 24 * 3_600_000;

pub fn load(conn: &Connection) -> rusqlite::Result<Settings> {
    let s = conn.query_row(
        "SELECT launch_at_login, theme, default_block_duration_ms, default_break_duration_ms,
                expiration_sound, system_notification, available_work_minutes_per_day,
                menu_bar_show_timer, first_run_done,
                work_start_minutes, work_end_minutes, working_weekdays
         FROM settings WHERE id = 1",
        [],
        |r| {
            let theme: String = r.get(1)?;
            let minutes: i64 = r.get(6)?;
            Ok(Settings {
                launch_at_login: r.get::<_, i64>(0)? != 0,
                theme: Theme::parse(&theme),
                default_block_duration_ms: r.get(2)?,
                default_break_duration_ms: r.get(3)?,
                expiration_sound: r.get::<_, i64>(4)? != 0,
                system_notification: r.get::<_, i64>(5)? != 0,
                available_work_ms_per_day: minutes * MINUTES_PER_DAY_COL,
                menu_bar_show_timer: r.get::<_, i64>(7)? != 0,
                first_run_done: r.get::<_, i64>(8)? != 0,
                work_start_ms: r.get::<_, i64>(9)? * MINUTE_MS,
                work_end_ms: r.get::<_, i64>(10)? * MINUTE_MS,
                working_weekdays: r.get::<_, i64>(11)? as u8,
            })
        },
    )?;
    Ok(s.sanitized())
}

pub fn save(conn: &Connection, s: &Settings) -> rusqlite::Result<Settings> {
    let s = s.sanitized();
    conn.execute(
        "UPDATE settings SET launch_at_login=?1, theme=?2, default_block_duration_ms=?3,
                             default_break_duration_ms=?4, expiration_sound=?5,
                             system_notification=?6, available_work_minutes_per_day=?7,
                             menu_bar_show_timer=?8, first_run_done=?9,
                             work_start_minutes=?10, work_end_minutes=?11,
                             working_weekdays=?12
         WHERE id = 1",
        params![
            s.launch_at_login as i64,
            s.theme.as_str(),
            s.default_block_duration_ms,
            s.default_break_duration_ms,
            s.expiration_sound as i64,
            s.system_notification as i64,
            // The column is minutes; the API is milliseconds like everything
            // else. Rounded up so a half-minute is never lost to zero.
            (s.available_work_ms_per_day + MINUTES_PER_DAY_COL - 1) / MINUTES_PER_DAY_COL,
            s.menu_bar_show_timer as i64,
            s.first_run_done as i64,
            // Wall-clock settings the user types, stored as the minutes they
            // are; the rest of the app stays in milliseconds.
            s.work_start_ms / MINUTE_MS,
            s.work_end_ms / MINUTE_MS,
            s.working_weekdays as i64,
        ],
    )?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn defaults_match_the_spec() {
        let db = Db::in_memory().unwrap();
        let s = db.with(load).unwrap();
        assert_eq!(s, Settings::default(), "seeded row must equal SPEC §4.4 defaults");
    }

    #[test]
    fn every_field_round_trips() {
        let db = Db::in_memory().unwrap();
        let want = Settings {
            launch_at_login: true,
            theme: Theme::Dark,
            default_block_duration_ms: 45 * 60_000,
            default_break_duration_ms: 5 * 60_000,
            expiration_sound: false,
            system_notification: false,
            available_work_ms_per_day: 300 * 60_000,
            menu_bar_show_timer: false,
            first_run_done: true,
            work_start_ms: 8 * 3_600_000 + 30 * 60_000,
            work_end_ms: 17 * 3_600_000,
            working_weekdays: 0b011_1111,
        };
        db.with(|c| save(c, &want)).unwrap();
        assert_eq!(db.with(load).unwrap(), want);
    }

    #[test]
    fn an_overnight_window_is_refused_rather_than_half_supported() {
        // The window has to resolve to a pair of instants on one calendar day
        // (IDLE_TIME §8). 22:00-06:00 would need the day boundary to move with
        // it, which is a different feature.
        let bad = Settings {
            work_start_ms: 22 * 3_600_000,
            work_end_ms: 6 * 3_600_000,
            ..Settings::default()
        };
        assert!(bad.rejection().is_some(), "update_settings must refuse it");

        // And a row that carries one anyway falls back rather than reporting a
        // negative window for every day.
        let db = Db::in_memory().unwrap();
        let saved = db.with(|c| save(c, &bad)).unwrap();
        assert_eq!(saved.work_start_ms, Settings::default().work_start_ms);
        assert_eq!(saved.work_end_ms, Settings::default().work_end_ms);
    }

    #[test]
    fn a_zero_block_duration_can_never_be_stored() {
        // Otherwise every new task would create a block that expires on its
        // first tick, and the checkpoint would fire immediately.
        let db = Db::in_memory().unwrap();
        let bad = Settings { default_block_duration_ms: 0, ..Settings::default() };
        let saved = db.with(|c| save(c, &bad)).unwrap();
        assert_eq!(saved.default_block_duration_ms, 60_000);
        assert_eq!(db.with(load).unwrap().default_block_duration_ms, 60_000);
    }
}
