# Idle Time — Implementation Plan

**Companion to:** [IDLE_TIME.md](IDLE_TIME.md) · **Follows:** [SLEEP_DETECTION.md](SLEEP_DETECTION.md) (D21, sequenced after this)
**Status:** ✅ Complete — code and manual acceptance · **Last updated:** 2026-08-24 18:40

Everything here is derived from `IDLE_TIME.md` — if the two disagree, the spec
wins and this file should be corrected. Task ids are prefixed `I` so they cannot
be confused with the MVP plan's in `docs/IMPLEMENTATION_PLAN.md`.

---

## Changelog

| Date (WIB)       | Change                                              |
| ---------------- | --------------------------------------------------- |
| 2026-08-24 19:10 | **G1 fixed.** The Idle figure no longer sits frozen. `timebox://changed` is now also emitted on window focus (`lib.rs`), and `useTimebox.init` polls every 10s while the timer is not `Running` — the states where the tick thread parks on its condvar and the backend goes silent, which is exactly when idle accrues. This also unfreezes `stalenessMs`, whose checkpoint line had the same flaw since Phase 7. No arithmetic moved to TypeScript: the UI still asks Rust again (R6/R7), and could not interpolate idle anyway, since `idle_spans` are `#[serde(skip)]`. |
| 2026-08-24 18:40 | **I6.5 passed — the feature is done.** Gigih ran the full manual pass on `tauri:dev`: both quit doors park the block, the popover and tray breaks park the work block and return to the same task, the working-hours group persists and refuses an overnight window, idle accrues while paused, a non-working day reports zero idle with its work intact, and idle seals at `work_end` while work past it still records. The pass caught **two defects, both above the reducer and neither reachable by a Rust test** — see the 17:45 and 18:10 entries. One known gap is left open deliberately: the Idle figure is stale on screen until the next action, because it accrues exactly when the tick loop is parked. Recorded in *Known gaps* below rather than fixed, since it is a display concern and not a measurement one. |
| 2026-08-24 18:10 | **The tray's break item follows the state.** It read *Take a break* during a break, where clicking it did nothing — the reducer refused the event correctly and the menu said otherwise. Now *End break* during a break, and disabled at a work checkpoint. Found in the I6.5 pass. |
| 2026-08-24 17:45 | **I3.2 corrected.** The D16 park was hooked on `RunEvent::ExitRequested`; `Cmd+Q` does not emit it, so quitting with `Cmd+Q` left the block running. Moved to `RunEvent::Exit`. Found by Gigih in the I6.5 manual pass — the case no reducer test could have caught, since it lives entirely in the shell's event plumbing. |
| 2026-08-24 16:40 | **All six phases landed.** `cargo test` 96 passing, `cargo clippy --all-targets -- -D warnings` clean, `npm run typecheck` clean. Three deviations from the plan, each argued in `IDLE_TIME.md`'s changelog: **I1.5's `time_blocks.updated_at` was not added** and **I3.4 (D20's crash fallback) was not implemented** — an open span now survives an unclean exit and keeps accruing, because nothing writes to the database while a span is open, so "close at the last state write" would have erased the whole gap and broken tests 26 and 27; and **I2.4** keeps `away_ms` and the `AWAITING` span as two writers of one fact rather than folding one into the other (they are driven by the same transition, and test 25 asserts they agree). I6.5, the manual pass on the built app, is Gigih's and is still open. |
| 2026-08-24 12:05 | **Q1 and Q2 closed.** Break is accumulated regardless of how it was started (no new column, migration 003 is final on this point). A quit-parked span is tagged `Paused` — which collapses I3.1: quitting *is* a pause, so `Event::Pause` is dispatched on exit and no new event is needed. |
| 2026-08-24 11:47 | Initial version — 6 phases, 31 tasks, 2 open questions. |

---

## Progress

| Phase | Scope | Tasks | Status |
|---|---|---:|---|
| I1 | Working window — settings & schema | 5 / 5 | ✅ |
| I2 | Idle spans in the domain core | 6 / 6 | ✅ |
| I3 | Quitting parks the block (D16) | 4 / 5 | ✅ (I3.4 ✂ dropped, see below) |
| I4 | Summary — the measure itself | 5 / 5 | ✅ |
| I5 | Deliberate breaks (D22) | 5 / 5 | ✅ |
| I6 | Surfaces & verification | 5 / 5 | ✅ |
| | **Total** | **30 / 31** | |

**Status key:** ⬜ Not started · 🟡 In progress · ✅ Done · ⛔ Blocked · ⏸ Deferred · ✂ Dropped (with the reason in Notes)

**Definition of done for every phase:** `cargo test` clean · `cargo clippy --all-targets -- -D warnings` clean · `npm run typecheck` clean · the listed acceptance tests pass as reducer tests. Per Rule 16, the build and the manual pass are Gigih's, not the agent's.

**Sequencing rationale.** I1 and I2 are independent of each other and both are
prerequisites of I4, which is where a number first appears. I3 is deliberately
*after* I2: parking on quit is only correct once there is a span to write the
gap into, and shipping it earlier would change behaviour while measuring
nothing. I5 is separable and could ship first if the break gap is felt more
urgently than the report — it is the one phase with user-visible value on its own.

---

## Phase I1 — Working window: settings & schema

**Goal:** the window is stored, validated, and resolvable to a pair of instants for a given local day. No behaviour changes and nothing is measured yet.
**Exit criteria:** migration 003 applies to the live database; `window_for(day)` returns `None` on a non-working weekday and the correct pair otherwise.

| # | Task | Status | Notes |
|---|---|---|---|
| I1.1 | Migration `003_working_window.sql` — `work_start_minutes`, `work_end_minutes`, `working_weekdays`, `time_blocks.updated_at`, `idle_spans` table + indexes | ✅ | Landed. `time_blocks.updated_at` deliberately omitted — nothing reads it once I3.4 is dropped |
| I1.2 | `db/settings.rs` — read/write the three columns, converting minutes at the boundary | ✅ | Landed as `work_start_ms` / `work_end_ms` (ms from local midnight) and `working_weekdays` |
| I1.3 | Reject `work_start >= work_end` in `update_settings` | ✅ | `Settings::rejection()`, returned from `update_settings` as `AppError::Rejected`. `sanitized()` also falls back to the default if a row carries an overnight window anyway |
| I1.4 | `state::window_for(day_start) -> Option<(Millis, Millis)>` — weekday bitmask, local minutes → absolute instants | ✅ | Landed in `state.rs`. Built as a local wall-clock time through chrono rather than `day_start + offset`, so a DST day still starts at 09:00 by the wall clock |
| I1.5 | `db/repo.rs` — persist/load `idle_spans`, and stamp `time_blocks.updated_at` on every save | ✅ | Spans persist and reload, the open one still open. Closed spans are written before the open one so the partial unique index cannot trip |

---

## Phase I2 — Idle spans in the domain core

**Goal:** every instant the timer is not running is bracketed by a span, tagged with why. Pure, deterministic, no clock of its own.
**Exit criteria:** tests 24, 25, 32 pass as reducer tests.

| # | Task | Status | Notes |
|---|---|---|---|
| I2.1 | `core/model.rs` — `IdleSpan { id, started_at, ended_at, reason }`, `IdleReason { Awaiting, Paused, Untracked }` | ✅ | Landed, with `IdleReason::of(TimerState)` as the single state→reason mapping |
| I2.2 | `MachineState` — `open_idle: Option<IdleSpan>` + `idle_spans: Vec<IdleSpan>` | ✅ | Landed. Both fields are `#[serde(skip)]` — the UI needs the summary's numbers, not the list |
| I2.3 | `reduce` — close the open span on entering `Running`; open one tagged by the state entered on leaving it | ✅ | Landed as `sync_idle`, called once from the end of `reduce` whenever `timer_state` changed |
| I2.4 | Fold `settle_away` into the `Awaiting` span | ✅ | Kept as two writers of one fact rather than folded. Both fire only on a real transition, and test 25 asserts they agree |
| I2.5 | Hydrate — an open span reloads open; a span open across launch closes per D20 | ✅ | An open span reloads open and keeps accruing — see the D20 revision |
| I2.6 | Reducer tests for I2 | ✅ | `core::tests::idle`. Every test asserts the sum, not only test 32 |

> ⚠ **I2.4 is where this phase can go wrong.** `away_ms` and the `AWAITING` span
> record the same interval by two mechanisms. If both accumulate independently,
> the day double-counts every checkpoint, and the sum property (test 32) will not
> catch it — both halves would be equally wrong. Pick one writer.

---

## Phase I3 — Quitting parks the block (D16)

**Goal:** the clock stops when the app does, so a quit interval is idle and not work.
**Exit criteria:** tests 26, 27, 31 pass; `docs/SPEC.md` §6 and `CLAUDE.md`'s invariant list no longer contradict the code.

| # | Task | Status | Notes |
|---|---|---|---|
| I3.1 | Dispatch the existing `Event::Pause` on exit — no new event | ✅ | `commands::park_for_quit` dispatches `Event::Pause` |
| I3.2 | Wire it into `RunEvent::Exit` in `lib.rs`, before exit | ✅ | **Corrected 2026-08-24 17:45** — the plan said `ExitRequested`, which `Cmd+Q` never emits (it is `terminate:` → `applicationWillTerminate` → `Exit`). Caught in manual testing: the popover's Quit parked, `Cmd+Q` did not. The exit is never prevented; `dispatch` persists the whole state before it returns |
| I3.3 | Delete the D14 quit confirmation | ✅ | Deleted, with `request_quit`'s branch and the `quit-confirm` route |
| I3.4 | D20 crash fallback — an unclosed span closes at `time_blocks.updated_at` | ✂ | **Dropped.** Nothing writes to the database while a span is open (the ticker parks), so `updated_at` is almost always the span's own `started_at` — closing there erases the gap rather than its tail, and under D16 a clean quit has that exact shape. The span stays open instead |
| I3.5 | Amend `docs/SPEC.md` §6 and `CLAUDE.md` — replace the annotation with the new rule | ✅ | `SPEC.md` §6, D14, §7.4 and §11 amended; `CLAUDE.md`'s invariant list too |

---

## Phase I4 — Summary: the measure itself

**Goal:** `idle_ms` and its three causes, computed as a set difference and clipped to the window.
**Exit criteria:** tests 23, 28, 29, 30, 33 pass.

| # | Task | Status | Notes |
|---|---|---|---|
| I4.1 | `core::summary::summarize` takes `window: Option<(Millis, Millis)>` | ✅ | Landed |
| I4.2 | `covered(D)` — the union of intervals where any block was running | ✅ | Landed as the complement of the spans, which is what `covered` is once anything has ever run. A small `union` / `intersect` / `subtract` helper set does the arithmetic |
| I4.3 | `idle_ms` = `|window \ covered|`, attributed to the three sub-buckets | ✅ | Each bucket is its own interval intersection; the total is their sum, so a leak cannot hide |
| I4.4 | `outside_hours_ms`; D19 — a day with no blocks reports nothing | ✅ | `outside_hours_ms` excludes break intervals — it is about work |
| I4.5 | Summary tests | ✅ | Landed |

> The whole phase turns on I4.2. `window − worked − break` is the tempting
> one-liner and is wrong whenever a block runs past `work_end` or a paused block
> spans a gap — and it can go negative, which a duration cannot.

---

## Phase I5 — Deliberate breaks (D22)

**Goal:** a break can be started without waiting for a checkpoint, so rest is recordable and idle keeps a sharp meaning.
**Exit criteria:** tests 42–47 pass.

| # | Task | Status | Notes |
|---|---|---|---|
| I5.1 | `Event::StartBreak { ms }` + `Action::StartBreak` | ✅ | Landed |
| I5.2 | Mid-block: park the work block, task stays at the queue **head** | ✅ | Landed |
| I5.3 | Do not increment `interruptions` | ✅ | Landed |
| I5.4 | No-op at a work checkpoint and during a break | ✅ | Landed |
| I5.5 | Reducer tests | ✅ | Plus one for starting a break from IDLE and from PAUSED |

---

## Phase I6 — Surfaces & verification

**Goal:** the user can set the window, start a break, and see the number.
**Exit criteria:** a manual pass on the built app (Gigih), plus the full check suite clean.

| # | Task | Status | Notes |
|---|---|---|---|
| I6.1 | Settings — *Working hours* group: start, end, weekday picker | ✅ | A pair of `input type=time` fields plus a weekday bitmask picker |
| I6.2 | Today — replace the `Away` line with `Idle` | ✅ | The stat reads *Idle in working hours* — named as what it measures, per §2 |
| I6.3 | *Take a break* — popover control with a duration segmented control, plus the tray menu item | ✅ | The tray item is on the **right-click** menu; left click still opens the popover, which it would otherwise have swallowed |
| I6.4 | `Snapshot` carries the new fields; `src/core/format.ts` gains **no** logic | ✅ | No logic added to `format.ts` |
| I6.5 | Manual acceptance pass on the built app | ✅ | **Passed 2026-08-24** on `tauri:dev` (its database is separate from the installed build's — see `CLAUDE.md`). Ten cases: both quit doors; popover break; tray break in four states; working-hours persistence and the overnight refusal; paused idle; a non-working day; the `work_end` seal. Caught the `RunEvent::Exit` and tray-label defects |

---

## Acceptance test coverage

Every test in `IDLE_TIME.md` §7 is owned by a phase. 34–41 belong to `SLEEP_DETECTION.md` and are not in scope here.

| Test | Subject | Phase | Status |
|---|---|---|---|
| 23 | Late first block → untracked idle | I4 | ✅ |
| 24 | Pause → `idle_paused_ms` | I2 | ✅ |
| 25 | Checkpoint → `idle_awaiting_ms` | I2 | ✅ |
| 26 | Quit mid-block parks and does not credit work; the gap is `idle_paused_ms` | I3 | ✅ |
| 27 | Idle stops at `work_end`, not at reopen | I3, I4 | ✅ |
| 28 | Work after `work_end` counts; idle does not | I4 | ✅ |
| 29 | Non-working day: work recorded, zero idle | I4 | ✅ |
| 30 | No blocks → no day | I4 | ✅ |
| 31 | An open span survives an unclean exit and keeps accruing *(rewritten; D20 revised)* | I2 | ✅ |
| 32 | Sub-buckets sum to `idle_ms` (property) | I2 | ✅ |
| 33 | Set difference, not subtraction | I4 | ✅ |
| 42 | `StartBreak` parks, keeps queue head | I5 | ✅ |
| 43 | `StartBreak` does not count as an interruption | I5 | ✅ |
| 44 | No-op at a work checkpoint | I5 | ✅ |
| 45 | No-op during a break | I5 | ✅ |
| 46 | A deliberate break is break, not idle | I4, I5 | ✅ |
| 47 | A break does not consume capacity | I5 | ✅ |

Tests 26, 33 and 43 are the ones that encode why this feature exists. 33 in
particular fails only if `idle` was implemented as arithmetic instead of as a
set difference — the mistake that is easiest to make and hardest to see.

---

## Known gaps

Left open deliberately at the close of I6.5. None is a measurement error — the
stored numbers are correct in every case below.

| # | Gap | Why it is acceptable for now |
|---|---|---|
| ~~G1~~ | ~~The Idle figure is stale on screen until the next action.~~ **Fixed 2026-08-24 19:10.** Both halves: `RunEvent::WindowEvent { Focused(true) }` emits `timebox://changed`, so a window coming forward refetches; and `useTimebox.init` polls every 10s while `timerState !== "Running"`, covering a window left open. | — |
| G2 | **A crash or force-quit while `RUNNING`** still credits the gap as work, capped at the block's allocation. No park is written, and no span exists to keep open. | The bounded, known over-report D20's revision names explicitly. D21 narrows the common cause (sleep); a genuine crash mid-block is rare and capped. |
| G3 | **The three idle causes and `outside_hours_ms` have no UI.** | They are not peers of the total, so showing them beside it would misrepresent them (§6). They belong to the daily report, a separate unbuilt feature. Carried by tests 23–33 in the meantime. |

---

## Open questions

| # | Question | Needed by | Status |
|---|---|---|---|
| Q1 | ~~Distinguish a deliberate break from a checkpoint break?~~ — **Closed 2026-08-24: no.** Break time accumulates regardless of the door it came through. No column, and migration 003 is settled on this point. | I1 | ✅ Closed |
| Q2 | ~~Tag a quit-parked span `Untracked` or `Paused`?~~ — **Closed 2026-08-24: `Paused`.** The block ends `PAUSED`, so the span matches it. Collapses I3.1 to dispatching the existing `Event::Pause`. | I3 | ✅ Closed |
