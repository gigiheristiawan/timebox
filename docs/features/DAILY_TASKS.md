# Daily tasks

Some work comes back every day — standup, inbox, the exercise you keep not
doing. Under the MVP rules those tasks could only be added once and completed
once: completing a task sets it `Done` and removes it from the queue, so a
recurring one had to be re-typed each morning or never ticked off at all.

This spec adds a **daily** task: one that is never `Done` and never leaves the
queue. Completing it means *done for today*, and tomorrow it is simply
outstanding again.

Extends `docs/SPEC.md`. Decisions continue its numbering from D22
(`IDLE_TIME.md`); acceptance tests continue from 53.

Status: **implemented** (issue
[#16](https://github.com/gigiheristiawan/timebox/issues/16)).

---

## Changelog

| Date (WIB)       | Change                                                                                  |
| ---------------- | --------------------------------------------------------------------------------------- |
| 2026-09-04 10:55 | Initial version. D23–D26; migration 005; acceptance tests 54–60.                          |

---

## 1. The premise

The MVP's task lifecycle is one-way: `Todo → InProgress → Done`, and `Done`
leaves the queue. That is right for a task that is *finished*. A daily task is
never finished — only current — so it needs a lifecycle that returns.

The one thing this must not become is a scheduler. There are no weekly tasks,
no "every second Tuesday", no due dates and no catch-up for a day you missed.
A daily is outstanding today or it is not.

---

## 2. Decisions

| #   | Decision | Reasoning |
| --- | -------- | --------- |
| D23 | **A daily task is never `Done` and never leaves the queue.** Completing one stamps `completed_at` and leaves `status` at `Todo`. It is removed only by an explicit *Remove* (`Action::RemoveTask`), exactly as the issue asks. | The queue is the product — a playlist for the day. A recurring task that vanished on completion would have to be re-added every morning, which is the friction the feature exists to remove. Leaving it visible and ticked is also the record of having done it: the queue doubles as the day's checklist. |
| D24 | **"Done today" is derived, not stored.** `Task::done_today(day_start)` is `daily && completed_at >= day_start`. There is no reset event, no midnight timer, and no stored per-day flag. | A stored flag needs something to fire at midnight, and the app is routinely not running then — quit for the night, asleep, or on a machine that was off for a week. Every one of those cases is a chance to miss the reset and start the day with yesterday's ticks still showing. Derivation cannot miss: the comparison is re-evaluated on each snapshot, so the day turning over needs no code at all. It is the same argument that made `hydrate` feed a single `Tick` rather than grow a recovery path per failure mode. |
| D25 | **Done today means *inert*, not merely deprioritised.** Rotation steps over it (`start_next` picks the first *startable* task, not the queue head) and an explicit `SwitchTo` is refused. | One rule rather than two. If rotation skipped it but a click still started it, the tick would be a suggestion about the day rather than a fact about it, and the same task could be completed twice in one day — which would then be counted twice in Today. |
| D26 | **Completing a daily discards any parked block for it**, so tomorrow starts from a fresh allocation. | SPEC D10 parks a block so that switching away and back cannot re-grant time. That rule bounds a task *within a run of work*; it has nothing to say about a day that has been closed out. Resuming yesterday's 20-minute remainder tomorrow would be the wrong end of the anti-gaming rule — punishing the user for having finished early rather than stopping them farming time. |

### 2.1 Position in the queue

Completing a daily does **not** rotate it to the back, which is what `Skip` and
`Pending` do (SPEC D2/D10). It keeps its place, so a queue the user has dragged
into an order stays in that order day after day.

It cannot literally hold index 0, because the queue head is by construction
whatever is running — starting the next task brings that task to the front. What
is guaranteed is that completion itself moves nothing.

### 2.2 What is deliberately absent

- **No other recurrence.** Daily is a `bool`, not a `Recurrence` enum. Weekly,
  weekday-only and every-*n*-days are all defensible and none was asked for;
  adding the enum now would be an abstraction for a single case (Rule 2).
- **No streaks, no history.** `completed_at` holds the *last* completion, not a
  log of them. A per-day record is a different feature — reporting — and
  `work_spans` already covers the time question.
- **No catch-up.** A daily missed yesterday is not owed today. It is simply
  outstanding, which it would have been anyway.

---

## 3. Extending a daily

Both grants work on a daily exactly as they do on any other task, and they mean
different things — which is the answer to "this one usually takes longer" versus
"this one ran long today":

| Control | Effect | Lifetime |
| ------- | ------ | -------- |
| `AddTime` (`+5/+10/+15` in the task editor) | Raises `block_duration_ms` | **Permanent.** A daily bumped to 45m starts at 45m every day after. |
| `DecideExtend` (Extend at the checkpoint) | Raises the live block's `extension_ms` | **Today only.** The block is discarded with the day (D26); tomorrow returns to the task's allocation. |

Neither needed special-casing.

---

## 4. Data model

`Task` gains one field:

```rust
pub daily: bool,
```

and `completed_at` changes meaning for a daily only: it is the *last*
completion rather than *the* completion.

### 4.1 Migration 005

```sql
ALTER TABLE tasks ADD COLUMN daily INTEGER NOT NULL DEFAULT 0;
```

Forward-only, following 001–004. Existing tasks are ordinary ones; the default
is the whole migration for them.

### 4.2 `reduce` is handed local midnight

```rust
pub fn reduce(state, event, now, day_start, ids) -> (MachineState, Vec<Effect>)
```

`day_start` is injected for the same reason `core::summary` is handed it: a
timezone is a shell concern, and the core stays pure (SPEC R6). It is resolved
by `state::day_start_ms` at the two dispatch points — `App::hydrate` and
`App::dispatch`.

### 4.3 Turning recurrence off

`EditTask` carries `daily` too. Un-marking a task that was already ticked today
clears `completed_at`, because a `Todo` task carrying a completion stamp is
neither done nor cleanly outstanding — it would vanish from *both* columns of
Today.

---

## 5. Surfaces

- **Add task** and the **task editor** carry a `Daily` checkbox.
- **Up next** shows a `daily` chip on every daily row. A row done for today is
  dimmed and struck through, its action reads `✓ today`, and click, `Enter` and
  drag-to-start are all inert — the backend refuses the switch either way, so
  the row says so rather than looking live and doing nothing.
- **Popover** lists it the same way: still present, ticked, disabled. Seeing the
  dailies you have already done is the point of keeping them in the queue.
- **Rotation** omits it. The strip is what is *left* to do.

### 5.1 `Snapshot.doneToday`

The set of task ids done for today is computed in Rust and carried on the
snapshot, beside `summary`. The UI never compares `completedAt` against a date
of its own — SPEC R7.

---

## 6. Today's numbers

A daily counts in `tasks_completed` exactly like any other completion — ticking
one off is a thing you did today — and correspondingly stops counting in
`tasks_pending` until tomorrow. Counting it in both columns at once is the
failure test 59 pins.

Worked time, idle time and capacity need no change: a daily's blocks are
ordinary work blocks and were already measured by `work_spans`.

---

## 7. Acceptance tests

Continuing SPEC's numbering. All in `src-tauri/src/core/tests.rs` (`mod daily`)
except 61, which needs persistence and lives in `src/state/tests.rs`.

| #   | Test |
| --- | ---- |
| 54  | Completing a daily leaves it in the queue with `status = Todo` and a `completed_at` stamp; an ordinary task completed the same way is `Done` and gone (D23). |
| 55  | Rotation steps over the daily it has just ticked off, and does not rotate it to the tail — the order is preserved (D25, §2.1). |
| 56  | `SwitchTo` a daily already done today is refused; nothing moves (D25). |
| 57  | The same state, evaluated against the next day's `day_start`, makes the task startable again — with no reset event fired (D24). |
| 58  | Completing a daily discards its parked block, and the next day starts on the task's full allocation rather than yesterday's remainder (D26). |
| 59  | A daily counts once in `tasks_completed` and not in `tasks_pending`; tomorrow the two swap back (§6). |
| 60  | Un-marking a daily completed earlier today clears `completed_at`, and it is outstanding again (§4.3). |
| 61  | `daily` and `completed_at` survive a quit and relaunch; the task comes back queued, `Todo`, and still done for that day (§4.1). |

The wire encoding of `daily` on both `addTask` and `editTask` is pinned in
`commands::tests`, for the reason the priority test exists: a wrong field name
does not fail, it silently makes every task ordinary.

---

## 8. Known limits

- **A daily is done for a *day*, not for a rolling 24 hours.** Ticking one off
  at 23:55 leaves it outstanding again five minutes later. That is the correct
  reading of "daily" and the same convention Today already uses, but it is worth
  stating.
- **A day's boundary is the machine's current timezone**, resolved fresh on
  every snapshot. Flying across timezones can therefore make a task that was
  done today outstanding again, or the reverse. `day_start_ms` has always
  behaved this way; dailies are the first place it is visible.
