# macOS Task Rotation Timeboxing App

Build a lightweight **native macOS desktop productivity application** for task rotation and timeboxing.

The application is designed around a specific productivity problem:

> I can become deeply focused on one task and accidentally spend too much of my finite working hours on it, while other important tasks are neglected.

Traditional Pomodoro does not solve this well because it encourages continuous focus on one task.

This application should instead enforce **time allocation across multiple tasks**.

The core philosophy is:

> **The timer controls how long I work on a task, not whether the task is completed.**

When the allocated time expires, the application must force me to make a conscious decision about what happens next.

---

# Platform

The application is specifically for:

**macOS**

It should behave like a lightweight Mac utility rather than a conventional large desktop application.

## Primary UX

The application should:

- Run as a macOS menu bar application.
- Have a menu bar icon.
- Be accessible from the macOS menu bar.
- Open a compact popover/window when clicking the menu bar icon.
- Be minimizable/closable without terminating the application.
- Continue timers while the main UI is not visible.
- Show macOS desktop notifications where appropriate.
- Support dark/light mode.
- Support keyboard shortcuts where practical.
- Launch without requiring a large application window.

The application should feel similar to a lightweight menu-bar utility.

---

# Recommended Technology

Prefer:

```text
Tauri 2
React
TypeScript
Vite
Zustand
Tailwind CSS
SQLite or another lightweight local persistence layer
```

Use Rust/Tauri for native macOS functionality.

Avoid Electron unless there is a strong technical reason to use it.

The final application should be lightweight and have low CPU/memory usage while running in the background.

---

# Core Concept

There are two separate concepts:

## Task

A piece of work that may require multiple time blocks.

Example:

```text
Fix attendance calculation bug
```

A task can remain unfinished after its allocated time expires.

## Time Block

A fixed allocation of working time assigned to a task.

Example:

```text
Fix attendance calculation bug
Allocated time: 30 minutes
```

The user works for 30 minutes.

When the 30 minutes expire:

```text
Time block = COMPLETED
Task = STILL UNFINISHED
```

This distinction is extremely important.

A task may have multiple time blocks:

```text
Fix attendance calculation bug

30 min
30 min
15 min
```

The task is only completed when the user explicitly marks it as complete.

---

# Example Workflow

The user starts the day with:

```text
Task A — Fix API bug            30 min
Task B — Payroll calculation    45 min
Task C — Code review             30 min
Task D — Client proposal        45 min
Task E — Documentation           30 min
```

The user starts Task A.

After 30 minutes:

```text
TIME'S UP
```

The application does NOT silently start Task B.

Instead, it blocks the application UI and asks the user to make a decision.

The user can:

```text
1. Complete Task A → Start Task B
2. Keep Task A pending → Start Task B
3. Extend Task A → Continue working
```

If the user chooses option 2:

```text
Task A → remains unfinished

Task B → starts
```

Later, Task A can return to the queue.

The intended workflow is:

```text
A → B → C → D → A → B → C
```

rather than:

```text
A → work on A until completely finished
```

---

# macOS Menu Bar

The application should primarily live in the macOS menu bar.

Example:

```text
┌─────────────────────────────┐
│  ◉ TimeBox                  │
│                             │
│  Current                    │
│  Fix attendance bug         │
│                             │
│          24:17              │
│                             │
│  Next                       │
│  Payroll calculation        │
│                             │
│  [ Pause ]   [ Skip ]       │
│                             │
│  ─────────────────────────  │
│                             │
│  Today's Tasks              │
│                             │
│  → Fix attendance bug       │
│    Payroll calculation      │
│    Code review              │
│    Client proposal          │
│                             │
│  ─────────────────────────  │
│  Open App                   │
│  Settings                   │
│  Quit                       │
└─────────────────────────────┘
```

Clicking the menu bar icon should reveal the current task, timer and queue.

The user should NOT need to open a large application window for normal usage.

---

# Menu Bar Icon

The menu bar should communicate timer state.

For example:

```text
Running:
◉ 24:17

Paused:
◉ PAUSED

Waiting for decision:
⚠ TIME'S UP
```

If showing text in the menu bar is practical, display the remaining time.

Otherwise use an icon and show the timer inside the popover.

---

# Main Application Window

The application should also have a larger window for task management.

The main window should contain:

```text
TODAY

CURRENT TASK

Fix attendance calculation bug

24:17 remaining

[ Pause ] [ Skip ] [ Complete ]

─────────────────────────────

UP NEXT

1. Payroll calculation       45m
2. Code review               30m
3. Client proposal           45m
4. Documentation             30m

─────────────────────────────

+ Add Task
```

Allow drag-and-drop reordering.

---

# Timer

The timer must be based on timestamps rather than a simple JavaScript decrement loop.

Do NOT rely on:

```typescript
setInterval(() => {
  remaining--;
}, 1000);
```

as the source of truth.

Instead store something like:

```typescript
startedAt
pausedAt
endAt
remainingWhenPaused
```

and calculate remaining time using actual timestamps.

For example:

```typescript
remaining = endAt - Date.now()
```

This is important because macOS applications can:

- go into the background
- be minimized
- lose focus
- sleep
- wake up
- experience event-loop delays

The timer must remain accurate.

---

# Timer States

Use an explicit state machine.

Minimum states:

```text
IDLE
RUNNING
PAUSED
AWAITING_DECISION
```

Example:

```text
IDLE
 ↓
RUNNING
 ↓
Timer reaches zero
 ↓
AWAITING_DECISION
```

From `AWAITING_DECISION`:

```text
COMPLETE
   ↓
NEXT TASK
   ↓
RUNNING
```

or:

```text
PENDING
   ↓
NEXT TASK
   ↓
RUNNING
```

or:

```text
EXTEND
   ↓
RUNNING
```

Do not implement this with a large collection of boolean flags.

---

# Critical Feature: Expiration Checkpoint

When the timer reaches zero, the application must enter:

```text
AWAITING_DECISION
```

It must NOT automatically advance.

The user must explicitly decide what to do.

---

# Full-Screen Expiration UI

When the timer expires, display a **full-screen modal/window within the application**.

The user must not be able to interact with the normal application UI until they make a decision.

Example:

```text
┌───────────────────────────────────────────────────────────────┐
│                                                               │
│                                                               │
│                        TIME'S UP                              │
│                                                               │
│                Fix attendance calculation                    │
│                                                               │
│                 Your 30-minute block is over.                │
│                                                               │
│             What do you want to do with this task?            │
│                                                               │
│                                                               │
│        ┌─────────────────────────────────────────┐            │
│        │ ✓ Complete & Start Next                 │            │
│        └─────────────────────────────────────────┘            │
│                                                               │
│        ┌─────────────────────────────────────────┐            │
│        │ → Keep Pending & Start Next             │            │
│        └─────────────────────────────────────────┘            │
│                                                               │
│        ┌─────────────────────────────────────────┐            │
│        │ + Extend Time                           │            │
│        └─────────────────────────────────────────┘            │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

There must be no generic:

```text
Dismiss
Close
Later
Continue
```

button.

The user must make a conscious decision.

---

# Important macOS Behavior

The expiration checkpoint should be highly visible even if the main application was minimized.

When a timer expires:

1. Bring the application's expiration window to the foreground.
2. Make the application window active.
3. Display the full-screen expiration UI.
4. Play a subtle notification sound.
5. Optionally send a macOS notification.
6. Require explicit user action.

Do NOT attempt to lock the entire Mac or prevent interaction with unrelated applications.

The blocking behavior applies to the application itself.

If the user is working in another application, the app should use normal macOS window activation/notification mechanisms to bring attention to the expiration checkpoint.

---

# Expiration Option 1 — Complete

Button:

```text
✓ Complete & Start Next
```

Behavior:

1. Mark the current task as `DONE`.
2. Mark the current time block as `COMPLETED`.
3. Record actual duration.
4. Remove the task from the active queue.
5. Start the next task.
6. Close the expiration checkpoint.

This action explicitly means:

> The work is finished.

---

# Expiration Option 2 — Pending

Button:

```text
→ Keep Pending & Start Next
```

This should be the recommended/default action.

Behavior:

1. Keep the task unfinished.
2. Mark the current time block as `COMPLETED`.
3. Record actual duration.
4. Keep task status as `TODO` or `IN_PROGRESS`.
5. Move the task to the end of the queue.
6. Start the next task.
7. Close the expiration checkpoint.

Example:

```text
Before:

A → B → C → D

A expires.

After:

B → C → D → A
```

The user has made progress on A without allowing A to consume unlimited time.

---

# Expiration Option 3 — Extend

Provide:

```text
+5 min
+10 min
+15 min
Custom
```

Behavior:

1. Keep the current task active.
2. Add the selected duration.
3. Resume the timer.
4. Close the expiration checkpoint.

Track extensions.

Example:

```text
Task:
Fix attendance bug

Original:
30 minutes

Extensions:
+10 minutes
+5 minutes

Total:
45 minutes
```

Do not automatically complete the task.

The user must still explicitly complete it.

---

# Prevent Abuse of Extend

The application should make extensions visible.

For example:

```text
Original allocation: 30m
Extended: +20m

⚠ You've already extended this task today.
```

Do not prevent extensions.

The purpose is not to police the user.

The purpose is to make the user consciously acknowledge:

> "I am choosing to spend more time on this."

---

# Task Model

Suggested model:

```typescript
type TaskStatus =
  | 'TODO'
  | 'IN_PROGRESS'
  | 'DONE'
  | 'CANCELLED';

interface Task {
  id: string;
  title: string;
  description?: string;
  status: TaskStatus;
  priority?: 'LOW' | 'MEDIUM' | 'HIGH';
  estimatedMinutes?: number;
  createdAt: string;
  completedAt?: string;
}
```

---

# Time Block Model

```typescript
type TimeBlockStatus =
  | 'PLANNED'
  | 'RUNNING'
  | 'PAUSED'
  | 'COMPLETED'
  | 'SKIPPED'
  | 'CANCELLED';

interface TimeBlock {
  id: string;
  taskId: string;

  plannedDurationSeconds: number;
  actualDurationSeconds?: number;

  startedAt?: string;
  endedAt?: string;

  status: TimeBlockStatus;

  extensionSeconds: number;
}
```

---

# Queue

Maintain an explicit task queue.

Example:

```text
[
  taskA,
  taskB,
  taskC,
  taskD
]
```

When a task expires and the user chooses Pending:

```text
[
  taskB,
  taskC,
  taskD,
  taskA
]
```

The queue order must persist across application restarts.

---

# Daily Planning

The user should be able to see today's workload.

Example:

```text
TODAY

Available working time
7h 00m

Allocated
6h 30m

Remaining
30m
```

If the user allocates more time than available:

```text
Available: 7h
Allocated: 8h 15m

Over capacity: 1h 15m
```

Make this visible but do not prevent the user from doing it.

The user may intentionally over-plan.

---

# Daily Summary

Show:

```text
TODAY

Total worked       4h 20m
Allocated          6h 30m

Tasks completed    3
Tasks pending      5

Most time spent:
1. Payroll          1h 30m
2. HRIS attendance  1h 10m
3. API bug            45m
```

Keep analytics simple for MVP.

---

# Task Creation

Task creation must be fast.

Example:

```text
+ Add Task

Title:
[ Fix attendance API              ]

Duration:
[ 30 min ▼ ]

Priority:
[ Medium ▼ ]

[ Add ]
```

Keyboard shortcut:

```text
Cmd + K
```

for quick task creation if practical.

---

# Multiple Time Blocks

A task may receive multiple time blocks.

Example:

```text
Fix attendance API

Today:

30m completed
30m completed
15m planned

Total allocated: 75m
Total worked:    60m
```

The task remains unfinished until explicitly completed.

---

# Keyboard Shortcuts

Suggested:

```text
Space       Pause / Resume
N           New Task
S           Skip current block
D           Complete current task
↑ / ↓       Navigate tasks
Enter       Start selected task
Cmd + K     Quick Add
```

Make shortcuts configurable later if needed.

---

# Notifications

When the timer expires:

- Play a subtle sound.
- Activate the expiration window.
- Show a macOS notification if appropriate.

Example:

```text
TIME'S UP

Fix attendance API

Your 30-minute time block has ended.
Choose what to do next.
```

---

# Persistence

The application must persist all important state locally.

Persist:

- Tasks
- Task statuses
- Queue order
- Time blocks
- Timer state
- Current task
- Expiration state
- Daily history
- Settings

The application must not require an account or server.

---

# Application Restart

The application must correctly recover from:

- App quit
- App crash
- Mac sleep
- Mac wake
- User logout/login

Example:

```text
Task A
30-minute block

Started:
10:00

Mac sleeps:
10:10

Mac wakes:
10:40
```

The application must determine that the timer expired while the Mac was asleep.

It should enter:

```text
AWAITING_DECISION
```

rather than incorrectly showing remaining time.

---

# Menu Bar States

The menu bar should reflect the current application state.

Examples:

```text
RUNNING

◉ 24:31
```

```text
PAUSED

◉ PAUSED
```

```text
AWAITING_DECISION

⚠ TIME'S UP
```

If practical, dynamically update the menu bar timer.

Do not sacrifice battery/CPU efficiency to update it excessively frequently.

---

# Settings

MVP settings:

```text
General

☐ Launch at login

Theme:
○ System
○ Light
○ Dark

Timer

Default task duration:
[ 30 min ]

Expiration sound:
[ On ]

Automatic notification:
[ On ]
```

Do not add unnecessary configuration.

---

# Native macOS Requirements

The app should:

- Be a proper `.app`.
- Have a proper application icon.
- Support Apple Silicon.
- Ideally support Intel Macs if practical.
- Use macOS menu bar APIs through Tauri.
- Use native desktop notifications.
- Support application activation when the timer expires.
- Store local data securely and reliably.
- Avoid unnecessary background CPU usage.

If possible, configure the project for proper Apple code signing and notarization during release.

Do not distribute an unsigned ad-hoc application as the recommended production build.

---

# Architecture

Separate the application into:

```text
UI
 ├── Task List
 ├── Current Task
 ├── Timer
 ├── Queue
 ├── Expiration Checkpoint
 └── Settings

State
 ├── Task State
 ├── Timer State
 ├── Queue State
 └── Application State

Persistence
 ├── Tasks
 ├── Time Blocks
 ├── Settings
 └── Daily History

Native
 ├── Menu Bar
 ├── Notifications
 ├── Window Management
 ├── App Activation
 └── macOS Lifecycle
```

Keep business logic independent from React UI components.

The timer/state machine should be testable without rendering the UI.

---

# Timer State Machine

Implement this explicitly.

```text
                    ┌──────────────┐
                    │     IDLE     │
                    └──────┬───────┘
                           │
                         START
                           │
                           ▼
                    ┌──────────────┐
              ┌────►│    RUNNING   │
              │     └──────┬───────┘
              │            │
            RESUME       EXPIRES
              │            │
              │            ▼
              │     ┌─────────────────┐
              └─────│     PAUSED      │
                    └─────────────────┘
                           │
                           │ timer expires
                           ▼
                 ┌─────────────────────┐
                 │ AWAITING_DECISION   │
                 └───────┬─────────────┘
                         │
           ┌─────────────┼─────────────┐
           │             │             │
        COMPLETE       PENDING       EXTEND
           │             │             │
           ▼             ▼             ▼
       NEXT TASK      NEXT TASK      RUNNING
           │             │
           └──────┬──────┘
                  ▼
               RUNNING
```

Correct this diagram if necessary during implementation, but preserve the underlying behavior.

---

# Critical Acceptance Tests

## Test 1 — Expiration

Create:

```text
Task A = 1 minute
Task B = 1 minute
```

Start A.

After one minute:

Expected:

```text
Application state = AWAITING_DECISION
Task A = NOT DONE
Expiration screen = visible
Task B = NOT STARTED
```

The app must NOT automatically start B.

---

## Test 2 — Complete

At expiration:

Click:

```text
Complete & Start Next
```

Expected:

```text
Task A = DONE
Task B = RUNNING
```

---

## Test 3 — Pending

At expiration:

Click:

```text
Keep Pending & Start Next
```

Expected:

```text
Task A = TODO/IN_PROGRESS
Task B = RUNNING
Queue:
B → C → ... → A
```

---

## Test 4 — Extend

At expiration:

Click:

```text
+10 min
```

Expected:

```text
Task A = RUNNING
Timer = 10 minutes
```

The expiration checkpoint disappears and the timer resumes.

---

## Test 5 — App minimized

Start a timer.

Minimize/hide the application.

Wait until expiration.

Expected:

- Timer continues.
- Expiration state is triggered.
- Application becomes active or otherwise clearly alerts the user.
- Expiration screen is shown.

---

## Test 6 — Mac sleep

Start a 5-minute timer.

Put Mac to sleep for 10 minutes.

Wake Mac.

Expected:

```text
AWAITING_DECISION
```

The app must NOT reset the timer.

---

## Test 7 — App restart while awaiting decision

Allow a timer to expire.

Do not make a decision.

Quit the application.

Restart it.

Expected:

```text
AWAITING_DECISION
```

The user must still be forced to decide what happens to the task.

---

## Test 8 — Task completion does not depend on timer

Create a 30-minute task.

Complete it manually after 10 minutes.

Expected:

```text
Task = DONE
Time block = COMPLETED
Actual duration = approximately 10 minutes
```

---

## Test 9 — Time block completion does not mean task completion

Allow a 30-minute time block to expire.

Choose Pending.

Expected:

```text
Time block = COMPLETED
Task = NOT DONE
```

This is one of the most important acceptance criteria.

---

# Product Philosophy

Do not turn this into another generic Pomodoro application.

The core product is:

> **A workday task rotation system that prevents one task from consuming unlimited time.**

Think of it as a **playlist for work**.

Example:

```text
30m  → Fix API
45m  → Payroll
30m  → Code review
30m  → Fix API
45m  → Client proposal
30m  → Documentation
```

The application should continuously force conscious decisions about where the user's limited working time goes.

The timer is not asking:

> "Did you finish?"

It is asking:

> **"You allocated this much time. What should happen next?"**

That distinction is the central design principle.

---

# Development Process

Before implementing:

1. Inspect the repository/project structure.
2. Confirm the Tauri/macOS architecture.
3. Design the data model.
4. Design the timer state machine.
5. Identify macOS-specific APIs required.
6. Propose the UI structure.
7. Identify edge cases.
8. Then implement incrementally.

Do not build everything in one huge change.

After each major feature:

1. Run tests.
2. Run TypeScript checks.
3. Run the Tauri build.
4. Verify the macOS application manually.

Prioritize correctness of the timer and state machine over visual polish.

The application should be usable as soon as the MVP is complete.