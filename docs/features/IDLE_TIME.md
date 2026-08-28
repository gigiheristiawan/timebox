# Feature Spec — Idle Time & the Working Window

How TimeBox measures time the user was *not* working, so a daily report can say
something true about how the day went. Status: **implemented 2026-08-24**
(migration 003; `core::summary`, `core::timer_machine`, `state::window_for`).
D21 (sleep) is still outstanding — see `SLEEP_DETECTION.md`.
Supersedes nothing; extends `docs/SPEC.md` §4 (data model), §11 (edge cases) and
D13 (time at an unanswered checkpoint).

---

## Changelog

| Date (WIB)       | Change                                                                 |
| ---------------- | ---------------------------------------------------------------------- |
| 2026-08-28 17:05 | **`ExtendBreak` adds to the break instead of replacing it (issue #10).** It set `end_at = now + ms` unconditionally, which is right at the break checkpoint — nothing is left there — and wrong everywhere else: `+5` four minutes into a 10-minute break cut it to five. The grant now pushes `end_at` forward while the break runs, lands on `remaining_when_paused_ms` while it is paused (and no longer force-resumes it), and still reads `now + ms` at the checkpoint. D22 unchanged; test 53 added. |
| 2026-08-28 15:10 | **Cross-day attribution is no longer a limit — §8's first bullet is struck (issue #11).** A block started 23:32 and extended into the next afternoon reported its whole 1h40 to the day it *started*, so the second day read `worked_ms` = 0 for the task actually being worked on. Migration **004** adds `work_spans(block_id, started_at, ended_at)`, the mirror of `idle_spans`: `timer_machine::sync_work` opens one whenever a block starts or resumes and closes it whenever it stops, so the two sets partition the timeline. `core::summary` now measures a day as `spans ∩ day` (per task, too) and takes a `day_end` argument beside `day_start` — `state::day_end_ms` resolves the next *local* midnight, so a DST day is 23 or 25 hours. Blocks written before the migration have no spans and keep the old `started_at` attribution, so past days read as they always did. `blocks_completed` now counts on the day a block **ended**, so one crossing midnight is not counted twice. Tests 48–52. |
| 2026-08-24 19:10 | **The Idle line refreshes while the timer is stopped.** It was only ever repainted by the tick loop, which parks in the very states idle accrues in, so an open window showed a frozen figure until the next action. Fixed by emitting `timebox://changed` on window focus and polling every 10s while not `RUNNING`. Nothing moved into TypeScript — idle is a set difference over `idle_spans`, which the snapshot does not carry. |
| 2026-08-24 18:10 | **The tray's break item follows the state.** It read *Take a break* during a break, where clicking it did nothing — the reducer refused it correctly and the menu said otherwise. It now reads *End break* during a break and is disabled at a work checkpoint. §6 amended. |
| 2026-08-24 17:45 | **D16 fix — the park is hooked on `RunEvent::Exit`, not `ExitRequested`.** Found in manual testing: quitting from the popover parked correctly, but `Cmd+Q` left the block `RUNNING` and the clock ran across the quit. `Cmd+Q` is muda's predefined Quit item, which sends `terminate:` to `NSApplication`; tao emits `Exit` alone for that path and no `ExitRequested` at all. `Exit` is reached by every quit path, and `Event::Pause` is already a no-op unless the timer is running, so arriving there already parked costs nothing. Corrects plan task I3.2. |
| 2026-08-24 16:40 | **Implemented.** Three corrections the code forced, each also fixed in the body below: (1) **D20 is not implemented as written** — closing an open span at the last state write cannot work here, because nothing writes while a span is open (the tick thread parks in `IDLE`/`PAUSED`/`AWAITING_DECISION`), so `updated_at` almost always equals the span's own `started_at` and "closing at the last write" would erase the whole gap. Worse, D16 makes a clean quit produce exactly that shape, which would have failed tests 26 and 27. An open span therefore **survives an unclean exit and keeps accruing**, which is what D15 says anyway: the app need not be running for idle to accrue. `time_blocks.updated_at` was consequently not added — nothing would read it. (2) **Test 26 said `idle_untracked_ms`**; the 12:05 entry closing Q2 had already made a quit-parked span `PAUSED`, so it is `idle_paused_ms`. Test 26 was stale, not the decision. (3) `settings` gained the three window columns as specified; the rest of §5.1's DDL is verbatim. |
| 2026-08-24 12:05 | Two implementation questions closed and folded back in (see `IDLE_TIME_PLAN.md` Q1/Q2): break time accumulates regardless of how the break was started, and a quit-parked block's idle span is tagged **`Paused`** — so D16's parking is the existing `Event::Pause`, not a new event. §3.1 and D16 amended. |
| 2026-08-24 11:52 | **D22 added** — a break can be started deliberately (`StartBreak`), not only from a checkpoint. Without it every rest taken between checkpoints is recorded as idle, which makes the measure punish honesty; §3.3 and §6.1 added. |
| 2026-08-24 11:20 | **D21 added** (sleep parks the block, specified in `SLEEP_DETECTION.md`); §9's two open questions resolved — the D14 quit confirmation is deleted, and Idle stays out of the menu bar. §8's "sleep and screen lock are idle" limit was only half true and is corrected in place. |
| 2026-08-24 10:51 | Initial version. Model agreed with Gigih in discussion; decisions D15–D20. |

---

## 1. The problem

Today the app measures three things: work (`Today.worked_ms`), break
(`Today.break_ms`), and time at an unanswered checkpoint (`away_ms`, D13).
Between them they cover only the moments a block existed. The hours a working
day loses — a morning that never starts, a block paused and forgotten, the app
quit at 15:00 — are invisible, and a daily effectiveness report built on the
current numbers would flatter every one of those days.

This spec adds the missing measure: **idle time**.

## 2. The principle

> Worked time is **observed**. Idle time is **inferred**.

A running block is a positive fact: something happened, and it is recorded.
Idle is the *absence* of a block — and an absence only means something inside a
span where the user asserted they would be at the desk. That span is the
**working window**.

Every rule below follows from this asymmetry, and it is the reason the feature
is not symmetric about the window's edges: work outside working hours is
recorded, idle outside working hours is not, because outside the window no
claim of presence was ever made.

**What this is not.** TimeBox does not observe the human. There is no input
monitoring, no screen-lock hook, no `NSWorkspace` idle query — all of which
would be new sandbox entitlements for a worse answer. The metric is *"time in
your working window that no block covered."* The UI must name it in those
terms and never as "time you were not working".

## 3. Definition

For a given calendar day `D` (local, resolved by `state::day_start_ms`):

```
window(D)   = [work_start, work_end] on D   if D is a working weekday
            = ∅                              otherwise

covered(D)  = the union of wall-clock intervals during which any block
              (work or break) was RUNNING on D

idle(D)     = |window(D) \ covered(D)|,   clipped to [day_start, now]
```

`idle` is therefore *not* `window − worked − break`. Those are durations; this
is a set difference over intervals, which is what makes it correct when a block
runs past `work_end` or a paused block spans a gap.

### 3.1 Sub-buckets

Idle is reported as one number and three causes, because they diagnose
different failures:

| Bucket | Meaning | Diagnosis |
| --- | --- | --- |
| `idle_awaiting_ms` | at an open checkpoint (the existing `away_ms`, D13) | slow to decide |
| `idle_paused_ms` | a block held in `PAUSED` — by the Pause control *or* by quitting (D16) | stopped and did not restart |
| `idle_untracked_ms` | no block at all — before the first, between blocks, after the last | never started |

The three sum to `idle_ms` exactly; every in-window uncovered instant falls in
exactly one, since `TimerState` is total.

### 3.2 Outside-hours work

Work performed outside `window(D)` — evenings, weekends, before `work_start` —
is recorded as worked and reported separately as `outside_hours_ms`. It is a
time-management signal in its own right, not an error, and it is never
subtracted from anything.

### 3.3 Deliberate breaks

The measure only means something if the user can tell it the truth. Today the
sole entrance to a break is `DecideBreak` at a checkpoint (`core/timer_machine.rs`
`Event::DecideBreak`, D7) — so lunch at 12:30, mid-block, is unrecordable, and
lands in `idle_untracked_ms` next to a wasted afternoon.

That is a measurement failure, not just a missing convenience. It makes idle
uninterpretable — 45 uncovered minutes could be lunch or drift, and the report
cannot distinguish them — and it makes the metric *worse* for the user who rests
deliberately, which is the opposite of what the app claims about rest (D9). D22
closes it.

---

## 4. Resolved decisions

Numbering continues `docs/SPEC.md` §3.

| # | Decision | Resolution |
| --- | --- | --- |
| D15 | What counts as idle | Window time not covered by a running block. The app need not be running for idle to accrue — quitting at 15:00 with an 18:00 `work_end` yields three hours of idle. Quitting *is* a deliberate stop, but it stops the work, not the clock the day is measured against. |
| D16 | Quitting parks the running block | Quit banks the block's remainder exactly as `Pause` does. **This reverses the invariant "`end_at` is absolute … quitting does not stop the clock" (SPEC §6).** It is forced by D15: an interval cannot be idle and worked at once. The anti-gaming reason behind the old rule survives — parking preserves the *remainder* and never re-grants a block, so quitting is no more exploitable than the existing Pause button. Quitting is therefore not merely *like* a pause, it **is** one: the exit path dispatches the existing `Event::Pause`, the block ends `PAUSED`, and the gap is tagged `idle_paused_ms` (§3.1). No new event — and `Pause`'s existing no-op behaviour at a checkpoint and in `IDLE` is already the right answer for quitting from those states. |
| D17 | Reopening after `work_end` | The day's **idle** is sealed at `work_end`; the day's **work** is not. Starting a block at 20:00 records worked time (as `outside_hours_ms`) and accrues no idle. Sealing exists for correctness as much as reporting: without it, a block left running over a long weekend reports days of work on the next hydrate. |
| D18 | Non-working days | A weekend is a day whose window is empty. Blocks still run and are still recorded, filed under that calendar day in `outside_hours_ms`, with zero idle. Continuing Friday's block on Saturday needs no special case. |
| D19 | Days with no activity | A day on which no block was ever started produces **no report and no idle**, regardless of the weekday setting. This covers public holidays and sick days without a holiday calendar. Accepted cost: a genuinely wasted working day also reports nothing — it is indistinguishable from a day off, and the app should not guess which it was. |
| D21 | Sleep is treated as a quit | A detected sleep parks the running block at the instant the Mac slept; the gap is idle, not work. Without it D16 seals the smaller hole and leaves the larger one open — closing the lid is the more common way to leave a desk than quitting. Detection and its failure modes are specified separately in `docs/features/SLEEP_DETECTION.md`; this spec depends only on the outcome, that the sleep gap arrives as `idle_untracked_ms`. |
| D22 | Breaks can be started deliberately | A new `StartBreak { ms }` action starts a break from `IDLE`, `RUNNING` or `PAUSED`, without waiting for a checkpoint. A break taken mid-block **parks** the work block (D10's mechanism) and, unlike a switch, leaves the task at the **head** of the queue — a break is a return to the same work, not a rotation away from it. It does **not** increment `interruptions` (D11): that counter measures churn between tasks, and tinting Today's *Switched early* warning because someone took lunch would contradict D9. Refused (no-op) at a work checkpoint, where the checkpoint has no side doors and D7 already offers break as a compound action, and during a break, where `ExtendBreak` is the operation. D7 is unchanged: at a checkpoint break remains a *modifier on the task decision*; away from one it stands alone. |
| D20 | Recovering an unclosed idle span | **Revised at implementation, 2026-08-24.** An open span survives an unclean exit and **keeps accruing**; it is not closed at the last state write. The original rule assumed `repo::save` runs periodically during a span, but the tick thread parks in `IDLE`, `PAUSED` and `AWAITING_DECISION` — so nothing writes while a span is open and `updated_at` is almost always the span's own `started_at`. Closing there would erase the entire gap, not just its tail. And under D16 a *clean* quit produces the same shape, so the rule would have deleted the very interval tests 26 and 27 exist to protect. Keeping the span open is also the more truthful answer: a paused timer is still paused whether or not the app is alive, which is D15's whole premise. **The gap this leaves open** is a crash while `RUNNING`, where no span exists and no park was written — that interval is still credited as work, capped at the block's allocation. D21 narrows it for the common cause (sleep); a genuine crash mid-block remains a known, bounded over-report. |

---

## 5. Data model

### 5.1 Migration `003_working_window.sql`

Forward-only, per the migration rule in `CLAUDE.md`.

```sql
-- Local minutes from midnight. Minutes, not ms: this is a wall-clock
-- setting the user types, and db/settings.rs already converts one such
-- column (available_work_minutes_per_day) at the boundary.
ALTER TABLE settings ADD COLUMN work_start_minutes INTEGER NOT NULL DEFAULT 540   -- 09:00
    CHECK (work_start_minutes BETWEEN 0 AND 1439);
ALTER TABLE settings ADD COLUMN work_end_minutes   INTEGER NOT NULL DEFAULT 1080  -- 18:00
    CHECK (work_end_minutes BETWEEN 1 AND 1440);
-- Bitmask, Monday = bit 0. Default 0b0011111 = Mon–Fri.
ALTER TABLE settings ADD COLUMN working_weekdays   INTEGER NOT NULL DEFAULT 31
    CHECK (working_weekdays BETWEEN 0 AND 127);

-- NOT SHIPPED (2026-08-24). D20's crash fallback was revised away, and nothing
-- would have read this column. `app_state.updated_at` already records the last
-- state write if a future feature needs one.
-- ALTER TABLE time_blocks ADD COLUMN updated_at INTEGER;

CREATE TABLE idle_spans (
    id         TEXT    PRIMARY KEY,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER,              -- NULL = open
    reason     TEXT    NOT NULL CHECK (reason IN ('AWAITING','PAUSED','UNTRACKED'))
);
CREATE INDEX idx_idle_spans_started ON idle_spans(started_at);
-- At most one span may be open, mirroring current_block_id.
CREATE UNIQUE INDEX idx_idle_spans_open ON idle_spans(ended_at) WHERE ended_at IS NULL;
```

`work_end_minutes` allows 1440 so an 09:00–24:00 window is expressible.
`work_start >= work_end` is rejected at the settings boundary; overnight
windows are **out of scope** (see §8).

### 5.1b Migration `004_work_spans.sql` (issue #11)

```sql
CREATE TABLE work_spans (
    id         TEXT    PRIMARY KEY,
    block_id   TEXT    NOT NULL REFERENCES time_blocks(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER              -- NULL = still running
);
CREATE UNIQUE INDEX idx_work_spans_open ON work_spans(ended_at) WHERE ended_at IS NULL;
```

The same argument as §5.2, applied to the other half of the timeline.
`accumulated_active_ms` is a total with no shape, so it can only be attributed
whole — and a block with extensions routinely outlives the day it started in.
`sync_work` reconciles against the *state* rather than the event, because a
switch leaves the timer RUNNING while changing which block is running; that
also self-heals a database written before this table existed.

### 5.2 Why spans and not a counter

Spans are banked on transition, not derived after the fact — the same reasoning
that makes `away_ms` a stored field (D13). A per-day counter would have to be
attributed at write time, which means resolving the local day inside the core;
spans keep the day boundary *injected* into `core::summary`, as it is today.

### 5.3 D22 needs no schema

A deliberate break is an ordinary `BlockKind::Break` row — no task, excluded from
worked totals and from capacity (D7, D9). Nothing distinguishes it in storage
from a break granted at a checkpoint, and nothing needs to: the report asks how
much rest was taken, not which door it came through. **Confirmed 2026-08-24** —
the alternative was a `source` column, which cannot be backfilled and so had to
be decided before migration 003 ships.

### 5.4 Core

- `MachineState` gains `open_idle: Option<IdleSpan>` and `idle_spans: Vec<IdleSpan>`.
- `reduce` closes the open span on entering `Running`, and opens one on leaving
  it, tagged by the state entered. This is the only new transition logic; it
  belongs beside `settle_away`, which becomes the `AWAITING` span's writer
  rather than a separate mechanism.
- `Event::StartBreak { ms }` and the matching `Action::StartBreak` (D22). The
  reducer's break-entry path already exists in `DecideBreak`; the new event
  reuses it, differing only in parking the work block instead of ending it and
  in leaving the queue order alone.
- `core::summary::summarize` gains `idle_ms`, the three sub-buckets, and
  `outside_hours_ms`, computed against a `window: Option<(Millis, Millis)>`
  argument — injected by `state.rs`, like `day_start` already is, so the core
  stays free of timezone and calendar knowledge.

---

## 6. Surfaces

- **Settings**: a *Working hours* group — start, end, and a weekday picker.
  Adjacent to `available_work_minutes_per_day`, which is a **different**
  quantity and must not be conflated in the copy: capacity is how much of the
  day you intend to *give*; the window is when you are at the desk. A 09:00–18:00
  window with 7h capacity is a coherent, normal configuration.
- **Today (popover)**: one `Idle` line beside the existing `Away` line. `Away`
  becomes a sub-bucket of it, so the two must not be shown as peers — the
  simplest resolution is to replace the `Away` line with `Idle`, detailing the
  causes in the report rather than the popover.
- **Take a break** (D22): a control in the popover — a duration segmented
  control defaulting to `default_break_duration_ms`, matching the checkpoint's,
  plus the same action in the tray menu (right-click; left-click stays the
  popover) so a break can be started without opening the popover. That one item
  follows the state — it reads **End break** during a break, and is shown
  disabled at a work checkpoint — because a menu has no room to explain why a
  control it is offering would do nothing. It is enabled in `IDLE`, `RUNNING` and `PAUSED`, and
  disabled with the reason shown at a checkpoint. Ending early uses the existing
  `EndBreak`; expiry lands on the existing break checkpoint (D8), whose
  *Start <next task>* names the parked task, since D22 did not rotate the queue.
- **Daily report**: out of scope here. This spec defines the measure it needs;
  the report is a separate feature spec.

---

## 7. Acceptance tests

Continuing the numbering in `docs/SPEC.md` §12; Rust test names must match.
34–41 are not missing — they belong to `docs/features/SLEEP_DETECTION.md` (D21).

| # | Test |
| --- | --- |
| 23 | Window 09:00–18:00, first block starts 10:30 → `idle_untracked_ms` = 90m. |
| 24 | Block paused 11:00, resumed 11:20 → `idle_paused_ms` = 20m; `worked_ms` excludes the pause (existing D4). |
| 25 | Block expires 14:00, checkpoint answered 14:25 → `idle_awaiting_ms` = 25m, matching the existing `away_ms`. |
| 26 | Quit 15:00 with a block running, reopen 16:00 → block is `PAUSED` holding its remainder (D16), `idle_paused_ms` = 60m (the span is tagged by the state the block is in — corrected 2026-08-24, it read `idle_untracked_ms`), `worked_ms` unchanged across the gap. |
| 27 | Quit 17:30, reopen next morning → idle stops at 18:00, not at reopen. Total idle for the day is bounded by the window. |
| 28 | Block started 20:00, after `work_end` → `outside_hours_ms` > 0, `idle_ms` unchanged (D17). |
| 29 | Saturday, weekday bit unset, a block runs → `worked_ms` and `outside_hours_ms` recorded, `idle_ms` = 0 (D18). |
| 30 | A working Tuesday with zero blocks → no report row, `idle_ms` = 0 (D19). |
| 31 | Open idle span, no clean exit → on hydrate the span comes back **open** and keeps accruing; the gap between the last state write and the relaunch is idle, not work. *(Rewritten 2026-08-24: the original read "the span closes at 16:10, not at `now`". See D20 and the changelog.)* |
| 32 | `idle_awaiting_ms + idle_paused_ms + idle_untracked_ms == idle_ms` for any sequence of events (property test). |
| 33 | A block running from 17:50 to 18:20 contributes no idle for 17:50–18:00 and none after (set difference, not subtraction). |
| 42 | `StartBreak` from `RUNNING` parks the work block holding its remainder, starts a break, and leaves the task at the queue **head** — the break checkpoint then offers that same task, not the next one (D22). |
| 43 | `StartBreak` from `RUNNING` does not increment `interruptions`, where `SwitchTo` from the same state does (D22 vs D11). |
| 44 | `StartBreak` at a work checkpoint is a no-op, alongside `SwitchTo` / `Pause` / `Resume`. |
| 45 | `StartBreak` during a running break is a no-op; `ExtendBreak` is the operation. |
| 46 | A 45 m deliberate break inside the window contributes to `break_ms` and **zero** to `idle_ms` — the point of D22. |
| 47 | A deliberate break does not consume `available_work_minutes_per_day` (D9 unchanged). |
| 48 | A block worked 23:40–00:10 gives the first day 20m and the second 10m; the two together are the block, neither losing nor duplicating it. The second day's top list attributes its 10m to the task (issue #11). |
| 49 | A block parked at 23:55 and resumed at 00:05 counts that interval as worked on neither day. |
| 50 | A block completed at 00:12 counts in `blocks_completed` on the day it finished, and only there. |
| 51 | A block written before migration 004 carries no spans and keeps its `started_at` attribution — a past day reads as it always did rather than dropping to zero. |
| 52 | A running block loaded without an open span opens one on the first `Tick` (hydrate's), and records from that instant on; the lost interval is not invented back. |
| 53 | `ExtendBreak` **adds** to the break: running with 6m left, `+5` gives 11m, not 5m (issue #10). Paused, the grant lands on the held remainder and the break stays paused. At the break checkpoint nothing is left, so the grant is the whole amount. |

---

## 8. Known limits, accepted

- ~~**Block-to-day attribution stays `started_at`-based.**~~ **Struck
  2026-08-28 (issue #11).** It *was* mistaken for a bug, correctly: with
  extensions a block routinely lives for a day or more, and the day it was
  worked reported zero. `work_spans` now splits it at the boundary; only blocks
  predating migration 004 still attribute whole.
- **Overnight windows (e.g. 22:00–06:00) are unsupported.** Night shifts would
  require the window to span two calendar days and the day boundary to move
  with it — a different feature. The settings validation rejects it rather than
  half-supporting it.
- **No holiday calendar.** D19 covers holidays adequately.
- **Screen lock is idle; sleep is idle only via D21.** A Mac asleep or locked
  during working hours with *no block running* is uncovered window time and
  needs nothing new. With a block running, the clock would otherwise credit the
  gap as work — that case is D21's, and until it lands this spec's idle figure
  understates a day spent with the lid closed mid-block.
- **D16 costs the "clock is absolute" guarantee.** `docs/SPEC.md` §6 and the
  invariant list in `CLAUDE.md` must both be amended when this lands, with a
  changelog entry saying which claim was reversed and why.

---

## 9. Resolved during specification

1. **The D14 quit confirmation is deleted.** Its whole subject is the cost of
   quitting — *"Quitting won't pause it"*, *"keeps consuming its allocation"* —
   and D16 removes that cost. Both statements become false, `Pause & Quit` and
   `Quit` become the same action, and `Cancel` is left as friction on a quit the
   user asked for with nothing to protect them from. Removed with it:
   `platform/quit_confirm.rs`, `src/components/QuitConfirm.tsx`, its route in
   `src/main.tsx`, and the `confirmQuit` / `cancelQuit` commands. The
   `RunEvent::ExitRequested` handler stays and shrinks to *park the block, then
   exit*.

   The dialog's choice disappears because the behaviour choice disappears: after
   D16 there is no way to quit and leave the clock running. Preserving that as a
   setting was considered and rejected — it would reintroduce exactly the
   ambiguity D15 resolves, an interval that is both idle and worked.

2. **Idle does not appear in the menu bar title.** The menu bar is for the
   current block. Idle belongs in Today and the daily report.
