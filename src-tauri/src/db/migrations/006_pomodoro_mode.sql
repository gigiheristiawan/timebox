-- Pomodoro mode (issue #15).
--
-- Two changes to `app_state`, and the second is why this is a table rebuild
-- rather than a bare ALTER.
--
-- 1. `pomodoro_since` — the instant the current pomodoro accrues from. One
--    column, not two: it is NULL exactly when the mode is off, so it doubles
--    as the mode flag rather than mirroring a separate `settings.pomodoro_mode`
--    boolean that would have to be kept in step with it across every toggle.
--
-- 2. `timer_state` gains 'AWAITING_POMODORO'. The CHECK constraint from 001
--    enumerates the states, and SQLite cannot alter a CHECK in place — the
--    table has to be recreated. Without this the constraint rejects every
--    write made while the Pomodoro prompt is open, which is a *runtime*
--    failure the Rust type system cannot catch.
--
-- Existing installs come back with the mode off, which is the right default:
-- a mode that turned itself on at upgrade would be a surprise interruption.

PRAGMA foreign_keys = OFF;

CREATE TABLE app_state_new (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    timer_state      TEXT NOT NULL
                       CHECK (timer_state IN ('IDLE','RUNNING','PAUSED',
                                              'AWAITING_DECISION','AWAITING_POMODORO')),
    current_block_id TEXT REFERENCES time_blocks(id) ON DELETE SET NULL,
    updated_at       INTEGER NOT NULL DEFAULT 0,
    pomodoro_since   INTEGER
);

INSERT INTO app_state_new (id, timer_state, current_block_id, updated_at)
SELECT id, timer_state, current_block_id, updated_at FROM app_state;

DROP TABLE app_state;
ALTER TABLE app_state_new RENAME TO app_state;

PRAGMA foreign_keys = ON;
