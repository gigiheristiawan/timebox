-- Per-block running intervals (issue #11).
--
-- `time_blocks.accumulated_active_ms` is a total with no shape, so a day's
-- worked time could only be attributed by `started_at` — a block started at
-- 23:32 and worked until 13:12 the next day counted entirely to the first day,
-- and the second reported zero for the task it was actually spent on.
--
-- The mirror of `idle_spans`: a span opens whenever a block starts or resumes
-- and closes whenever it stops, so the two sets partition the timeline.
CREATE TABLE work_spans (
    id         TEXT    PRIMARY KEY,
    block_id   TEXT    NOT NULL REFERENCES time_blocks(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER              -- NULL = still running
);
CREATE INDEX idx_work_spans_block   ON work_spans(block_id);
CREATE INDEX idx_work_spans_started ON work_spans(started_at);
-- At most one span may be open, mirroring current_block_id.
CREATE UNIQUE INDEX idx_work_spans_open ON work_spans(ended_at) WHERE ended_at IS NULL;
