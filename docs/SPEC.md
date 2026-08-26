# TimeBox — Product & Technical Specification

**Version:** 1.0 (MVP)
**Platform:** macOS 13+ (Apple Silicon primary, Intel best-effort)
**Source document:** `MacOS Task Rotation Timeboxing App — Claude Handoff.md`

---

## Changelog

All entries below are from a single working session on 2026-08-19. Hours before 21:55 are **reconstructed and approximate** — per-change timestamps were not recorded at the time. Entries from 21:55 onward are exact.

| Date (WIB)       | Change                                                                                                                                                                          |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-08-26 11:25 | **Tasks can be renamed, re-prioritised and granted more time** — new `Event::EditTask` and `Event::AddTime`, an inline editor on the queue row and on the current task (§7.3). Time is added, never assigned, and a grant is refused at a checkpoint where `Extend` already owns that decision. Also fixes the priority that never arrived — `Action::AddTask` took `priority` as a `String` and ran it through `Priority::parse`, which reads the *database* encoding (`HIGH`) while the UI sends serde's (`High`), so every task ever added stored `MEDIUM`. |
| 2026-08-24 16:40 | **§6 reversal landed, and D14 is superseded.** Quitting now parks the running block — `commands::request_quit` and `RunEvent::ExitRequested` both dispatch the existing `Event::Pause`, so the interval the app is closed reads as idle, not work (IDLE_TIME D16). The quit confirmation and its window are deleted: they existed to make the cost of quitting visible and there is no cost left. §6's paragraph is replaced rather than annotated, D14 is struck, §7.4's confirm paragraph and §11's quit rows are rewritten. D21 (sleep) is still pending and unaffected. |
| 2026-08-24 11:20 | **§6 amended — pending reversal of "quitting does not stop the clock".** `docs/features/IDLE_TIME.md` (D16) parks the running block on quit, and `docs/features/SLEEP_DETECTION.md` (D21) does the same for a detected sleep, so that measured idle time and measured work can never claim the same interval. The paragraph and the sleep/wake bullet below are marked with what replaces them; neither is implemented yet, so the body still describes shipped behaviour. |
| 2026-08-20 11:00 | Bundle identifier is `xyz.gigiheristiawan.timebox`; §4.5's data path updated to match. The identifier names the data directory, so the two can never be documented independently. |
| 2026-08-20 01:20 | **D13 corrected in the body.** It read that Away is "derived from the expiry timestamp"; that is only true of the checkpoint currently open. Once answered, the gap cannot be reconstructed — a *parked* block also carries a past `end_at` though no checkpoint was ever open, and a block extended after expiring reaches the checkpoint more than once. Migration **002** adds `time_blocks.away_ms`, banked at each accepted decision, plus `settings.first_run_done` for D12's one-time panel. Neither changes what the product does; both make Today's Away line true. §4.5 amended. |
| 2026-08-19 21:05 | §6: recorded *why* quitting does not stop the clock — pausing on quit would let a block be banked, the same loophole D10 closes. Noted the actual-duration cap.                    |
| 2026-08-19 20:55 | **D14** — quit confirmation while a block is running (Pause & Quit / Quit / Cancel). Two edge cases.                                                                              |
| 2026-08-19 22:35 | §7.1 corrected — the break example read `◔ BREAK 4:12`; minutes are zero-padded like every other clock in the app, so an unpadded minute would shift the menu bar's width once a minute. Implemented as `◔ BREAK 04:12` in `core::menubar`. |
| 2026-08-19 20:50 | **D12** (first-run discoverability) and **D13** (staleness line + `Away` total). Tests 20–21 added; old test 20 renumbered to 22.                                                 |
| 2026-08-19 20:45 | Q1/Q2/Q5 resolved. **D7 rewritten**: break is a modifier on *either* task decision, so the checkpoint became a 2×2 of compound actions plus Extend. D9 confirmed. Test 17b added. |
| 2026-08-19 20:35 | **R5 corrected** to `rusqlite` — `tauri-plugin-sql` exists to let the *frontend* run SQL, contradicting R6. **R7 reversed**: the TS mirror narrowed to formatters only.            |
| 2026-08-19 20:20 | §10.3 Stack Rationale added (R1–R8, each labelled *Inherited* vs *Chosen*). §13 replaced by a pointer to IMPLEMENTATION_PLAN.md so the two cannot drift.                          |
| 2026-08-19 20:04 | Moved into `docs/` alongside the handoff and prototype.                                                                                                                          |
| 2026-08-19 19:55 | **D10 corrected** — switching *parks* a block rather than terminating it, and returning resumes the remainder. Without this, switching away and back re-granted a full block, letting one task consume unlimited time. Anti-loophole tests 14–15 added; D1 scoped to blocks that ran their course. |
| 2026-08-19 19:40 | **D10/D11** — mid-block task switching. Resolved §9's `Enter — Start selected task` contradicting §5.2, which had no transition for it.                                            |
| 2026-08-19 19:20 | **D7–D9** — breaks as time blocks with no task, reusing the existing state machine rather than adding a mode.                                                                     |
| 2026-08-19 14:30 | Initial version — D1–D6, data model, timer state machine, §11 edge cases, acceptance tests 1–12.                                                                                  |

---

## 1. Product Definition

TimeBox is a native macOS menu bar utility that enforces **time allocation across multiple tasks**, rather than continuous focus on one.

**Central principle:**

> The timer controls how long you work on a task, not whether the task is completed.

When an allocated block expires, the app enters a blocking checkpoint and forces a conscious decision. It never silently advances.

**Non-goals for MVP:** cloud sync, accounts, team features, calendar integration, project hierarchies, tags, recurring tasks, rich analytics, iOS companion.

**Explicit anti-goal:** this is not a Pomodoro app. There are no fixed 25/5 cycles and no automatic breaks.

---

## 2. Core Domain Concepts

### 2.1 Task
A unit of work that may span multiple time blocks. A task is completed **only** when the user explicitly marks it done — never by timer expiry.

### 2.2 Time Block
A fixed allocation of working time assigned to one task. A block completes when its allocated time is consumed (or when skipped/cancelled). Block completion has **no** bearing on task completion.

### 2.3 Queue
An ordered list of tasks awaiting time blocks. The queue is the "playlist for work." Rotation through the queue is the product.

---

## 3. Resolved Design Decisions

These were ambiguous in the handoff and are now fixed:

| # | Decision | Resolution |
|---|---|---|
| D1 | Block duration when a pending task re-enters the queue | **Same as the task's `blockDurationSeconds`** (set at task creation). Extensions apply to a single block only and do not change `blockDurationSeconds`. **Scope:** this governs blocks that *ran their course* and rotated. A block set down mid-flight is not re-granted — see D10. |
| D2 | Skip behavior | Ends the current block with status `SKIPPED`, records actual elapsed duration, keeps the task pending, moves the task to the **end** of the queue, and starts the next task immediately. No expiration checkpoint is shown. |
| D3 | Day scope | **One persistent queue.** Unfinished tasks remain in it across days; nothing is reset at midnight. Statistics and the daily summary are scoped to a calendar day (local timezone) derived from time block timestamps. |
| D4 | Paused time | Does **not** consume block time. The block's end timestamp is recomputed on resume. |
| D5 | Empty queue at decision time | If no other task is queued, "Complete & Start Next" and "Keep Pending & Start Next" transition to `IDLE` with an empty-queue message rather than restarting the same task. |
| D10 | Switching tasks mid-block | Allowed from `RUNNING` and `PAUSED` via `switchTo(taskId)`. The block is **parked, not terminated**: it goes to `PAUSED` holding `remainingWhenPausedSeconds`, stays attached to its task, and the task rotates to the **queue tail**. Returning to the task **resumes that same block with its remaining time** — a fresh allocation is never granted. Without this, switching away at 29:00 of a 30:00 block and returning would re-grant a full 30:00, making unlimited time on one task reachable without ever hitting a checkpoint. |
| D11 | Switching is surfaced | Each block carries `interruptions: number`, incremented every time it is set down. Today shows `Switched early` = the sum across the day, warning-tinted above 2. This is a sharper signal than counting terminated blocks: it says *this one allocation was picked up and put down four times*. Never blocked — same treatment as extensions. |
| ~~D14~~ | ~~Quitting with a block running~~ | **Superseded by D16 (2026-08-24)** — quitting parks the block, so the confirm has nothing left to warn about and is deleted. The original text: Quit shows a confirm: *"A 30-minute block is running. Quitting won't pause it."* with **Pause & Quit** (default) / **Quit** / **Cancel**. It prevents nothing — it makes the cost of a thoughtless quit visible, the same treatment extensions and switching get. Not shown when `IDLE`, `PAUSED`, or at a checkpoint (quitting there is already safe: the checkpoint is restored). |
| D12 | First-run discoverability | `LSUIElement` leaves no Dock icon and no Cmd-Tab entry, and menu bar items can be hidden entirely by the notch. Two mitigations: relaunching an already-running instance **opens the popover** rather than silently focusing nothing, and a one-time first-run panel points at the menu bar. Without the first, a second double-click reads as a broken app. |
| D13 | Time at an unanswered checkpoint | The clock stops at `AWAITING_DECISION`, so time between expiry and the decision is neither work nor break. It is **surfaced, never guessed at**: the checkpoint shows `This block ended 2h 14m ago` once the gap exceeds 2 minutes, and Today carries an `Away` line. The open checkpoint's gap is derived live from the expiry timestamp; each answered one is banked on the block (`away_ms`) because it cannot be reconstructed afterwards. Either way there is no idle detection, and no retroactive crediting of the gap to any task. |
| D7 | Break as a checkpoint option | A break is a **time block with no task** (`kind: 'BREAK'`). **Break is a modifier on the task decision, not a substitute for it.** Both task decisions can be followed by a break, as single compound actions: `Complete & Break` and `Keep Pending & Break`. Break duration is selected *before* the action (a segmented control defaulting to `defaultBreakDurationSeconds`) so every compound action stays one click. |
| D8 | End of a break | A break expiring re-enters `AWAITING_DECISION` with a break-specific checkpoint (`Start <next task>` / `Extend Break`). It never auto-starts the next task — the user may be away from the desk. |
| D9 | Break accounting | Break time is excluded from "worked" totals and from per-task time. It is reported separately as `On break`. Breaks do **not** consume `availableWorkMinutesPerDay` in the capacity strip — capacity measures *work* the day can hold, and rest is not work. Confirmed 2026-08-19. |
| D6 | Single-task queue | If the current task is the *only* task, "Keep Pending & Start Next" moves it to the back of a one-item queue, so it becomes current again — but a **new block** is created and the checkpoint is dismissed. This is an explicit, logged decision, not an auto-continue. |

---

## 4. Data Model

Business logic is authored in TypeScript for the UI layer and mirrored in Rust for the persistence/state-machine layer. The **Rust side is the source of truth** for timer state.

### 4.1 Task

```typescript
type TaskStatus = 'TODO' | 'IN_PROGRESS' | 'DONE' | 'CANCELLED';
type Priority = 'LOW' | 'MEDIUM' | 'HIGH';

interface Task {
  id: string;                     // uuid v4
  title: string;
  description?: string;
  status: TaskStatus;
  priority: Priority;             // default 'MEDIUM'
  blockDurationSeconds: number;   // per-block allocation (D1)
  queuePosition: number | null;   // null when not queued (DONE/CANCELLED)
  createdAt: string;              // ISO 8601 UTC
  completedAt?: string;
  cancelledAt?: string;
}
```

`IN_PROGRESS` is set the first time a block for the task starts. It never reverts to `TODO`.

### 4.2 TimeBlock

```typescript
type TimeBlockStatus =
  | 'PLANNED' | 'RUNNING' | 'PAUSED'
  | 'AWAITING_DECISION'
  | 'COMPLETED' | 'SKIPPED' | 'CANCELLED';

type TimeBlockKind = 'WORK' | 'BREAK';

interface TimeBlock {
  id: string;
  kind: TimeBlockKind;      // 'BREAK' blocks have taskId === null
  taskId: string | null;

  plannedDurationSeconds: number;   // copied from task.blockDurationSeconds
  extensionSeconds: number;         // cumulative extensions on THIS block
  interruptions: number;            // times this block was set down mid-flight (D11)
  actualDurationSeconds?: number;   // wall time actually worked, excludes pauses

  startedAt?: string;               // ISO 8601 UTC, first start
  endedAt?: string;                 // ISO 8601 UTC, terminal transition

  status: TimeBlockStatus;

  // Timer arithmetic fields (see §6)
  endAt?: string;                   // projected expiry, recomputed on resume/extend
  pausedAt?: string;
  remainingWhenPausedSeconds?: number;
  accumulatedActiveSeconds: number; // running total of non-paused time
}
```

`plannedDurationSeconds + extensionSeconds` = total allocation for the block.

A `BREAK` block uses the identical timer fields and the identical state machine. It is not a separate mode — which is why breaks survive sleep, quit, and crash with no extra recovery code.

### 4.3 AppState (singleton row)

```typescript
type TimerState = 'IDLE' | 'RUNNING' | 'PAUSED' | 'AWAITING_DECISION';

interface AppState {
  timerState: TimerState;
  currentTaskId: string | null;
  currentBlockId: string | null;
  updatedAt: string;
}
```

### 4.4 Settings

```typescript
interface Settings {
  launchAtLogin: boolean;              // default false
  theme: 'SYSTEM' | 'LIGHT' | 'DARK';  // default 'SYSTEM'
  defaultBlockDurationSeconds: number; // default 1800 (30m)
  defaultBreakDurationSeconds: number; // default 600 (10m), pre-highlighted at the checkpoint
  expirationSound: boolean;            // default true
  systemNotification: boolean;         // default true
  availableWorkMinutesPerDay: number;  // default 420 (7h), used for capacity display
  menuBarShowTimer: boolean;           // default true
}
```

### 4.5 Persistence

SQLite via `tauri-plugin-sql` (bundled SQLite, no external dependency), stored at
`~/Library/Application Support/xyz.gigiheristiawan.timebox/timebox.db`.

Tables: `tasks`, `time_blocks`, `app_state` (single row, id=1), `settings` (single row, id=1), `schema_migrations`.

Durations and instants are stored in **milliseconds**, matching the domain core exactly; second-granularity columns would force a lossy conversion on every save and accumulate the rounding as real drift across pause/resume cycles. The `…Seconds` field names in §4.4 and §4.1 name the concept, not the column.

Requirements:
- All writes are synchronous and committed before the corresponding UI transition is acknowledged.
- Every state-machine transition writes to SQLite in a single transaction.
- WAL mode enabled; `synchronous=NORMAL`.
- Forward-only numbered migrations run at startup.

No account, no network calls, no telemetry.

---

## 5. Timer State Machine

Owned by Rust. React never mutates timer state directly — it invokes commands and subscribes to emitted events.

### 5.1 States

```
IDLE                — no active block
RUNNING             — block counting down
PAUSED              — block held, clock not consuming allocation
AWAITING_DECISION   — block allocation exhausted, user must choose
```

### 5.2 Transitions

| From | Event | To | Side effects |
|---|---|---|---|
| IDLE | `start(taskId)` | RUNNING | **If a parked block exists for the task, resume it** (`endAt = now + remainingWhenPaused`); otherwise create a block (PLANNED→RUNNING), `endAt = now + planned`. Task → IN_PROGRESS |
| RUNNING | `pause()` | PAUSED | `pausedAt = now`; `remainingWhenPaused = endAt - now`; accumulate active time |
| PAUSED | `resume()` | RUNNING | `endAt = now + remainingWhenPaused`; clear `pausedAt` |
| RUNNING | `tick` where `now >= endAt` | AWAITING_DECISION | Block → AWAITING_DECISION; fire expiration effects (§8) |
| RUNNING \| PAUSED | `switchTo(taskId)` | RUNNING | Current block → parked (`PAUSED`, `remainingWhenPausedSeconds` stored, `interruptions += 1`); task → queue tail; then `start(taskId)` (D10) |
| IDLE | `switchTo(taskId)` | RUNNING | Equivalent to `start(taskId)` |
| AWAITING_DECISION (BREAK) | `switchTo(taskId)` | RUNNING | Break block → `COMPLETED` (cut short, no interruption recorded); new block starts for `taskId` |
| RUNNING \| PAUSED | `skip()` | RUNNING or IDLE | Block → SKIPPED with `actualDuration`; task → back of queue; start next (D2) |
| RUNNING \| PAUSED | `completeTask()` | RUNNING or IDLE | Block → COMPLETED with `actualDuration`; task → DONE; start next |
| AWAITING_DECISION | `decideComplete()` | RUNNING or IDLE | Block → COMPLETED; task → DONE, dequeued; start next |
| AWAITING_DECISION | `decidePending()` | RUNNING or IDLE | Block → COMPLETED; task stays IN_PROGRESS, moved to queue tail; start next |
| AWAITING_DECISION | `decideExtend(seconds)` | RUNNING | `extensionSeconds += seconds`; `endAt = now + seconds`; block → RUNNING |
| AWAITING_DECISION (WORK) | `decideBreak(seconds, taskDecision)` | RUNNING | Work block → COMPLETED; task resolved per `taskDecision` (`Complete` → DONE and dequeued, `Pending` → queue tail); new `BREAK` block starts (D7) |
| AWAITING_DECISION (BREAK) | `endBreak()` | RUNNING or IDLE | Break block → COMPLETED; start next queued task, or IDLE if queue empty |
| AWAITING_DECISION (BREAK) | `extendBreak(seconds)` | RUNNING | `extensionSeconds += seconds`; `endAt = now + seconds`; block → RUNNING |
| any | `stop()` | IDLE | Block → CANCELLED; task remains queued in place |

`switchTo` is rejected from a **work** `AWAITING_DECISION` — the checkpoint has no side doors. It is accepted from a **break** `AWAITING_DECISION`, where picking a specific task simply answers the "what next" question the break checkpoint is already asking.

**Invariant:** exactly one block is ever `RUNNING`. *Parked* blocks (status `PAUSED`, not the current block) may exist for other tasks — at most one per task — each holding its own remainder. Recovery stays a single code path because only the current block has a live `endAt`.

`PAUSED` on the current block means "the clock is held, I am coming back to this shortly"; `switchTo` means "I am setting this down and working elsewhere". Both preserve the remainder; they differ only in whether the queue moves on.

The checkpoint's option set is a function of `currentBlock.kind`, not a new state. `BREAK` blocks reuse `RUNNING` / `PAUSED` / `AWAITING_DECISION` unchanged.

**AWAITING_DECISION is terminal until an explicit decision is made.** There is no dismiss, close, later, or continue action, and no timeout that resolves it.

### 5.3 Testability

The state machine is a pure module (`core/timerMachine.rs` + a TypeScript mirror `src/core/timerMachine.ts` for UI-side prediction) with:
- No I/O, no timers, no Tauri imports.
- Signature: `reduce(state: MachineState, event: MachineEvent, now: Instant) -> (MachineState, Effect[])`
- Fully unit-testable by injecting `now`. All nine acceptance tests in §12 must be expressible as pure reducer tests plus a thin integration layer.

No boolean flag soup. `isRunning`, `isPaused`, `hasExpired` etc. are derived, never stored.

---

## 6. Timer Accuracy

**Timestamps are the source of truth; ticks are only for rendering.**

- Remaining time is always computed as `endAt - now`. Never decremented.
- `endAt` is stored as an absolute UTC instant and persisted immediately on every start/resume/extend.
- The Rust tick loop runs at 1 Hz while `RUNNING` and is suspended entirely in `IDLE`, `PAUSED`, and `AWAITING_DECISION` (zero background work when not running).
- On every tick, on window focus, on app resume, and on macOS wake, the app re-evaluates `now >= endAt`.
- macOS sleep/wake: subscribe to `NSWorkspace.didWakeNotification`. On wake, immediately re-evaluate expiry before painting any UI. *(As built this is not subscribed to at all: the tick thread sleeps against wall time, so a wake produces a late tick that resolves expiry on its own. **Pending D21** that late tick gains a second job — a gap far exceeding the tick interval is evidence of sleep, and the block is parked at the instant the Mac went to sleep rather than credited with the gap. See `docs/features/SLEEP_DETECTION.md`.)*
- On app launch, hydrate from SQLite and re-evaluate expiry **before** first render. A block whose persisted `endAt` is in the past resolves to `AWAITING_DECISION`, never to a running or reset timer.
- If a stored `PAUSED` block is loaded, remaining time is `remainingWhenPausedSeconds` — wall clock elapsed while paused is irrelevant.

`actualDurationSeconds` is computed from accumulated active (non-paused) wall time, capped at total allocation for naturally-expired blocks. The cap is what keeps a block reopened after three days from reporting three days of work.

**Quitting parks the block (D16, landed 2026-08-24).** Quitting *is* a pause: the exit path dispatches the existing `Event::Pause`, the block ends `PAUSED` holding its remainder, and the interval the app is closed is recorded as idle rather than as work (`docs/features/IDLE_TIME.md` §3). An interval cannot be both idle and worked, which is what forced the reversal.

The loophole the old rule guarded against does not reopen. Parking preserves the **remainder** and never re-grants an allocation, so quitting at 29:00 of a 30:00 block leaves one minute, exactly as the Pause button does. `end_at` remains absolute and is never decremented; it is simply recomputed on the next resume, like any other park.

Closing the main *window* is still not quitting (§7.3): it hides the window and the timer runs on. **Sleep is not yet covered** — until D21 lands, a Mac that sleeps mid-block still has the gap credited as work, bounded by the block's allocation cap.

> **Reversed 2026-08-24 — the rule this replaced, kept for its reasoning.** *"Quitting does not stop the clock. A `RUNNING` block continues to consume its allocation while the app is not running, exactly as it does across sleep. This is deliberate and follows the same reasoning as D10: if quitting paused the allocation, quitting would become a way to bank time — close the app at 29:00 of a 30:00 block, reopen tomorrow, and still hold 29 minutes — letting one task consume unlimited time without ever reaching a checkpoint."* The anti-gaming concern was real and is answered above: parking holds the remainder rather than re-granting a block. What the old rule got wrong was the measurement, not the loophole.

---

## 7. Application Surfaces

### 7.1 Menu Bar Icon (primary)

Title text reflects state, updated at most 1 Hz:

| State | Title |
|---|---|
| RUNNING (work) | `◉ 24:17` (mm:ss; `h:mm:ss` above 1h) |
| RUNNING (break) | `◔ BREAK 04:12` |
| PAUSED | `◉ PAUSED` |
| AWAITING_DECISION | `⚠ TIME'S UP` |
| IDLE | icon only |

If `menuBarShowTimer` is false, show the icon only and put the timer in the popover. Icon is a template image so it adapts to light/dark menu bars automatically.

### 7.2 Popover (click menu bar icon)

Compact, ~320px wide:
- Current task title + large countdown
- Next task preview
- `Pause` / `Skip` buttons
- Divider
- Today's queue (first 4–5 entries, current marked `→`)
- Divider
- `Open App`, `Settings`, `Quit`

Normal daily usage must be fully possible from the popover alone.

### 7.3 Main Window

Opened on demand; closing it does **not** quit the app (`NSApplication` activation policy `.accessory`, window close hides).

Sections:
- **Current task** — title, priority marker, countdown ring, `Pause` / `Skip` / `Complete`, extension badge if `extensionSeconds > 0`; `✎` opens an inline editor for the **title, priority and added time**
- **Up next** — ordered queue with duration, drag-and-drop reordering (persists `queuePosition`). **Clicking a row, or `↑`/`↓` then `Return`, switches to that task immediately** (D10). A task holding a parked block shows its **remaining time rather than its full allocation** (`12:34 left`), is tinted, and its affordance reads `Resume ▶` instead of `Start ▶` — so the queue never implies time the task no longer has. `✎` on a row edits the **title and priority** in place; a blank title is refused whole. Time is granted in the editor the way the checkpoint grants it — `+5/+10/+15`, **added and never assigned**: an allocation that could be typed over would let a running block be shortened into an early checkpoint, or a parked one cut below the remainder already promised. A grant lands on the task's allocation *and* on its live block; at a checkpoint the block is left alone, since `Extend` is how a checkpoint grants time and this must not be a way around answering it
- **Capacity strip** — available / allocated / remaining, with over-capacity shown but never blocked
- **Add task** — inline row and `Cmd+K` quick add
- **Today** — completed count, pending count, total worked, **time on break**, **time away** (D13), top 3 tasks by time
- **Break state** — while a break block runs, the current-task panel is replaced by a break panel showing the countdown, the next queued task, `+5 min`, and `End break & start <next>`

### 7.4 Expiration Checkpoint Window

A separate always-on-top, borderless, screen-filling window on the active display.

- Blocks interaction with TimeBox's own UI only. It must **never** attempt to lock the Mac or block other applications.
- Contents: `TIME'S UP`, task title, block duration worked, prompt, then a break-length selector and exactly five actions.
- **Staleness line** — when more than 2 minutes have passed since the block expired, the checkpoint states how long it has been waiting (`This block ended 2h 14m ago`). The decision is often different when the block ended hours ago, and the user must not have to infer that.

- **Break length** — a segmented control `5 / 10 / 15 / 30`, pre-set to `defaultBreakDurationSeconds`. It selects, it does not act. Its only job is to make the two break actions below single-click.

- **Actions** — the task decision is the row; the transition is the verb:

  | | then start next | then take a break |
  |---|---|---|
  | **Complete** | `✓ Complete & Start Next` `1` | `✓ Complete & Break 10m` `2` |
  | **Keep pending** | `→ Keep Pending & Start Next` `3` `⏎` **default** | `→ Keep Pending & Break 10m` `4` |

  plus `+ Extend Time` `5`, which expands to `+5 / +10 / +15 / Custom`.

  Break buttons carry the selected duration in their label, so the action is never ambiguous. Extend has no break pairing — extending means continuing this task now.
- Shows extension history for this task today when non-zero: `Original 30m · Extended +20m · ⚠ You've already extended this task today.`
- No dismiss/close/escape path. `Esc` is a no-op. The window has no close button. Cmd+W is disabled while this window is key.

**Break variant** (shown when the expired block is a `BREAK`):

- Kicker reads `BREAK'S OVER`; headline is the *next queued task*, not the finished one.
- Two actions: `▶ Start <next task>` (recommended, `Return`) and `+ Extend Break` (`+5 / +10 / +15`).
- If the queue is empty the first action reads `▶ Finish for now` and transitions to `IDLE`.
- Same no-exit rule. A break that ends while the user is away holds at the checkpoint indefinitely and records no work time.
- Uses the rest accent (cool blue) rather than the alert red — a finished break is not an alarm.

---

## 8. Expiration Effects

On entering `AWAITING_DECISION`:

1. Persist the state transition (so a crash preserves it — Test 7).
2. Show the expiration window on the display containing the mouse cursor.
3. `NSApp.activate(ignoringOtherApps: true)` and make the expiration window key.
4. Play a subtle system sound (`Glass`) if `expirationSound`.
5. Post a macOS user notification if `systemNotification`:
   `TIME'S UP — <task title> — Your <n>-minute time block has ended. Choose what to do next.`
6. Update menu bar title to `⚠ TIME'S UP`.
7. Suspend the tick loop.

For a `BREAK` block the same sequence runs with break copy (`BREAK'S OVER — Your 10-minute break has ended.`) and a softer sound (`Ping`). Window activation still applies — a break that silently expired would defeat the point.

**Quitting (D16, was D14).** There is no confirmation. Quit parks a running block and exits; the park is written before the process goes away. The confirm this replaces existed to make the cost of quitting visible, and D16 removed the cost.

**Relaunch and first run (D12).** A second launch of an already-running instance opens the popover rather than merely focusing an invisible app. On the very first run only, a panel points at the menu bar item and explains that TimeBox has no Dock icon; it is dismissible and never shown again.

The app requests notification authorization on first run. If denied, all other effects still apply — the app must remain functional without notification permission.

---

## 9. Keyboard Shortcuts

**Global (registered app-wide):**
- `Cmd+Shift+T` — toggle popover

**Within app windows:**
| Key | Action |
|---|---|
| `Space` | Pause / Resume |
| `N` | New task |
| `S` | Skip current block |
| `D` | Complete current task |
| `↑` / `↓` | Navigate task list |
| `Enter` | Start selected task — invokes `switchTo` (D10), not just `start` |
| `Cmd+K` | Quick add |
| `Cmd+,` | Settings |

**In the work expiration window only:** `1` = Complete & Start, `2` = Complete & Break, `3` = Keep Pending & Start, `4` = Keep Pending & Break, `5` = Extend, `Return` = `3` (default). Nothing dismisses it.

**In the break expiration window only:** `1` / `Return` = Start next, `2` = Extend break. Nothing dismisses it.

---

## 10. Technology Stack

```
Tauri 2            — shell, menu bar, windows, notifications, lifecycle
Rust               — timer state machine, persistence, macOS integration
React 18 + TS      — UI
Vite               — build
Zustand            — UI state (mirrors backend, never authoritative for the timer)
Tailwind CSS       — styling
rusqlite (bundled)  — SQLite, accessed from Rust only
```

Additional Tauri plugins: `notification`, `autostart`, `global-shortcut`, `single-instance`. There is deliberately no SQL plugin — the webview cannot reach the database.

Electron is explicitly rejected. Target idle memory < 80 MB, idle CPU ≈ 0% when `IDLE`/`PAUSED`.

Rationale, provenance, and known reservations for each choice are in §10.3.

### 10.1 Project Structure

```
src/                       React UI
  components/              TaskList, CurrentTask, Timer, Queue, Expiration, Settings
  stores/                  Zustand stores (task, timer, settings)
  core/                    Formatters + countdown interpolation ONLY (no React imports,
                           no decision logic — see R7)
  ipc/                     Typed wrappers over Tauri commands/events
src-tauri/
  src/
    core/timer_machine.rs  Pure reducer, no I/O
    core/queue.rs          Pure queue operations
    db/                    Migrations + repositories
    platform/              Menu bar, notifications, window mgmt, wake observer
    commands.rs            Tauri command surface
```

Business logic must not import React. The state machine must be testable without rendering.

**`src/core/` is deliberately thin.** It contains time formatting (`clockStr`, `durStr`) and countdown interpolation from a backend-supplied `endAt` — nothing else. No transitions, no queue mutation, no decision rules. Every rule that could drift lives in Rust alone (§10.3 R7). If the UI appears to need a decision implemented client-side, that is a signal the Rust command surface is missing a command, not a reason to add logic here.

### 10.2 Packaging

- Universal binary (`aarch64` + `x86_64`) via `--target universal-apple-darwin`
- Proper `.app` bundle with a real app icon and `LSUIElement = true`
- Developer ID signing + notarization in the release pipeline
- Ad-hoc/unsigned builds are for local development only, never the recommended production artifact

### 10.3 Stack Rationale

**Provenance matters when reading this section.** Most of the stack was specified by the original handoff document under "Recommended Technology," not derived from the requirements in this spec. Choices are labelled accordingly, so a future reader knows which ones were actually reasoned about and which were inherited.

| # | Choice | Origin | Rationale |
|---|---|---|---|
| R1 | Tauri 2 over Electron | **Inherited** | Electron bundles Chromium: ~120 MB shipped, 100–200 MB resident at idle. Tauri uses the system WKWebView — ~10 MB shipped and far lower idle memory. For a utility running all day in the menu bar, idle footprint is the dominant cost, so this fits the app shape. Costs: Safari engine quirks instead of Chromium, a smaller ecosystem, and tray APIs thinner than AppKit's. |
| R2 | React 18 + TypeScript + Vite | **Inherited** | Uncontroversial. React's reconciliation is load-bearing here rather than incidental: the HTML prototype re-rendered by replacing `innerHTML` each frame, which destroyed rows between mousedown and mouseup and silently swallowed every click on the queue. Reconciliation prevents exactly that class of bug. |
| R3 | Zustand | **Inherited** | Fits because the store is a *mirror* of backend state, not a source of truth. Redux's ceremony would buy nothing over a mirror; plain React Context would cause re-render storms on a 1 Hz tick. |
| R4 | Tailwind CSS | **Inherited** | The weakest-justified choice. The design is built on CSS custom properties for theming (§ the three-state theme rule), and CSS modules over those tokens would be equally good and arguably cleaner. Retained because it was specified and nothing depends on the difference. |
| R5 | SQLite via `rusqlite` (`bundled` feature) | **Chosen** | The handoff allowed "SQLite or another lightweight local persistence layer." SQLite was picked for Test 7 (quit while awaiting a decision → still awaiting on relaunch), which requires every transition durably committed before the UI acknowledges it. A JSON file rewritten wholesale can tear on a crash mid-write; WAL-mode SQLite gives atomicity for free. Daily rollups (top 3 tasks by time) are also natural in SQL. Fair counter-argument: at a few hundred rows per year, an append-only event log would suffice with less machinery. **Corrected during Phase 1:** originally specified as `tauri-plugin-sql`, which exists to let the *frontend* run SQL. That contradicts R6 — if Rust owns persistence, the database must not be reachable from the webview at all. Replaced with `rusqlite`, which also lets the migration runner (§4.5) live in Rust where it belongs. |
| R6 | **Rust owns the timer state machine** | **Chosen** | The most consequential decision in §10. A webview can be throttled or suspended when hidden and is effectively dead across sleep, but the timer must stay correct through exactly those conditions. `NSWorkspace.didWakeNotification` is a native API, and persistence must happen even when the webview is not running. The machine therefore has to live in the process that stays alive. Everything in §6 follows from this. |
| R7 | TypeScript mirror of the state machine | **Chosen, then reversed** | Originally specified as a pure TS mirror for UI-side prediction. Reversed before Phase 2: a mirror would place the product's subtlest rules (block parking, the anti-farming guarantee of D10) in two languages, free to drift. **Resolved:** `src/core/` is narrowed to formatters and countdown interpolation from `endAt`. All decision logic lives in Rust alone. The UI renders backend state and never predicts it. |
| R8 | Plugin list, `LSUIElement`, universal binary, signing | **Chosen** | Mechanical consequences of "menu bar utility that must survive sleep and ship outside the App Store." |

**Considered and closed: Swift + SwiftUI instead of Tauri.**
For a macOS-only menu bar utility, native is a defensible — arguably better — fit. Tauri's principal advantages are a web UI and cross-platform reach, and this app spends neither. SwiftUI/AppKit would provide `NSStatusItem`, `NSWorkspace` wake notifications, `UNUserNotifications`, and activation policy as first-party APIs rather than through plugins, and the timer logic would live in one language instead of two (see R7).

Not adopted, for three reasons: the handoff specified Tauri explicitly; the app is small enough that either stack succeeds; and the deciding factor in practice is which language the implementer is fluent in, which favors TypeScript here. Recorded because the constraint that was actually reasoned about in the handoff was *"avoid Electron"* — Tauri versus native was never weighed, and a future reader should know that rather than assume it was.

---

## 11. Edge Cases

| Case | Behavior |
|---|---|
| Mac sleeps past `endAt` | On wake, resolve to `AWAITING_DECISION` before rendering |
| App quit while `AWAITING_DECISION` | Restored on next launch, still awaiting decision |
| App quit while `RUNNING` | Parked `PAUSED` at the instant of the quit, holding its remainder (D16). The gap is `idle_paused_ms`, never worked time — so the restart time no longer changes the outcome |
| Force-quit or crash while `RUNNING` | No park was written: the block is still `RUNNING` and resolves on hydrate as before (`AWAITING_DECISION` if `endAt` has passed). The gap is credited as work, capped at the allocation |
| Force-quit or crash while an idle span is open | The span comes back **open** and keeps accruing — paused is still paused whether or not the app is alive (D15) |
| App quit while `PAUSED` | Restored `PAUSED` with identical remaining time |
| Queue emptied while running | On decision, transition to `IDLE` with empty-state prompt (D5) |
| Current task deleted | Block → `CANCELLED`, transition to `IDLE` |
| Current task is the only task | See D6 |
| Timezone / DST change | All instants stored UTC; only day-bucketing for stats uses local time |
| Clock moved backwards | `endAt` still absolute; if `now < startedAt` treat elapsed as 0 rather than negative |
| Multiple app instances | `single-instance` plugin focuses the existing instance |
| Switch to the task already running | No-op; no interruption is recorded |
| Switch attempted at a work checkpoint | Rejected — the decision must be made first |
| Switch during a break | Break ends as `COMPLETED` (cut short), no interruption recorded |
| Resuming a block with under 30s left | Goes straight to `AWAITING_DECISION` rather than flashing a token countdown — the allocation is spent either way |
| Task completed or deleted while holding a parked block | Parked block closes (`COMPLETED` on task completion, `CANCELLED` on delete) with its accumulated active time |
| Switch immediately after starting a block | Records an interruption with near-zero elapsed time — the count is what matters, not the duration |
| Parked block still parked at app quit | Restored as parked with its exact remainder; only the current block re-evaluates expiry on launch |
| Break taken with an empty queue | Break runs normally; at its checkpoint the only action is `Finish for now` → `IDLE` |
| Mac sleeps through a break | Same recovery path as a work block — resolves to the break checkpoint on wake |
| Quit during a break | Break block restored with correct remaining time, or its checkpoint if it expired |
| Break extended repeatedly | Allowed and tracked; `On break` total is the visible feedback, no cap is enforced |
| Quit while `RUNNING` | No confirm. The block is parked with its remainder (D16) |
| Quit while `PAUSED` or at a checkpoint | No confirm — that state is restored exactly |
| Checkpoint answered within 2 minutes of expiry | No staleness line; the gap is below the reporting floor |
| Checkpoint left unanswered overnight | Staleness line shows the full elapsed time; `Away` accrues; no time is credited to any task |
| Menu bar item hidden by the notch | First-run panel and relaunch-opens-popover remain the recovery paths |
| Notifications denied | Sound + window activation still fire |
| Extension of 0 or negative | Rejected at the command boundary |
| Task title empty | Add-task rejected with inline validation |

---

## 12. Acceptance Tests

Each must be automated as a reducer test where possible, plus a manual verification pass on the built `.app`.

1. **Expiration** — A(1m), B(1m); start A; after 1m: state = `AWAITING_DECISION`, A not DONE, expiration window visible, B not started.
2. **Complete** — at expiry, `Complete & Start Next` → A = DONE, B = RUNNING.
3. **Pending** — at expiry, `Keep Pending & Start Next` → A still TODO/IN_PROGRESS, B RUNNING, queue = B→C→…→A.
4. **Extend** — at expiry, `+10 min` → A RUNNING with 10:00 remaining, checkpoint dismissed, `extensionSeconds = 600`.
5. **App minimized** — timer continues, expiration triggers, app activates, checkpoint visible.
6. **Mac sleep** — 5m timer, sleep 10m, wake → `AWAITING_DECISION`, timer not reset.
7. **Restart while awaiting decision** — quit without deciding, relaunch → still `AWAITING_DECISION`.
8. **Manual completion independent of timer** — 30m task completed manually at 10m → task DONE, block COMPLETED, `actualDurationSeconds ≈ 600`.
9. **Block completion ≠ task completion** — expire a 30m block, choose Pending → block COMPLETED, task NOT DONE. *(Highest-priority criterion.)*

Additional regression tests:
10. Pausing for 10 minutes does not consume block time (D4).
11. Skip records elapsed duration, marks block SKIPPED, rotates task to queue tail (D2).
12. A re-queued pending task's next block equals `blockDurationSeconds`, ignoring prior extensions (D1).
13. **Mid-block switch parks the block** — start A (30m), after 5m switch to C → A's block is `PAUSED` with `remainingWhenPausedSeconds ≈ 1500`, `interruptions = 1`, A still `IN_PROGRESS` and NOT done, queue = C→B→D→A, C running a **fresh** 30m block.
14. **Return resumes the remainder** *(the anti-loophole test)* — from Test 13, switch back to A → A resumes at **25:00, not 30:00**, and `blocks.filter(b => b.taskId === A).length === 1`. A second block for A must never be created by switching.
15. **Switching cannot farm time** — start a 30m block and switch away/back ten times → total time available to that task before its checkpoint is still 30 minutes, and `interruptions = 10`.
16. **Switch is not a skip** — after a switch and a skip, stats report one parked block with `interruptions = 1` and one separate `SKIPPED` block.
17. **Keep Pending & Break** — at expiry choose it → work block `COMPLETED`, task NOT done, queue = B→C→…→A, a `BREAK` block runs for the selected length.
17b. **Complete & Break** — at expiry choose it → task `DONE` and dequeued, work block `COMPLETED`, a `BREAK` block runs. On the break checkpoint, `Start <next>` names the next queued task, not the completed one.
18. **Break does not auto-advance** — let the break expire → state = `AWAITING_DECISION` on the break block, next task NOT started, no work time accruing.
19. **Break accounting** — after a completed break, `worked` excludes the break duration, `On break` includes it, and no task's per-task total changed.
20. **Staleness is reported** — let a block expire, wait 3 minutes, then look at the checkpoint → it states the block ended ~3 minutes ago; answering it credits no time to the task for that gap, and Today's `Away` increases by ~3 minutes.
21. **Relaunch opens the popover** — with the app already running and no window visible, launching it again opens the popover rather than doing nothing visible.
22. **Break survives sleep/restart** — start a 5-minute break, sleep 10 minutes → break checkpoint on wake, not a reset break.

---

## 13. Implementation Order

The phased plan, task breakdown, per-task status, acceptance-test coverage map, and open questions live in **[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)** — track progress there, not here.

Summary of the sequence:

1. Foundation → 2. Domain core → 3. Persistence & recovery → 4. Main window UI →
5. Expiration checkpoint → 6. Menu bar → 7. Polish → 8. Release

Two rules hold across every phase:

- Do not build everything in one change. Each phase ends with tests, `tsc --noEmit`, a Tauri build, and manual verification on the built `.app`.
- Correctness of the timer and state machine takes priority over visual polish throughout.
