-- Idle time & the working window (docs/features/IDLE_TIME.md).
--
-- Idle is *inferred*: it is window time that no running block covered. That
-- makes two things storable — when the user asserted they would be at the desk
-- (the window), and every interval the timer was not running (idle_spans).

-- Local minutes from midnight. Minutes, not ms: this is a wall-clock setting
-- the user types, and db/settings.rs already converts one such column
-- (available_work_minutes_per_day) at the boundary.
ALTER TABLE settings ADD COLUMN work_start_minutes INTEGER NOT NULL DEFAULT 540   -- 09:00
    CHECK (work_start_minutes BETWEEN 0 AND 1439);
ALTER TABLE settings ADD COLUMN work_end_minutes   INTEGER NOT NULL DEFAULT 1080  -- 18:00
    CHECK (work_end_minutes BETWEEN 1 AND 1440);
-- Bitmask, Monday = bit 0. Default 0b0011111 = Mon–Fri.
ALTER TABLE settings ADD COLUMN working_weekdays   INTEGER NOT NULL DEFAULT 31
    CHECK (working_weekdays BETWEEN 0 AND 127);

-- Every interval the timer was not RUNNING, tagged with why. Banked on the
-- transition rather than derived afterwards, for the same reason away_ms is
-- (D13): a past end_at cannot tell a parked block from an unanswered one.
CREATE TABLE idle_spans (
    id         TEXT    PRIMARY KEY,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER,              -- NULL = open
    reason     TEXT    NOT NULL CHECK (reason IN ('AWAITING','PAUSED','UNTRACKED'))
);
CREATE INDEX idx_idle_spans_started ON idle_spans(started_at);
-- At most one span may be open, mirroring current_block_id.
CREATE UNIQUE INDEX idx_idle_spans_open ON idle_spans(ended_at) WHERE ended_at IS NULL;
