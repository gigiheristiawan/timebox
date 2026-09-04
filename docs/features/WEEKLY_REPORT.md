# Weekly Report (issue #6)

A read-only **Report** tab in the main window: seven day rows for one week,
week totals against the capacity target, the week's top tasks, and the idle /
break / outside-hours breakdown — with prev/next week navigation.

Extends `docs/SPEC.md`. Decisions continue at **D34**, acceptance tests at
**82**. Status: **implemented** (`core/report.rs`, `components/Report.tsx`).

---

## Changelog

| Date (WIB)       | Change                                                                 |
| ---------------- | ---------------------------------------------------------------------- |
| 2026-09-04 23:10 | Audit pass. Two latent bugs in `summarize` found that only bite once a day other than today is queried — unbounded upper edges on the pre-004 span fallback and on `tasks_completed` — now **D44**, with tests 97/98. `away_ms` dropped from the week (a lifetime counter; `idle_awaiting_ms` measures the same thing correctly) and `switched_early` redefined over blocks that *ended* in the week, since `interruptions` carries no timestamp — D35, test 99. New **D45**: settings are unversioned, so changing them re-scores history. New **D39a**: a non-working day reports no idle, by inheritance. Field name corrected to `available_work_ms_per_day`; "no arithmetic in TS" softened to "no rules in TS", since a bar is a ratio of two supplied numbers. |
| 2026-09-04 23:07 | **Implemented.** `core/report.rs`, `state::week_start_ms` / `week_context`, `get_report`, the Focus/Report tab and `components/Report.tsx`. One thing the spec had wrong: **D46** — a daily task stores only its *last* `completed_at`, so a standup ticked Mon/Tue/Wed reports **one** completion for the week, not three. Test 96 rewritten to pin that and to assert the three blocks *are* all reported. Test 92 (the `offset > 0` refusal) is not unit-covered: it is a three-line guard inside a `#[tauri::command]` and reaching it needs a Tauri `State`. |
| 2026-09-04 22:44 | Initial version. Requirements settled with Gigih: main-window tab, Monday-start weeks with prev/next, worked + top tasks + idle/breaks + counts, target from working weekdays, full-week target even for the current week, all seven days always shown, on-demand `get_report`, no export. |

---

## 1. What this is, and what it is not

The report **describes** time already recorded. It adds no `Event`, no
`Effect`, and no new column: every number it shows is a pure function of
`MachineState` plus the calendar. Nothing in the report can change the timer,
the queue, or a task.

It is not a dashboard, not a chart library, and not an export. §9 lists what
was deliberately left out.

---

## 2. The shape of the data

New module **`core/report.rs`**, beside `core/summary.rs` and obeying the same
rule: no clock, no timezone, no I/O. The calendar is *injected*.

```rust
/// One day, resolved by the shell. `state::week_context` builds seven of these.
pub struct DayCtx {
    pub day_start: Millis,   // local midnight
    pub day_end: Millis,     // the next local midnight (state::day_end_ms)
    pub window: Option<Iv>,  // state::window_for — None on a non-working weekday
    pub available_ms: Millis // settings.available_work_ms_per_day
}

pub struct DayReport {
    pub day_start: Millis,
    pub weekday: u8,          // 0 = Monday … 6 = Sunday
    pub working_day: bool,    // window.is_some()
    pub target_ms: Millis,    // available_ms on a working day, else 0
    pub worked_ms: Millis,
    pub break_ms: Millis,
    pub idle_ms: Millis,
    pub idle_awaiting_ms: Millis,
    pub idle_paused_ms: Millis,
    pub idle_untracked_ms: Millis,
    pub outside_hours_ms: Millis,
    pub tasks_completed: usize,
    pub blocks_completed: usize,
}

pub struct WeekTotals {
    // additive over the seven days
    pub worked_ms: Millis,
    pub break_ms: Millis,
    pub idle_ms: Millis,
    pub idle_awaiting_ms: Millis,
    pub idle_paused_ms: Millis,
    pub idle_untracked_ms: Millis,
    pub outside_hours_ms: Millis,
    pub tasks_completed: usize,
    pub blocks_completed: usize,
    // not a sum of the days — see D35
    pub switched_early: u32,
    // the baseline
    pub target_ms: Millis,
    pub working_days: usize,
    pub days_worked: usize,   // days with worked_ms > 0
}

pub struct WeekReport {
    pub week_start: Millis,   // Monday, local midnight
    pub week_end: Millis,     // the following Monday, exclusive
    pub offset: i32,          // 0 = current week, -1 = last week, …
    pub is_current_week: bool,
    pub days: Vec<DayReport>, // always exactly 7, Monday first
    pub totals: WeekTotals,
    pub top: Vec<TopTask>,    // TOP_N over the whole week (D35)
}

pub fn report(state: &MachineState, days: &[DayCtx], offset: i32, now: Millis) -> WeekReport;
```

`TopTask` is reused from `core::summary` unchanged.

---

## 3. Decisions

### D34 — A day in the report is the same day `summarize` already defines

`DayReport` is filled by the existing per-day arithmetic, not by a second
implementation of it. `summarize`'s day-scoped work — `spans_by_block`,
`spent_in_day`, `idle_intervals ∩ window`, the `blocks_completed` end-day
bucketing, the `completed_at >= day_start` completion count — is factored into
a shared private routine that both `summary::summarize` and `report::report`
call.

**Why:** issue #11 (worked time bucketed by `started_at`) and issue #16
(done-today derived, not stored) were each one subtle rule about what a day
*is*. A report that re-derived them would drift from the Today strip, and the
first symptom would be two screens in the same app disagreeing about
yesterday. `summarize` keeps its signature; only its internals move.

The refactor is the only change this feature makes to existing behaviour, and
it must be behaviour-preserving: tests 1–81 pass untouched.

### D35 — The week is the sum of its days, *except* for the per-block metrics

Worked, break, all three idle causes, outside-hours, tasks completed and
blocks completed are **sums of the seven `DayReport`s**. They are safe to add:
worked/break/idle are measured from spans that partition the timeline, and
completions and block-ends are each bucketed into exactly one day.

`switched_early` is **not** summed, and `away_ms` is **not reported at all**.
Both are properties of a *block* rather than of an interval, and a block
outlives a day — it can be parked across midnight, and across weeks.

- `b.interruptions` is a **lifetime** counter with no per-switch timestamp, so
  no amount of interval algebra can say how many of a block's switches
  happened in a given week. The week therefore defines churn over the blocks
  it can attribute exactly once: **`switched_early` sums `interruptions` over
  the work blocks that *ended* within `[week_start, week_end)`** — the same
  end-day bucketing `blocks_completed` already uses. A block still parked
  contributes nothing until the week it finishes in. `DayReport` carries no
  `switched_early`: it is a week-level figure only.
- `away_ms` is likewise lifetime (`b.away_ms` plus the open wait), so a block
  parked for three weeks would report three weeks of waiting into whichever
  week is being viewed. Rather than ship a number that is wrong in a way the
  reader cannot see, the report omits it. Nothing is lost:
  `idle_awaiting_ms` measures the same waiting from `idle_spans`, is
  interval-based, and is correctly week-scoped.

`top` is likewise not a merge of the daily top-3s: a task that placed fourth
every day would vanish from a list it belongs at the head of. The week ranks
the full per-task map over the whole week, then takes `TOP_N`.

**Cost, stated:** a day row's `away_ms` can therefore exceed its share, and the
day rows' `switched_early` is not shown per-day at all — only the week's. That
is why `DayReport` carries neither field.

### D36 — The target is `capacity × working weekdays`, and the current week is measured against the whole week

`target_ms` for a day is `available_work_ms_per_day` when
`settings.working_weekdays` has that day's bit set, and **0** otherwise. The
week's target is the sum, i.e. capacity × the number of working weekdays in
the week.

A non-working day's target is a real zero, not an absent one. Work done on it
is **over target**, and is shown as such — the same stance
`outside_hours_ms` already takes on work outside the working window
(IDLE_TIME §3.2): it is recorded and reported, never suppressed and never
subtracted. There is no "unmeasured day" case in the report; every day has a
target and every day is measured against it.

The one mechanical consequence: a percentage against a zero target is
undefined, so a zero-target day renders as **over** when it has work and `—`
when it does not. That is a rendering rule, not a second kind of target — the
alternative considered and rejected was giving non-working days a token 1h
target to keep the division defined, which would assert a goal the user never
set (a rested Saturday would read 0% of it) and would inflate the week's
baseline to `capacity × 5 + 2h`, so a perfect Mon–Fri week could never reach
100%.

The current, incomplete week is measured against the **full** week's target,
not a week-to-date one. Gigih's call: the number answers "how much of this
week's plan is done", and a Tuesday reading 40% is the honest answer to that
question. A week-to-date target would read 100% on a Tuesday and then fall,
which is a worse signal than a number that only climbs.

**Attainment is reported, never enforced.** Over 100% is shown as over, the
same way the capacity strip shows over-allocation (SPEC §7.3).

### D37 — The UI asks for a week by *offset*; it never computes a date

The command takes `offset: i32` (`0` = the week containing now, `-1` = the
week before) and Rust resolves the Monday. The UI receives `week_start` /
`week_end` as instants and formats them.

**Why:** the same rule that put `Snapshot.doneToday` on the wire as ids
(DAILY_TASKS) and that keeps `day_start_ms` in the shell. "Which Monday" needs
a timezone and DST-correct calendar arithmetic; `src/core/format.ts` holds
three formatters and no rules, and this does not change that.

New `state::week_start_ms(now, offset)`: local midnight of the Monday of
`now`'s week, then `offset` weeks stepped as **calendar days on the local
date** (`chrono::Duration::days(7)` on `NaiveDate`, re-resolved to local
midnight), not `± 7 × 86_400_000` — the arithmetic that a DST week would put
an hour out. `state::week_context(week_start, settings)` then builds the seven
`DayCtx` with `day_end_ms` and `window_for` per day, so a DST day's window
still starts at 09:00 on the wall clock.

### D38 — On demand, not on the snapshot

New command **`get_report(offset: i32) -> WeekReport`**, beside
`get_snapshot`. `Snapshot` is unchanged.

**Why:** the snapshot is rebuilt every second by the tick loop and read by the
tray and both windows. Folding a week of interval algebra into it would
recompute seven days of spans once a second for a tab that is usually not
open. `get_report` runs only when the Report tab is open and something asked.

### D39a — A non-working day reports no idle, by inheritance

`window_for` returns `None` for a day outside `working_weekdays`, and
`summarize` computes idle only inside the window (D17/D18: outside it, no
claim of presence was made). A Saturday therefore reports `idle_ms == 0`
however little was done, while still reporting its `worked_ms` and its
`outside_hours_ms`.

That is the existing semantics, restated here only because a column of zeros
down the weekend rows looks like a defect and is not one.

### D39 — All seven days, always

Every week renders seven rows including zeros, including weeks entirely before
the app was installed. A zero day is information — a day off, a day missed —
and a grid whose shape changes with the data is harder to read across weeks
than one that does not.

### D40 — Only the current week follows `timebox://changed`

The tab refetches on `timebox://changed` (and on the 10s idle poll) **only
while `offset == 0`**. A past week is settled history; the timer cannot change
it.

The one thing that can still change a past week is a task *title* — the report
resolves `top` titles live, so a rename shows on the next visit. That is
correct and needs no invalidation.

### D41 — Back is unbounded, forward stops at the current week

`offset > 0` is refused by the command (`AppError`), so "next" is disabled at
`offset == 0`; the app makes no claim about a week that has not happened.
Going back has no floor: a week before any data is a legal, all-zero week
(D39), and finding the earliest block just to grey out a button is not worth a
scan.

### D42 — Blocks written before migration 004 keep their inherited attribution

Blocks with no `work_spans` rows fall back to `started_at` bucketing, as
`summarize` does today. The report inherits the limitation rather than
inventing a different answer for old data; a week made only of pre-004 blocks
reports their whole allocation on the day each started — bounded by D44.

### D44 — Every day filter is bounded at **both** ends

`summarize` is only ever asked about *today*, and two of its filters exploit
that by testing the lower bound alone:

- `spent_in_day`'s span-less fallback, and the `today` block filter behind it,
  accept any block with `started_at >= day_start`;
- `tasks_completed` counts any task with `completed_at >= day_start`.

Nothing can start or complete after *now*, so for today both are sound. Asked
about **any earlier day**, both are wrong in the same direction: a pre-004
block started this morning would be attributed to *every* past day in the
report, and every task completed since Monday would be counted on Monday, and
again on Tuesday, and so on.

The shared routine D34 factors out therefore bounds both filters at
`day_end` as well. This is **behaviour-preserving for `summarize`** — for
today the upper bound is vacuous — and load-bearing for the report. It is the
one place where the refactor is not purely mechanical, and tests 97 and 98
exist to pin it.

### D46 — A daily's repeated ticks are not recoverable; its work is

`Task.completed_at` holds the **last** time a task was ticked off, and a daily
has no history beyond it. A standup done Monday, Tuesday and Wednesday leaves
one timestamp, so the week reports **one** completion — on Wednesday.

Today's strip is unaffected: the last tick of a task done today *is* today's.
It is only the week that can ask about a day whose tick has since been
overwritten.

The same shape as `interruptions` (D35): a counter with no per-occurrence
record. Unlike `interruptions` there is no repair available at all — one field
cannot hold five dates — so the report states less rather than guessing.

**What is not lost:** the *work* is. Each day's block is banked in
`work_spans` and reported on its own day, and `blocks_completed` counts each
one. So a week of standups shows three blocks, three days of worked time, and
one completion. Fixing the count would mean recording completions as events —
a table and a migration, for a number that `blocks_completed` largely already
answers.

### D45 — Settings are read as they are *now*, and history is re-scored when they change

`target_ms`, the working window, and therefore `idle_ms` and
`outside_hours_ms` for a past week are all computed from **current**
settings. There is no per-day snapshot of what the settings were at the time;
the schema has never stored one.

So raising capacity from 4h to 6h retroactively lowers last month's
attainment, and turning Saturday on retroactively gives every past Saturday a
target and an idle figure.

**Accepted, not fixed.** Versioning settings means a new table, a migration,
and a rule for which version a block that spans a change belongs to — a large
cost for a report whose purpose is "how am I doing lately". The report is a
view of the present's expectations applied to the past's record, and reads
correctly under that description.

### D43 — The report reads memory, not SQL

`report` is a pure function of the already-hydrated `MachineState`, which
holds every block and span. No query, no migration, no new table — and so the
whole feature is testable in `cargo test` with no database.

**Known limit, deliberately accepted:** `MachineState` grows without bound and
is loaded whole at startup. That is true today, before this feature, and the
report does not make it worse. If a retention or paging story is ever needed
it is a separate change (and the moment the report is asked to reach further
back than memory holds, it becomes one).

---

## 4. The command surface

```rust
#[tauri::command]
pub fn get_report(app: State<'_, Arc<App>>, offset: i32) -> AppResult<WeekReport>;
```

- refuses `offset > 0` (D41);
- resolves the week with `state::week_start_ms` / `state::week_context` against
  the app's cached settings;
- calls `core::report::report` with `now_ms()`.

TypeScript gets `getReport(offset)` in `src/ipc/commands.ts` and the matching
types. It is a *read*: it emits no `timebox://changed` and takes no lock the
tick loop needs.

---

## 5. The UI

A two-tab switcher at the top of the main window — **Focus** (everything the
window shows today) and **Report** — rendered by `App.tsx`, with the report
body in a new `components/Report.tsx`.

**Header.** `‹ Sep 1 – Sep 7 ›` plus a *This week* control when `offset != 0`.
`‹` / `›` are also `←` / `→` while the Report tab is open; `›` is disabled at
`offset == 0`.

**Week summary.** Worked against target — `12h 20m of 20h · 62%` — plus tasks
completed, blocks completed, and switched-early for the week.

**Seven day rows.** Weekday, date, worked, a bar against that day's target,
and the day's breaks / idle / outside-hours. A day whose target is 0 and which
has work on it reads as over, with no percentage (D36). A zero day shows `—`,
not `0m`.

**Top tasks.** Up to three, with time and share of the week's worked total.

**Idle breakdown.** The week's `idle_awaiting` / `idle_paused` /
`idle_untracked` and `outside_hours`, with the same labels the Today strip
uses so the two read as one vocabulary.

Rules the tab must obey:

- **The Report tab is a view, not a mode.** The timer keeps running, the
  checkpoint still takes the window when it opens, and the existing keyboard
  shortcuts stay live. `←`/`→` are the only keys the tab adds.
- **A checkpoint outranks the tab.** `PendingDecision` renders above the tab
  switcher, unchanged — the checkpoint has no exit, and a report is not one.
- **No rules in TypeScript.** Every figure, total and ranking arrives decided;
  `Report.tsx` formats with `durStr` and takes the ratio of two numbers the
  backend already supplied (worked/target for a bar, task/total for a share).
  A ratio drawn on screen is rendering. Deciding *which* tasks are top, what a
  day's worked time is, or whether a day is over target, is not — none of that
  may be computed here (SPEC R6/R7).

---

## 6. Acceptance tests

Rust, in `core::report::tests`, named to match (`t82_…`).

| #  | Test |
| -- | ---- |
| 82 | A day in `WeekReport` equals `summarize`'s `Today` for the same day, for worked, break, all three idle causes, outside-hours, tasks completed and blocks completed. (D34 — the anti-drift test; it fails if either implementation moves.) |
| 83 | A block worked 23:40 Mon → 00:20 Tue puts 20m on Monday and 20m on Tuesday, and the week's `worked_ms` is 40m. (D35, additive) |
| 84 | The same block's `interruptions` are counted **once** in the week's `switched_early`, not once per day. (D35, non-additive — mutation-checked against a naive day-sum) |
| 85 | A task that is fourth-largest on every day but second-largest over the week appears in the week's `top`. (D35, top is not a merge of tops) |
| 86 | With `working_weekdays = Mon–Fri` and capacity 4h, `totals.target_ms` is 20h and Saturday's `target_ms` is 0 while its `worked_ms` is whatever ran — i.e. Saturday is over target, not exempt from one. (D36) |
| 87 | Asking for the current week on a Tuesday still returns the full week's target, not two days of it. (D36) |
| 88 | A week with no blocks at all returns seven rows, all zero, `idle_ms == 0`, and does not panic. (D39 + D19: no blocks, no idle) |
| 89 | `days.len() == 7`, `days[0].weekday == 0`, and `days[i].day_start` is strictly increasing. (D39) |
| 90 | `week_start_ms(now, 0)` is local Monday midnight; `offset = -1` is exactly seven calendar days earlier, and stays midnight **across a DST transition**. (D37) |
| 91 | `week_end` equals `week_start_ms(now, offset + 1)` — the weeks tile with no gap and no overlap. (D37) |
| 92 | `get_report` with `offset > 0` returns an error and no report. (D41) |
| 93 | A block with no `work_spans` rows is attributed to the day it started, matching `summarize`. (D42) |
| 94 | The three idle causes in `totals` sum to `totals.idle_ms` exactly — the week-level restatement of test 32. |
| 95 | A running block at `now` contributes only up to `now`, and the days after today in the current week are all zero. (No forward spill.) |
| 96 | A daily ticked off on Mon, Tue and Wed reports **one** completion for the week, on Wednesday — and all three of its blocks. (D46) |
| 97 | A **span-less** (pre-004) block started *after* the week being reported contributes 0 to every day of it. Fails before D44's upper bound; the whole week would otherwise read the block's allocation on all seven days. |
| 98 | A task completed on Wednesday counts on Wednesday only — not on Monday and Tuesday as well. (D44) |
| 99 | A work block interrupted twice last week and once this week, finishing this week, contributes 3 to *this* week's `switched_early` and 0 to last week's; while it is still parked it contributes to neither. (D35 — the definition, and the absence of a lifetime leak) |
| 100 | A Saturday with work on it and `working_weekdays = Mon–Fri` reports `worked_ms > 0`, `idle_ms == 0`, `outside_hours_ms == worked_ms`, `target_ms == 0`. (D39a + D36) |
| 101 | Changing `available_work_ms_per_day` changes a *past* week's `totals.target_ms` on the next call. (D45 — pinned as intended, so it is not later "fixed" by accident) |

`npm run typecheck` covers the wire types; there is no TS logic to test.

---

## 7. Implementation order

1. Factor the per-day arithmetic out of `summary::summarize` — **no behaviour
   change**, tests 1–81 green. (D34)
2. `state::week_start_ms` + `state::week_context`, with test 90/91.
3. `core/report.rs` and tests 82–101.
4. `get_report` command — registered in `generate_handler!` in `lib.rs`,
   refusing `offset > 0` with `AppError::Rejected`, reading state through
   `app.snapshot()` so the machine lock is held only for the clone — plus the
   `src/ipc/commands.ts` binding and types.
5. `components/Report.tsx` and the tab switcher in `App.tsx`.
6. Update `CLAUDE.md` (layer map + this doc in the Docs list) and
   `docs/IMPLEMENTATION_PLAN.md`.

Branch: `feature/6-weekly-report` (Rule 18).

---

## 8. What this does not do

Named so they are not re-proposed as oversights:

- **No export.** No CSV, no clipboard, no file dialog — asked and declined.
  Adding one later is a button and a formatter; nothing here forecloses it.
- **No monthly or arbitrary range.** Week navigation only.
- **No charts beyond a bar per day.** No library.
- **No per-day `switched_early` or `away_ms`.** See D35 — they are not
  day-shaped.
- **No retention or paging.** D43.
