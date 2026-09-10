-- The timer overlay (issue #23).
--
-- A floating always-on-top card showing the current block, so the timer is
-- readable without leaving the app you are working in. It is presentation
-- only: nothing here decides anything about the timer, which is why it lives
-- in `settings` rather than in `app_state`.
--
-- Four columns, all with a default, so an existing install upgrades to the
-- overlay switched *off*. A window that appeared over everything at upgrade
-- would be a surprise, and this one cannot be clicked away — it ignores the
-- cursor entirely (D48).
ALTER TABLE settings ADD COLUMN overlay_show            INTEGER NOT NULL DEFAULT 0
    CHECK (overlay_show IN (0,1));
-- The title is the half that leaks: a card on a shared screen shows what you
-- are working on. Separately switchable from the clock for that reason alone.
ALTER TABLE settings ADD COLUMN overlay_show_task_title INTEGER NOT NULL DEFAULT 1
    CHECK (overlay_show_task_title IN (0,1));
ALTER TABLE settings ADD COLUMN overlay_position        TEXT    NOT NULL DEFAULT 'BOTTOM_RIGHT'
    CHECK (overlay_position IN ('TOP_LEFT','TOP_RIGHT','BOTTOM_LEFT','BOTTOM_RIGHT','CENTER'));
-- Whole percent, not a fraction: it is a slider the user reads in percent, and
-- the same argument as `available_work_minutes_per_day` — store the unit the
-- setting is typed in and convert at the boundary if the app needs another.
ALTER TABLE settings ADD COLUMN overlay_opacity_pct     INTEGER NOT NULL DEFAULT 85
    CHECK (overlay_opacity_pct BETWEEN 0 AND 100);
