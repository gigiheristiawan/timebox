-- Phase 7. Two things the polish phase needs that the schema could not derive.
--
-- away_ms (SPEC D13): time spent at an unanswered checkpoint. It cannot be
-- reconstructed from end_at after the fact — a *parked* block also carries a
-- stale end_at, and a block extended after expiring reaches the checkpoint more
-- than once — so the gap is banked on the block each time it is answered.
-- The currently open checkpoint's gap is still derived live (`staleness_ms`).
ALTER TABLE time_blocks ADD COLUMN away_ms INTEGER NOT NULL DEFAULT 0 CHECK (away_ms >= 0);

-- first_run_done (SPEC D12): the one-time panel pointing at the menu bar. An
-- accessory app has no Dock icon to fall back on, so the pointer has to be
-- shown once and then never again.
ALTER TABLE settings ADD COLUMN first_run_done INTEGER NOT NULL DEFAULT 0
    CHECK (first_run_done IN (0,1));
