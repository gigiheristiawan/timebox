-- TimeBox initial schema. Mirrors SPEC §4.
--
-- All instants are milliseconds since the Unix epoch, and all durations are
-- milliseconds, matching the domain core exactly. Storing seconds would force a
-- lossy conversion at every save, and repeated pause/resume cycles would
-- accumulate the rounding as real drift in a user's recorded time.

CREATE TABLE tasks (
    id                TEXT PRIMARY KEY,
    title             TEXT NOT NULL CHECK (length(trim(title)) > 0),
    status            TEXT NOT NULL
                        CHECK (status IN ('TODO','IN_PROGRESS','DONE','CANCELLED')),
    priority          TEXT NOT NULL DEFAULT 'MEDIUM'
                        CHECK (priority IN ('LOW','MEDIUM','HIGH')),
    block_duration_ms INTEGER NOT NULL CHECK (block_duration_ms > 0),
    -- NULL when the task is not queued (DONE / CANCELLED). Contiguous from 0
    -- for queued tasks; the queue order is this column's order.
    queue_position    INTEGER,
    created_at        INTEGER NOT NULL,
    completed_at      INTEGER
);

CREATE INDEX idx_tasks_queue  ON tasks (queue_position) WHERE queue_position IS NOT NULL;
CREATE INDEX idx_tasks_status ON tasks (status);

CREATE TABLE time_blocks (
    id                       TEXT PRIMARY KEY,
    -- 'BREAK' blocks carry no task (SPEC D7).
    kind                     TEXT NOT NULL DEFAULT 'WORK' CHECK (kind IN ('WORK','BREAK')),
    task_id                  TEXT REFERENCES tasks(id) ON DELETE CASCADE,

    planned_ms               INTEGER NOT NULL CHECK (planned_ms > 0),
    extension_ms             INTEGER NOT NULL DEFAULT 0 CHECK (extension_ms >= 0),
    -- Times this block was set down mid-flight (SPEC D11).
    interruptions            INTEGER NOT NULL DEFAULT 0 CHECK (interruptions >= 0),
    actual_ms                INTEGER,

    status                   TEXT NOT NULL
                               CHECK (status IN ('PLANNED','RUNNING','PAUSED',
                                                 'AWAITING_DECISION','COMPLETED',
                                                 'SKIPPED','CANCELLED')),

    -- Timer arithmetic (SPEC §6). end_at is an absolute instant; time left is
    -- never stored as a countdown, only as a held remainder while parked.
    started_at               INTEGER,
    ended_at                 INTEGER,
    end_at                   INTEGER,
    paused_at                INTEGER,
    remaining_when_paused_ms INTEGER,
    accumulated_active_ms    INTEGER NOT NULL DEFAULT 0,
    last_resume_at           INTEGER,

    -- A break block has no task; a work block must have one.
    CHECK ((kind = 'BREAK' AND task_id IS NULL)
        OR (kind = 'WORK'  AND task_id IS NOT NULL))
);

CREATE INDEX idx_blocks_task    ON time_blocks (task_id);
CREATE INDEX idx_blocks_started ON time_blocks (started_at);

-- At most one parked (PAUSED) block per task, so returning to a task can never
-- be ambiguous about which remainder it resumes (SPEC D10).
CREATE UNIQUE INDEX idx_blocks_one_parked_per_task
    ON time_blocks (task_id) WHERE status = 'PAUSED' AND task_id IS NOT NULL;

CREATE TABLE app_state (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    timer_state      TEXT NOT NULL
                       CHECK (timer_state IN ('IDLE','RUNNING','PAUSED','AWAITING_DECISION')),
    current_block_id TEXT REFERENCES time_blocks(id) ON DELETE SET NULL,
    updated_at       INTEGER NOT NULL DEFAULT 0
);

INSERT INTO app_state (id, timer_state) VALUES (1, 'IDLE');

CREATE TABLE settings (
    id                             INTEGER PRIMARY KEY CHECK (id = 1),
    launch_at_login                INTEGER NOT NULL DEFAULT 0 CHECK (launch_at_login IN (0,1)),
    theme                          TEXT    NOT NULL DEFAULT 'SYSTEM'
                                     CHECK (theme IN ('SYSTEM','LIGHT','DARK')),
    default_block_duration_ms      INTEGER NOT NULL DEFAULT 1800000 CHECK (default_block_duration_ms > 0),
    default_break_duration_ms      INTEGER NOT NULL DEFAULT 600000  CHECK (default_break_duration_ms > 0),
    expiration_sound               INTEGER NOT NULL DEFAULT 1 CHECK (expiration_sound IN (0,1)),
    system_notification            INTEGER NOT NULL DEFAULT 1 CHECK (system_notification IN (0,1)),
    available_work_minutes_per_day INTEGER NOT NULL DEFAULT 420 CHECK (available_work_minutes_per_day > 0),
    menu_bar_show_timer            INTEGER NOT NULL DEFAULT 1 CHECK (menu_bar_show_timer IN (0,1))
);

INSERT INTO settings (id) VALUES (1);
