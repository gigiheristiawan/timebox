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
        self
    }
}

const MINUTES_PER_DAY_COL: Millis = 60_000;

pub fn load(conn: &Connection) -> rusqlite::Result<Settings> {
    let s = conn.query_row(
        "SELECT launch_at_login, theme, default_block_duration_ms, default_break_duration_ms,
                expiration_sound, system_notification, available_work_minutes_per_day,
                menu_bar_show_timer, first_run_done
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
                             menu_bar_show_timer=?8, first_run_done=?9
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
        };
        db.with(|c| save(c, &want)).unwrap();
        assert_eq!(db.with(load).unwrap(), want);
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
