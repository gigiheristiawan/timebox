# Feature Spec — Sleep Detection

Makes a Mac going to sleep stop the timer, so that a block running when the lid
closes is not credited with the hours the lid was shut. Status: **specified, not
implemented.** Implements **D21** of `docs/features/IDLE_TIME.md`; amends
`docs/SPEC.md` §6.

---

## Changelog

| Date (WIB)       | Change           |
| ---------------- | ---------------- |
| 2026-08-24 11:20 | Initial version. |

---

## 1. Why this exists

`docs/features/IDLE_TIME.md` D16 makes quitting park the running block, because
an interval cannot be counted as idle and as work at once. Sleep is the same
interval problem and the *more common* one — closing a lid is how most people
leave a desk, and unlike quitting it takes no decision at all. Sealing quit and
leaving sleep open would fix the smaller hole and advertise the larger one.

Today a lid closed at 14:10 during a 30-minute block reopens at 17:00 to a
checkpoint reporting a full 30 minutes worked. D13's staleness line
(*"This block ended 2h 44m ago"*) surfaces the wait *after* expiry, but the
minutes between 14:10 and 14:30 are recorded as work that did not happen.

The end state is one rule, replacing the "clock is absolute" language in
`docs/SPEC.md` §6:

> **The clock runs only while the Mac is awake and the app is alive.**

## 2. Detection

Sleep is **measured, not guessed at**. Darwin exposes two monotonic clocks that
differ by exactly the quantity wanted:

| Clock | Mach equivalent | Advances during sleep? |
| --- | --- | --- |
| `CLOCK_MONOTONIC_RAW` | `mach_continuous_time` | yes |
| `CLOCK_UPTIME_RAW` | `mach_absolute_time` | no |

So for any interval:

```
slept_ms = Δ CLOCK_MONOTONIC_RAW − Δ CLOCK_UPTIME_RAW
```

The `_RAW` pair specifically: plain `CLOCK_MONOTONIC` is subject to frequency
adjustment, and a clock the system may re-rate has no business being one half of
a subtraction whose whole value is that the two halves differ by exactly one
thing.

Both are read through `libc::clock_gettime` — no new crate, no entitlement, and
nothing private. The tick loop samples both on every tick and carries the
previous pair; the difference of the differences is the sleep that occurred
between the two ticks.

**Why not a wall-clock gap heuristic.** The obvious alternative — "this tick is
much later than 1 s after the last, so the Mac must have slept" — cannot
distinguish sleep from a delayed thread, and a background `LSUIElement` app is
exactly the kind of process macOS throttles under App Nap or heavy load. It
would park blocks on a busy machine. The clock difference has no such ambiguity:
it is zero whenever the Mac stayed awake, however late the tick was. A wall-clock
gap is used only as a corroborating log line, never as the trigger.

**Sampling is only needed while `RUNNING`.** The ticker parks on its condvar in
`IDLE`, `PAUSED` and `AWAITING_DECISION` (`state.rs:157`), and in each of those
there is no running block to protect. Sleep during those states already lands in
idle by the `IDLE_TIME.md` §3 definition — window time no block covered — with no
code at all. So this feature adds **zero background work**: the clock pair is
read on ticks that already happen.

### 2.1 Floor

Sleeps shorter than `SLEEP_FLOOR_MS = 60_000` are ignored. Below a minute the
mis-credited work is negligible and parking the block would cost the user a
resume click for nothing. The floor is a product choice, not a detection limit —
the measurement is exact at any scale.

### 2.2 When sleep began

The sleep is bracketed by two ticks and its start is not directly observable.
The block is parked at the **last successful tick instant**, which overestimates
work by at most one tick interval (1 s). Sub-second precision is not worth a
wake notification to obtain.

## 3. Behaviour

On a tick where `slept_ms >= SLEEP_FLOOR_MS` and a block is `RUNNING`, the shell
dispatches a new core event:

```rust
Event::SystemSlept { from: Millis, to: Millis }   // from = last tick, to = now
```

`reduce` then:

1. Parks the block at `from` — the same `park()` the `Pause` path uses, so the
   remainder is banked and `end_at` is discarded rather than decremented.
2. Opens an `UNTRACKED` idle span at `from` (see `IDLE_TIME.md` §5.1), closed
   immediately at `to`. `core::summary` clips it to the working window, so an
   overnight sleep contributes only the in-window part.
3. Leaves `TimerState::Paused`. The block does **not** auto-resume on wake.

Sleep during `AWAITING_DECISION` needs no event: the open checkpoint's gap is
already live-derived from `end_at` (D13), and it is already idle.

### 3.1 Park, not auto-resume

The considered alternative was to keep the block running and push `end_at` out
by `slept_ms`, so a short lid-close resumes transparently. Rejected: it is a
second, invisible mechanism for the same thing `park` already does, and the
convenience it buys is one click. Sleep is a form of walking away, and the
product's whole premise is that walking away is an explicit state rather than
something the app papers over. One rule — sleep parks, exactly as quit parks —
is also the only version a user can predict.

Accepted cost: a 20-minute commute with the lid shut returns to a paused block
needing an explicit resume.

### 3.2 Layering

Detection is a shell concern and lives in `state.rs`'s tick loop, next to the
existing `Tick` dispatch. The core receives an already-measured interval and
stays pure and deterministic — `SystemSlept` is testable by passing two instants,
with no clock of its own, exactly like every other event.

`platform/` gains nothing. There is deliberately still **no**
`NSWorkspace.didWakeNotification` observer: the late tick that already resolves
expiry now also carries the evidence of sleep, so an observer would add an
AppKit dependency to learn something already in hand.

## 4. Acceptance tests

Continuing `docs/features/IDLE_TIME.md` (23–33).

| # | Test |
| --- | --- |
| 34 | `slept_ms` is the difference of the two clock deltas: a synthetic tick pair with equal deltas reports zero sleep however large the wall gap. |
| 35 | Tick delayed 10 minutes with both clocks advancing equally (App Nap, load) → no sleep, block stays `RUNNING`, no idle span. |
| 36 | Sleep of 45 s while running → below the floor, ignored; block stays `RUNNING`. |
| 37 | Sleep 14:10→17:00 during a 30 m block started 14:00 → block `PAUSED` holding 20 m, `worked_ms` = 10 m, not 30. |
| 38 | Same sleep with an 18:00 `work_end` → `idle_untracked_ms` = 170 m; a sleep 17:30→09:00 next day contributes only 30 m to that day. |
| 39 | `SystemSlept` while `AWAITING_DECISION` is a no-op — the gap is already `away_ms`, and banking it twice is the D13 double-count. |
| 40 | `SystemSlept` while `IDLE` or `PAUSED` is a no-op; idle for that span still accrues from the window definition. |
| 41 | Sleep during a **break** block parks the break the same way. A break is a block; nothing here is work-specific. |

## 5. Known limits, accepted

- **Sleep start is accurate to one tick (1 s).** §2.2.
- **Screen lock is not sleep.** A locked-but-awake Mac keeps the block running
  and keeps crediting work. Detecting it needs a session-state observer and a
  separate decision about whether locking means "away"; out of scope. A user who
  locks and leaves still hits the checkpoint, which is the existing backstop.
- **Hibernation and deep standby** are sleep by these clocks; nothing special.
- **A clock moved backwards** yields a negative difference; clamped to zero,
  matching `TimeBlock::active_ms` (SPEC §11).
- **The floor hides very short sleeps** by design (§2.1).

## 6. Dependencies

Cannot ship before `docs/features/IDLE_TIME.md` — it needs `idle_spans`, the
working window, and D16's parking path. Shipping this first would park blocks on
sleep with nowhere to record the gap, which is strictly worse than today.
