# Pomodoro mode

TimeBox's timer answers *how long do I stay on this task*. It has nothing to
say about *how long have I been at the desk without standing up* — a task
allocated 90 minutes runs 90 minutes, and the app never interrupts.

This spec adds **Pomodoro mode**: a toggle that, while on, puts a second clock
beside the task clock and offers a break every 25 minutes of actual work,
whatever the task is doing. The two clocks are independent. The task timer keeps
its allocation, its checkpoint and its decisions unchanged; the Pomodoro timer
only ever asks one question — *break, or keep going?*

Extends `docs/SPEC.md`. Decisions continue its numbering from D26
(`DAILY_TASKS.md`); acceptance tests continue from 61.

Status: **implemented** (issue
[#15](https://github.com/gigiheristiawan/timebox/issues/15)).

---

## Changelog

| Date (WIB)       | Change                                                             |
| ---------------- | ------------------------------------------------------------------ |
| 2026-09-04 22:26 | The Pomodoro countdown is **hidden during a break**. "Break in 24:40" while a break runs is redundant, and the number was stale as well as redundant: the clock resets when the break *ends* (D29), so it showed the pre-break value counting down to a break already being taken. §5 updated. |
| 2026-09-04 22:14 | The popover's Pomodoro toggle was **missed** in the first implementation — D33 asks for Settings *and* a tray/popover quick toggle, and only Settings and the tray were built. Added as a switch under the break row, disabled at either checkpoint. |
| 2026-09-04 21:53 | **Implemented.** Two deviations from the spec above, both recorded in §9. (1) `settings.pomodoro_mode` was dropped: `pomodoro_since` is `NULL` exactly when the mode is off, so it *is* the flag — one store rather than two to keep in step, which is §4.6's own objection applied to the storage as well as the write. (2) Migration 006 **rebuilds `app_state`**: 001 put a `CHECK (timer_state IN (…))` on the column, SQLite cannot alter a CHECK in place, and without the rebuild every write made while the prompt is open is rejected at *runtime*. Two further escape hatches found during implementation, neither in the §4.7 audit: `switch_to` (a positive `at_work_checkpoint` test used as a **refusal**, so false means allowed) and the popover's `canBreak`. Tests 62–81 pass. |
| 2026-09-04 16:52 | **Second audit pass — the shell, not the core.** Four more gaps (§4.7): `Checkpoint.tsx` gates on `timerState === "AwaitingDecision"` and would render a **blank undismissable window**; `Event::StartBreak` guards on `!at_work_checkpoint`, a *negation*, so the tray's break item is a live side door out of the Pomodoro prompt; `Skip` and `CompleteCurrentTask` carry no checkpoint guard at all; `menubar.rs` needs a fourth string. Corrects the §4.5 claim that `at_work_checkpoint` needs no caller changes — true for the four `Decide*` guards, false for `StartBreak`. Tests 79–81. |
| 2026-09-04 16:30 | **Audit against the code.** Nine gaps closed. The two that mattered: a Pomodoro checkpoint was indistinguishable from a work checkpoint in state, so every `Decide*` guard would have fired on it, and nothing persisted it across a quit — both fixed by a fourth `TimerState::AwaitingPomodoro` (§4.5), which supersedes the `CheckpointKind`-on-the-effect design. Also: §3.2's reset points corrected (a break *expiring* is not a break *ending*), §4.3's table name (`app_state`, not `state`), the toggle moved onto `reduce` as an event (§4.6), `staleness_ms` (§6.1), `RemoveTask` (§6.2), and the crash caveat (§8). |
| 2026-09-04 16:05 | §6.1 added: what an ignored Pomodoro checkpoint does, and why `away_ms` stays 0 there. Corrects §4.5, which said the interrupted block "keeps its `end_at`" — it holds its *remainder* and recomputes `end_at` on resume. Test 73. |
| 2026-09-04 15:50 | Initial version. D27–D33; migration 006; acceptance tests 62–72.     |

---

## 1. The premise

The issue asks for a mode that "forces the user to take a break every 25 mins
regardless of the task status". Two words in that sentence carry the design.

**"Regardless of the task status"** is the whole reason this is a second timer
rather than a setting on the first. Making Pomodoro mean *every block is 25
minutes* would delete per-task allocation — the thing the product is for — and
would mean a 45-minute task could no longer be given 45 minutes. Instead the
task timer is untouched and a Pomodoro timer runs beside it. Whichever expires
first opens its own checkpoint; the other keeps its remaining time.

**"Forces"** is softened by one deliberate step, at Gigih's call: the Pomodoro
checkpoint offers *Skip break & continue* next to *Take a break*. What is forced
is the **decision**, not the break — which is exactly the force the rest of the
app applies (SPEC §5.2: the checkpoint has no exit). A task two minutes from
done should not have to be abandoned to a five-minute break, and after
completing it the user can take a break from the task checkpoint as they always
could.

### 1.1 The two clocks, side by side

```
Pomodoro ON.  Task A allocated 45m.

  0m ──────────────── work ──────────────── 25m
                                             │
                              ┌──────────────┴──────────────┐
                              │  Time for a break           │  ← Pomodoro
                              │  [Take 5m break] [Skip]     │    checkpoint
                              └──────────────┬──────────────┘
                                             │ take break
  break 5m ──────────────────────────────────┘
                                             │
 25m ──────────── work (task has 20m left) ── 45m
                                             │
                              ┌──────────────┴──────────────┐
                              │  Task A — 45m spent         │  ← task
                              │  [Complete] [Pending] [+5…] │    checkpoint
                              └─────────────────────────────┘
```

The task clock is paused for the length of the break — parked, not spent, as
every break already does (SPEC D7). Twenty minutes of Task A remain on the other
side of it.

---

## 2. Decisions

| #   | Decision | Reasoning |
| --- | -------- | --------- |
| D27 | **Pomodoro is a second timer, not a change to block allocation.** A task's `block_duration_ms`, its extensions and its checkpoint behave identically whether the mode is on or off. The Pomodoro clock is a separate quantity that produces a separate checkpoint. | The task allocation is the product (SPEC §1). Overriding it with a fixed 25 would make "give this one 45 minutes" impossible to express while the mode is on, and would silently rewrite allocations the user set deliberately. Two clocks also match what the user is actually tracking: one is a budget for a task, the other is a rest interval for a body. Neither is a good proxy for the other. |
| D28 | **The Pomodoro clock counts only running work time.** It advances exactly when a `WORK` block is `RUNNING`, and parks in `PAUSED`, `IDLE`, `AWAITING_DECISION` and during a break. Twenty-five minutes of work, however long that takes on the wall clock. | Wall-clock 25 would fire the moment a user came back from an hour's meeting to say "time for a break" — the exact opposite of useful. The clock exists to bound *continuous effort*, and the app already knows precisely when effort is happening: it is the same `RUNNING` interval `work_spans` records. Using a different definition here would mean two answers to "how long have you been working" in one app. |
| D29 | **Any break resets it, and so does answering the prompt.** The clock restarts from zero when a break ends, when *Skip & continue* is chosen, and when the mode is switched on. A pause or idle stretch, however long, **parks** it rather than resetting it. | A break is rest, whatever door it came through — the Pomodoro prompt, a task checkpoint's break option, or the tray. Refusing to credit rest the app itself provided would be dishonest. A pause is not rest: it is the user stepping away from the timer, and nothing observed says they stopped working. Wiping 20 minutes of accumulated work because someone paused for 30 seconds to answer a question is a rule that punishes correct use of the pause button. Skipping resets in full rather than nagging again in five minutes: a re-prompt at a shorter interval is a second, undeclared duration, and a mode that argues with the answer it just received is worse than one that does not ask. |
| D30 | **The prompt is a blocking checkpoint with two doors: *Take a break* and *Skip & continue*.** Same window, same rules as the work checkpoint — no dismiss, `Esc` inert, `Cmd+W` refused, no timeout. The task itself is not decided here: it is neither completed nor rotated, and it is still current when work resumes. | A notification would be ignorable, and an ignorable break interval is not a Pomodoro mode. Making it a checkpoint reuses the one mechanism the app already has for "you must answer this", including the away-time banking that keeps a wait from being counted as work (SPEC D13). Keeping the task out of it is what makes the second timer *independent*: mixing a break decision with a complete/pending decision would make every 25 minutes a referendum on the task, which is churn (D11) the mode has no business creating. |
| D31 | **When both clocks are due, the task checkpoint wins and the Pomodoro prompt is dropped, not queued.** The task checkpoint already offers a break, and that option is pre-checked when a Pomodoro was due. | Two blocking windows in a row for one moment is the interruption the user was trying to bound. The task checkpoint is strictly the richer of the two — it can do everything the Pomodoro prompt can do and more — so showing it alone loses nothing. Dropping rather than queueing follows: once the break has been offered and answered, asking again is asking twice. |
| D32 | **The work interval is fixed at 25 minutes; the break length is the existing `default_break_duration_ms`.** No new duration settings. | 25 is what the technique is, and the issue names it. The break duration is already a setting the user has set once, and a Pomodoro break is a break — a second break-length field would be two answers to one question, and the first thing to get out of sync. If a configurable interval is wanted later it is one column; speculating it now is an abstraction for a single case (Rule 2). |
| D33 | **The toggle is a setting, mirrored as a one-click item in the tray and the popover, and it is refused while any checkpoint is open.** Switching it on starts a fresh 25 from that instant; switching it off discards the clock entirely. | The mode is meant to be flipped mid-day — "I need to get through this one, no interruptions" — so burying it in a settings window would make it unused. Refusing it at a checkpoint is the same rule that makes `SwitchTo`/`Pause`/`Resume`/`StartBreak` no-ops there: the checkpoint has no side doors, and letting the mode be switched off to dismiss its own prompt would be exactly such a door. Starting fresh rather than crediting the current block's elapsed work means the toggle can never fire a prompt the instant it is flipped, which would read as a bug. |

### 2.1 What is deliberately absent

- **No long break, no cycle count.** The classic technique lengthens every
  fourth break. That needs a counter, a persistence rule and an answer to "when
  does it reset" — none of it asked for, and each a thing to get wrong across a
  quit or a day boundary.
- **No auto-start of the next pomodoro or of the break.** Every transition still
  passes through a decision the user makes. That is the app's premise, not a
  detail of this feature.
- **No Pomodoro statistics.** No count of pomodoros completed today, no streak.
  `work_spans` already answers the time question, and Today already reports it.
- **No effect on capacity, idle or Today.** A Pomodoro break is an ordinary break
  block: it carries no task, counts as no work, and consumes no daily capacity,
  exactly as breaks do today.

---

## 3. The clock, precisely

### 3.1 Definition

Let `pomodoro_since` be the instant of the last reset (D29). The clock reads:

```
elapsed = Σ { |span ∩ [pomodoro_since, now]| : span ∈ work_spans
                                             , block(span).kind == WORK }
```

— the same interval algebra `core::summary` uses for a day's worked time, over
a different window. The open span counts as `[started_at, now]`.

The prompt is due when `elapsed >= 25 min`.

**It is derived, not accumulated.** There is no counter to advance on each tick
and no field that can drift out of step with the spans. This is D24's argument
in a second place: a stored accumulator would need something to maintain it
across a quit, a crash and a sleep, and each is a chance to be wrong; a
derivation cannot miss, because the spans are written by `sync_work` on the
transition regardless.

Break spans are excluded by kind rather than by trusting D29 — a break inside
the window should be impossible, since a break's end resets `pomodoro_since`,
but a rule that holds by construction *and* by filter is cheaper than one that
holds only by argument.

### 3.2 Reset points

`pomodoro_since = now` on each of:

| Trigger | Where |
| ------- | ----- |
| The mode is switched on | `Event::SetPomodoroMode { on: true }` (§4.6) |
| A break **ends** — any path that reaches `end_current` on a `Break` block: `EndBreak`, and `Skip` during a break | `reduce` |
| *Skip & continue* is chosen | `Event::DecideSkipPomodoro` |
| *Take a break* is chosen | `Event::DecidePomodoroBreak` (the break's end resets again; harmless and keeps the rule "answering the prompt resets" whole) |

A break block *expiring* is deliberately **not** a reset point. Expiry opens the
break checkpoint (`at_break_checkpoint`); the break has not ended until the user
answers it, and the interval spent waiting is not rest. The reset belongs to
`end_current`, which is the one place every break's ending passes through.

Switching the mode **off** sets `pomodoro_since = None`. Nothing is remembered:
switching it back on starts a fresh 25 (D33).

### 3.3 Why a pause parks rather than resets

`pomodoro_since` is an instant, not a stopwatch, so a pause needs no code at all
to park the clock: no work span accrues while the timer is not `RUNNING`, so
`elapsed` simply stops growing. This is the whole reason the quantity is defined
as a set intersection rather than as `now - pomodoro_since`.

---

## 4. Data model

### 4.1 Settings

```rust
pub pomodoro_mode: bool,
```

### 4.2 `MachineState`

```rust
/// The instant the current pomodoro started accruing from (§3.1). `None`
/// exactly when the mode is off.
pub pomodoro_since: Option<Millis>,
```

Persisted — an open pomodoro must survive a quit the way an open idle span does
(IDLE_TIME D15). It is *not* sent to the UI as a raw field; see §5.1.

### 4.3 Migration 006

```sql
ALTER TABLE settings  ADD COLUMN pomodoro_mode  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE app_state ADD COLUMN pomodoro_since INTEGER;
```

The state table is `app_state` (001), not `state`.

Forward-only, following 001–005. Existing installs come back with the mode off,
which is the correct default: a mode that turned itself on at upgrade would be a
surprise interruption.

### 4.4 Events

```rust
/// Answer the Pomodoro checkpoint: take the break (D30). `ms` is the settings
/// break duration, passed the way `DecideBreak` passes it.
DecidePomodoroBreak { ms: Millis },
/// Answer the Pomodoro checkpoint: keep working. Resets the clock (D29).
DecideSkipPomodoro,
```

Both are no-ops unless a Pomodoro checkpoint is actually open — the same
guard every `Decide*` event already carries.

### 4.5 A fourth `TimerState`, not a new `BlockKind`

```rust
pub enum TimerState { Idle, Running, Paused, AwaitingDecision, AwaitingPomodoro }
```

No new `BlockKind`: no block is created here. The Pomodoro checkpoint
interrupts the work block that is running, and that block stays the current
block — **parked** by `park()`, so `remaining_when_paused_ms` holds its
remainder and `end_at` is recomputed on resume. It is emphatically *not*
`expire()`d: `expire` sets `status = AwaitingDecision` and clears
`last_resume_at`, which throws the remainder away.

The state variant is doing real work, and a discriminator on
`Effect::EnterCheckpoint` alone cannot replace it:

- **`at_work_checkpoint()` is `AwaitingDecision && !on_break()`**, and a
  Pomodoro prompt satisfies both — its block is a `WORK` block that is not on
  break. Without a distinct state, `DecideComplete`, `DecidePending`,
  `DecideExtend` and `DecideBreak` would all be *accepted* at the Pomodoro
  prompt, silently completing or rotating the task the prompt was supposed to
  leave alone; `AddTime`, which keys on `AwaitingDecision`, would refuse when it
  should not care. A fourth variant excludes the Pomodoro checkpoint from every
  one of those guards without editing any of them.
- **Effects are transient; state is persisted.** `app_state.timer_state` is what
  comes back after a quit. With the kind living only on the effect, a relaunch
  could not tell which checkpoint had been open — and `reduce`'s `Tick` arm acts
  only when `timer_state == Running`, so nothing would re-derive it either.
- **Deriving it from the block instead was rejected.** `end_at > now` stops
  being true once the prompt has been ignored long enough, and keying on
  `status == Paused` makes a block field secretly mean "which window is open".

Consequences to carry through, each a compile error rather than a silent
acceptance:

| Site | Change |
| ---- | ------ |
| `IdleReason::of` | `AwaitingPomodoro => Some(IdleReason::Awaiting)` — the wait is idle for the same reason a work checkpoint's wait is (§6.1). |
| `at_work_checkpoint` / `at_break_checkpoint` | The *functions* are unchanged and correctly false at a Pomodoro prompt — but that is only the right answer for callers that test them **positively**. See §4.7: `StartBreak` tests the negation and breaks. |
| `str_enum!(TimerState …)` | `AwaitingPomodoro => "AWAITING_POMODORO"`. The column is `TEXT`, so no schema change — but the round-trip test in `model.rs` must cover it. |
| `Effect::EnterCheckpoint`, `Effect::Notify` | Carry `CheckpointKind { Work, Break, Pomodoro }` instead of `BlockKind`. `platform::checkpoint::notify` matches on it and needs its third arm; `BlockKind` stays two-valued, because `blocks.kind` can still only ever be written `WORK` or `BREAK`. |

### 4.6 The toggle is an event, not a settings write

```rust
/// Turn Pomodoro mode on or off (D33). Refused while any checkpoint is open.
SetPomodoroMode { on: bool },
```

The obvious implementation — a checkbox handled by `update_settings` — splits one
change across two stores and two code paths: `pomodoro_mode` lives in
`Settings`, `pomodoro_since` lives in `MachineState`, and `update_settings`
never sees machine state, so it can neither set the reset instant nor enforce
D33's refusal at a checkpoint. Routing the toggle through `reduce` puts both
writes in one transition and one transaction, and the refusal beside the other
checkpoint guards where it will be found.

`settings.pomodoro_mode` remains the persisted flag; it is written by the
dispatch that handles this event, not by `update_settings`. The Settings UI
sends the action like every other control (SPEC R7).

### 4.7 Guards that a fourth state does not fix for free

`AwaitingPomodoro` excludes the Pomodoro prompt from the four `Decide*` guards
because each tests `at_work_checkpoint(&state)` positively. Three other sites do
not have that shape, and each is a hole rather than a compile error:

| Site | What happens | Required |
| ---- | ------------ | -------- |
| `Event::StartBreak` | Guarded by `!at_work_checkpoint(&state) && !state.on_break()` — a **negation**. At a Pomodoro prompt `at_work_checkpoint` is false, so the guard *passes* and the break is accepted. `tray.rs:92` enables its break item on `timer_state != AwaitingDecision`, so this is reachable by one click: a live side door out of a checkpoint that D30 says has none. | Guard on "any checkpoint is open", not on the work one. `tray.rs` must disable the item for `AwaitingPomodoro` too. |
| `Event::Skip` | No state guard whatsoever — `if let Some(b) = state.current_block()`. Ends the block and rotates the task straight out of the prompt. | Refuse while `AwaitingPomodoro`. (It is equally unguarded at the *work* checkpoint today; Pomodoro mode is what makes it reachable, since the popover stays usable while the prompt is up.) |
| `Event::CompleteCurrentTask` | Likewise unguarded, and the popover offers **Complete** on the current-task row (changelog 2026-08-27). Test 69 asserts it is a no-op; with no guard, that test fails. | Refuse while `AwaitingPomodoro`. |

`Pause`, `Resume` and `AddTime` need nothing: the first two test `== Running` /
`== Paused` and are already inert, and `AddTime`'s acceptance is intended (test
75).

The rule underneath is one predicate, added beside the other two:

```rust
fn at_any_checkpoint(s: &MachineState) -> bool {
    matches!(s.timer_state, TimerState::AwaitingDecision | TimerState::AwaitingPomodoro)
}
```

"The checkpoint has no exit" was previously enforced by four positive tests and
two accidents. It needs to be enforced by one predicate, or the next state added
re-opens the same doors.

---

## 5. Surfaces

- **Settings** gains a `Pomodoro mode` toggle, with the fixed 25 min / break
  duration stated beneath it as text, not as fields (D32).
- **Tray** and **popover** each gain a checkable `Pomodoro mode` item, disabled
  while a checkpoint is open (D33).
- **The main window and popover** show the Pomodoro remaining time beside the
  task countdown while the mode is on — two numbers, clearly the smaller of the
  two being the next interruption. Nothing is shown when the mode is off, and
  nothing **during a break**: the clock resets when the break ends (D29), so a
  break would otherwise display the stale pre-break number, counting down to a
  break already being taken. The guard lives in `PomodoroCountdown` itself as
  well as at each call site, since the "break in" label sits in the caller.
- **The checkpoint window** gates *everything* on
  `snap.state.timerState === "AwaitingDecision"` (`Checkpoint.tsx:28,55`) and
  returns a bare `<div className="min-h-screen bg-surface" />` when that is
  false. Left as is, a Pomodoro checkpoint shows an **empty, always-on-top,
  undismissable window** — the worst available failure, since the window has no
  close affordance by design. The gate must become "either awaiting state", with
  a third branch beside `onBreak`, and the `useEffect` keyboard handler
  (gated on the same `awaiting`) extended with `1` = take break, `2` = skip.
- **The menu bar** matches `TimerState` exhaustively (`menubar.rs:17-20`) and
  needs a fourth arm — `"☕ BREAK?"` — or it will not compile.
- **The checkpoint window's Pomodoro variant** renders: title *Time for a break*,
  the current task and how long it has run, and the two buttons. No close
  affordance, matching the work checkpoint exactly.

### 5.1 `Snapshot.pomodoro`

```ts
pomodoro: { on: boolean; remainingMs: number } | null
```

Computed in Rust from §3.1 and rebuilt with the snapshot each second. The UI
never sums spans and never compares `pomodoro_since` to anything — SPEC R7. It
interpolates the countdown against the backend instant exactly as the task
countdown does, and **never concludes the pomodoro is due itself**: at 00:00 it
shows zero and waits for the backend's checkpoint.

---

## 6. Interaction with what already exists

| Existing behaviour | Under Pomodoro mode |
| ------------------ | -------------------- |
| Task checkpoint (Complete / Pending / Extend / break) | Unchanged, except that the break option is pre-checked when a pomodoro was also due (D31). |
| `Skip`, `SwitchTo`, rotation | Unchanged. The clock keeps accruing across a switch — it measures the body, not the task. |
| `StartBreak` from the tray | Unchanged; its end resets the clock (D29). |
| Daily tasks | Orthogonal. A pomodoro during a daily is a pomodoro. |
| Idle time | Unchanged. A Pomodoro break is a break block, so the interval is neither work nor idle, as breaks already are. |
| Quit mid-pomodoro | The block is parked (D16) and no span accrues while the app is dead, so `elapsed` is exactly what it was at quit. Nothing to recover. |
| An unanswered prompt | §6.1. |
| `hydrate`'s single `Tick` | Resolves a pomodoro that came due while the app was closed, the same way it resolves an expired block — except it cannot, because the clock parks when the timer stops and quitting parks the timer. The prompt therefore never fires on launch, which is right: no work happened. |

### 6.1 Ignoring the prompt

The checkpoint has no timeout (D30), so a user who walks away leaves it open
indefinitely. Everything about that case falls out of mechanisms that already
exist, and **nothing is lost or advanced**:

1. `TimerState` is `AwaitingDecision` and the work block is **parked** — its
   remainder held, exactly as `Pause` holds it.
2. An idle span opens with `IdleReason::Awaiting`, so the wait is reported as
   idle in Today when it falls inside the working window — the same treatment a
   work checkpoint's wait already gets.
3. No work span accrues, so the Pomodoro clock parks with everything else
   (§3.3). `elapsed` sits at 25 min and does not grow.
4. The tick thread parks on its condvar; `useTimebox`'s 10s poll is what keeps
   `idleMs` and `stalenessMs` moving on screen.
5. Quitting with it open and relaunching brings the same checkpoint back, via
   `hydrate`'s single `Tick`. There is no separate recovery path.
6. Whatever is eventually chosen acts from **now**, never retroactively: *Take a
   break* runs a full break starting now, and *Skip & continue* unparks the
   task's held remainder and starts a fresh 25.

**`staleness_ms()` needs a second branch.** It reads
`(now - end_at).max(0)`, and at a Pomodoro checkpoint the block is parked with
`end_at` in the future, so it returns 0 and the staleness line shows nothing
however long the prompt is ignored. It must read from `paused_at` when
`timer_state == AwaitingPomodoro`. This is the opposite call from `away_ms`
below, and for a reason: staleness answers *how long has this been waiting*,
which is a real number here, while `away_ms` answers *how much time did the wait
cost the block*, which is genuinely zero.

**`away_ms` is deliberately not banked here.** `settle_away` computes
`now - end_at`, and at a Pomodoro checkpoint `end_at` is still in the future, so
the `.max(0)` yields zero. That is the right answer rather than an oversight:
`away_ms` exists because a *work* checkpoint's block has already expired and has
no remainder left to park, so the wait would otherwise be unrecorded. Here the
park preserves the remainder, and banking the gap as well would charge the wait
twice. Test 73 pins it so the zero is not later "fixed".

### 6.2 Paths that end the block while the prompt is open

`RemoveTask` is guarded by no checkpoint test: removing the current task calls
`end_current` and `start_next` and pushes **no** `LeaveCheckpoint`, which would
leave the Pomodoro window on screen over a block that no longer exists. Every
path that ends the current block must push `LeaveCheckpoint` when
`timer_state == AwaitingPomodoro`. (The same hole exists today for the work
checkpoint; Pomodoro mode is what makes it easy to reach.)

### 6.3 Not credited as rest

The wait is also not credited as rest — an hour ignoring the prompt does not
reset the clock by itself (D29 resets on breaks, not on idle). This is moot in
practice, because *answering* the prompt resets it either way; it matters only
if the mode is switched off while the prompt is open, which D33 refuses.

---

## 7. Acceptance tests

Continuing the numbering. All in `src-tauri/src/core/tests.rs` (`mod pomodoro`)
except 71, 72 and 76, which need persistence and settings and live in
`src/state/tests.rs`, and 81, which is a UI test.

| #   | Test |
| --- | ---- |
| 62  | With the mode on and a task allocated 45 min, 25 min of running work opens a **Pomodoro** checkpoint; the block's `end_at` is untouched and the task is still current (D27, D30). |
| 63  | With the mode **off**, the same 25 min of work opens nothing (D33). |
| 64  | *Skip & continue* resumes the same task with its full remaining allocation, and the next prompt is 25 min of work later — not sooner (D29, D30). |
| 65  | *Take a break* parks the work block, runs a break of `default_break_duration_ms`, and on `EndBreak` resumes the *remainder* of the task's block (SPEC D10 is not weakened by this feature). |
| 66  | The clock parks across a pause: 20 min worked, paused an hour, resumed → the prompt fires 5 min later, not on resume (D28, D29). |
| 67  | A break taken from a **task** checkpoint resets the clock; 25 min of work after it, not before it, is what fires the next prompt (D29). |
| 68  | Both due at once → one window, the task checkpoint, with the break option pre-checked; no Pomodoro checkpoint is queued behind it (D31). |
| 69  | The Pomodoro checkpoint has no exit: `SwitchTo`, `Pause`, `Resume`, `StartBreak`, `CompleteCurrentTask` and toggling the mode off are all no-ops while it is open (D30, D33). |
| 70  | Switching the mode on 12 min into a running block starts a fresh 25 — the prompt is 25 min of work away, not 13 (D33). |
| 71  | `pomodoro_since` and `pomodoro_mode` survive a quit and relaunch; work done before the quit still counts toward the current pomodoro, and the interval the app was closed does not (§4.2, §6). |
| 72  | Switching the mode off and on again discards the accumulated work: a fresh 25 (D33). |
| 73  | A Pomodoro checkpoint left unanswered for an hour: the block is parked with its remainder intact, `away_ms` stays **0**, the hour is reported as `IdleReason::Awaiting`, and the clock has not grown past 25 min. Answering then acts from `now` — the break is full length and the task's remainder is untouched (§6.1). |
| 74  | While a Pomodoro checkpoint is open, `DecideComplete`, `DecidePending`, `DecideExtend` and `DecideBreak` are **no-ops** — the task is not completed, not rotated, and its block is not re-armed (§4.5). |
| 75  | `AddTime` *is* accepted at a Pomodoro checkpoint and lands on the parked block, unlike at a work checkpoint where it is refused (§4.5). |
| 76  | `AWAITING_POMODORO` survives the `TimerState` string round-trip, and a state persisted in it reloads as a Pomodoro checkpoint rather than a work one (§4.5). |
| 77  | `RemoveTask` on the current task with the prompt open emits `LeaveCheckpoint` and starts the next task (§6.2). |
| 78  | A break block *expiring* does not reset the clock; answering the break checkpoint with `EndBreak` does. `Skip` during a break resets it too (§3.2). |
| 79  | `Event::StartBreak` is refused while a Pomodoro checkpoint is open, and the tray's break item is disabled there (§4.7). |
| 80  | `Event::Skip` and `Event::CompleteCurrentTask` are refused while a Pomodoro checkpoint is open — the block is not ended and the queue does not move (§4.7). |
| 81  | The checkpoint window renders the Pomodoro variant, not a blank surface, when `timerState == AwaitingPomodoro` (§5). A UI test, so it lives beside the other component tests. |

The wire encoding of `pomodoroMode` on `update_settings`, and of the two new
actions, is pinned in `commands::tests` — for the reason the `priority` and
`daily` tests exist: a wrong field name does not fail, it silently leaves the
mode off forever.

---

## 9. Deviations taken while implementing

| Spec said | Built instead | Why |
| --------- | ------------- | --- |
| §4.1/§4.3: `settings.pomodoro_mode` beside `app_state.pomodoro_since` | **`pomodoro_since` only.** It is `NULL` exactly when the mode is off, so it doubles as the flag. | §4.6 rejected splitting the *write* across two stores. The same argument applies to splitting the *storage*: two columns are two things to keep in step across every toggle, and the second adds nothing the first does not already say. `MachineState.pomodoro_since.is_some()` is the mode, everywhere. |
| §4.3: a bare `ALTER TABLE app_state ADD COLUMN` | **A table rebuild.** | 001 declared `CHECK (timer_state IN ('IDLE','RUNNING','PAUSED','AWAITING_DECISION'))`, and SQLite cannot alter a CHECK in place. Left alone it rejects every write made while the Pomodoro prompt is open — a *runtime* constraint violation no Rust type could catch, and one that would have appeared only on the first real 25-minute block. Test 76 is what found it. |
| §4.7 listed three unguarded sites | **Five.** `switch_to` and the popover's `canBreak` were also holes. | `switch_to` refuses with `if at_work_checkpoint(state) { return; }` — a *positive* test used as a refusal, so `false` means "allowed", which is the same polarity trap as `StartBreak`'s negation used as permission. The audit checked the shape of the predicate and not the shape of its *use*. Test 69 caught it; `canBreak` was found by reading the popover for the same pattern. |

The `at_any_checkpoint` predicate is what makes the last row a one-line fix
rather than a recurring one, which was §4.7's point.

---

## 8. Known limits

- **It is not actually forced.** *Skip & continue* is always available, so a
  determined user can work all day and never break. This was a deliberate call
  (§1): the alternative — a break with no way out — makes finishing a task two
  minutes from done impossible, and a tool that cannot be overruled gets
  switched off entirely rather than overruled once.
- **The break offered is one length.** There is no long break and no way to say
  "just two minutes" at the prompt. Extending a break already works once it has
  started (`ExtendBreak`), which covers the other direction.
- **Twenty-five minutes is not configurable.** Deliberate (D32), and the first
  thing to revisit if the mode is used and the number is wrong.
- **A crash, unlike a quit, can make the prompt fire on launch.** A clean quit
  parks the block (D16), so no span accrues while the app is dead and §6's claim
  holds. A crash leaves `open_work` open, and `sync_work` reconciles it on the
  next tick — so the hours the process was gone count as work and the prompt can
  fire immediately on relaunch. This is the same blindness the block allocation
  already has, and it is `SLEEP_DETECTION.md` (D21) that closes it for both.
- **The clock is blind to time away from the machine that is not a break.** A
  user who leaves the timer running and walks away accrues pomodoro time they
  did not work. That is the same blindness `work_spans` has, and it is bounded
  by the same thing — the block's allocation and the checkpoint at the end of
  it. `SLEEP_DETECTION.md` (D21) is where that gap is addressed for both.
