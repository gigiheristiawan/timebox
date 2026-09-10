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

/// Where the overlay sits on the main display (issue #23). A corner or the
/// centre — not free coordinates: the window cannot be dragged (it ignores the
/// cursor, D48), so a stored position has to be one the user can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl OverlayPosition {
    pub fn as_str(self) -> &'static str {
        match self {
            OverlayPosition::TopLeft => "TOP_LEFT",
            OverlayPosition::TopRight => "TOP_RIGHT",
            OverlayPosition::BottomLeft => "BOTTOM_LEFT",
            OverlayPosition::BottomRight => "BOTTOM_RIGHT",
            OverlayPosition::Center => "CENTER",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "TOP_LEFT" => OverlayPosition::TopLeft,
            "TOP_RIGHT" => OverlayPosition::TopRight,
            "BOTTOM_LEFT" => OverlayPosition::BottomLeft,
            "CENTER" => OverlayPosition::Center,
            _ => OverlayPosition::BottomRight,
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

    /// The floating timer overlay (issue #23). Presentation only: it shows the
    /// state, offers no control, and no value here changes whether a
    /// checkpoint appears.
    pub overlay_show: bool,
    pub overlay_show_task_title: bool,
    pub overlay_position: OverlayPosition,
    /// Whole percent. Applied as the window's alpha, so 0 is invisible — the
    /// slider the user reads says so.
    pub overlay_opacity_pct: u8,
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
            overlay_show: false,
            overlay_show_task_title: true,
            overlay_position: OverlayPosition::BottomRight,
            overlay_opacity_pct: 85,
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
        self.overlay_opacity_pct = self.overlay_opacity_pct.min(100);
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
                work_start_minutes, work_end_minutes, working_weekdays,
                overlay_show, overlay_show_task_title, overlay_position, overlay_opacity_pct
         FROM settings WHERE id = 1",
        [],
        |r| {
            let theme: String = r.get(1)?;
            let minutes: i64 = r.get(6)?;
            let overlay_position: String = r.get(14)?;
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
                overlay_show: r.get::<_, i64>(12)? != 0,
                overlay_show_task_title: r.get::<_, i64>(13)? != 0,
                overlay_position: OverlayPosition::parse(&overlay_position),
                overlay_opacity_pct: r.get::<_, i64>(15)? as u8,
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
                             working_weekdays=?12, overlay_show=?13,
                             overlay_show_task_title=?14, overlay_position=?15,
                             overlay_opacity_pct=?16
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
            s.overlay_show as i64,
            s.overlay_show_task_title as i64,
            s.overlay_position.as_str(),
            s.overlay_opacity_pct as i64,
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
            overlay_show: true,
            overlay_show_task_title: false,
            overlay_position: OverlayPosition::Center,
            overlay_opacity_pct: 40,
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

    /// Test 102. The overlay is off for an existing install and the slider is
    /// where the default puts it. A window that appeared over everything on
    /// upgrade would be a surprise, and this one cannot be clicked away —
    /// closing it means finding the setting (D48).
    #[test]
    fn t102_the_overlay_is_off_until_it_is_asked_for() {
        let db = Db::in_memory().unwrap();
        let s = db.with(load).unwrap();
        assert!(!s.overlay_show, "migration 007 must upgrade to the overlay off");
        assert!(s.overlay_show_task_title);
        assert_eq!(s.overlay_position, OverlayPosition::BottomRight);
        assert_eq!(s.overlay_opacity_pct, 85);
    }

    /// Test 103. The percentage is applied as the window's alpha, and AppKit
    /// takes any float — a stored 250 would be silently meaningless rather than
    /// rejected, so it is clamped on the way in and on the way out.
    #[test]
    fn t103_an_out_of_range_opacity_is_clamped_rather_than_passed_to_appkit() {
        let db = Db::in_memory().unwrap();
        let bad = Settings { overlay_opacity_pct: 250, ..Settings::default() };
        assert_eq!(db.with(|c| save(c, &bad)).unwrap().overlay_opacity_pct, 100);
        assert_eq!(db.with(load).unwrap().overlay_opacity_pct, 100);
    }

    /// Test 104. `BOTTOM_RIGHT` is what the *database* stores, `BottomRight`
    /// what the UI sends — the same deliberate split as `Theme` and
    /// `TimerState`. Both encodings are load-bearing and neither fails loudly:
    /// a mismatch would just park the card in the default corner forever.
    #[test]
    fn t104_the_position_encodings_are_the_database_one_and_the_wire_one() {
        for (p, sql, wire) in [
            (OverlayPosition::TopLeft, "TOP_LEFT", r#""TopLeft""#),
            (OverlayPosition::TopRight, "TOP_RIGHT", r#""TopRight""#),
            (OverlayPosition::BottomLeft, "BOTTOM_LEFT", r#""BottomLeft""#),
            (OverlayPosition::BottomRight, "BOTTOM_RIGHT", r#""BottomRight""#),
            (OverlayPosition::Center, "CENTER", r#""Center""#),
        ] {
            assert_eq!(p.as_str(), sql);
            assert_eq!(OverlayPosition::parse(sql), p);
            assert_eq!(serde_json::to_string(&p).unwrap(), wire);
            let back: OverlayPosition = serde_json::from_str(wire).unwrap();
            assert_eq!(back, p);
        }
    }

    /// Test 105. Every position must survive the round trip — the CHECK in 007
    /// enumerates them, so a value the enum knows and the column does not fails
    /// at runtime, not at compile time (the lesson of migration 006).
    #[test]
    fn t105_every_position_is_accepted_by_the_column() {
        let db = Db::in_memory().unwrap();
        for p in [
            OverlayPosition::TopLeft,
            OverlayPosition::TopRight,
            OverlayPosition::BottomLeft,
            OverlayPosition::BottomRight,
            OverlayPosition::Center,
        ] {
            let want = Settings { overlay_position: p, ..Settings::default() };
            db.with(|c| save(c, &want)).unwrap();
            assert_eq!(db.with(load).unwrap().overlay_position, p);
        }
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
