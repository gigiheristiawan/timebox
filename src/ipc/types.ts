/** Mirrors the serde types on the Rust side. Data shapes only, never behavior. */

export type TaskStatus = "Todo" | "InProgress" | "Done" | "Cancelled";
export type Priority = "Low" | "Medium" | "High";
export type TimerState =
  | "Idle" | "Running" | "Paused" | "AwaitingDecision"
  /** The Pomodoro prompt is open (issue #15). A separate state from
   *  `AwaitingDecision` because the two checkpoints accept different actions —
   *  this one decides the break, never the task. */
  | "AwaitingPomodoro";
export type BlockKind = "Work" | "Break";
export type BlockStatus =
  | "Planned" | "Running" | "Paused" | "AwaitingDecision"
  | "Completed" | "Skipped" | "Cancelled";

export interface Task {
  id: string;
  title: string;
  status: TaskStatus;
  priority: Priority;
  blockDurationMs: number;
  /** Recurs every day (issue #16). A daily is never `Done` and never leaves
   *  the queue; `completedAt` records the *last* time it was ticked off. Use
   *  `Snapshot.doneToday` rather than comparing that against a date here. */
  daily: boolean;
  createdAt: number;
  completedAt: number | null;
}

export interface TimeBlock {
  id: string;
  kind: BlockKind;
  taskId: string | null;
  plannedMs: number;
  extensionMs: number;
  /** Times this block was set down mid-flight (SPEC D11). */
  interruptions: number;
  actualMs: number | null;
  status: BlockStatus;
  startedAt: number | null;
  endedAt: number | null;
  endAt: number | null;
  pausedAt: number | null;
  remainingWhenPausedMs: number | null;
  accumulatedActiveMs: number;
  lastResumeAt: number | null;
}

export interface MachineState {
  timerState: TimerState;
  tasks: Task[];
  blocks: TimeBlock[];
  queue: string[];
  currentBlockId: string | null;
}

export type Theme = "System" | "Light" | "Dark";

/** SPEC §4.4. Durations are milliseconds like everything else on the wire. */
export interface Settings {
  launchAtLogin: boolean;
  theme: Theme;
  defaultBlockDurationMs: number;
  defaultBreakDurationMs: number;
  expirationSound: boolean;
  systemNotification: boolean;
  availableWorkMsPerDay: number;
  menuBarShowTimer: boolean;
  /** The one-time panel pointing at the menu bar has been dismissed (D12). */
  firstRunDone: boolean;
  /** The working window: when the user asserts they are at the desk
   *  (IDLE_TIME §2). Milliseconds from *local* midnight, not instants.
   *  A different quantity from `availableWorkMsPerDay`, which is how much of
   *  the day they intend to give. */
  workStartMs: number;
  workEndMs: number;
  /** Which weekdays the window applies to. Bitmask, Monday = bit 0. */
  workingWeekdays: number;
}

/** All computed by `core::summary` in Rust. The UI formats these; it does not
 *  derive them (SPEC R7). */
export interface Capacity {
  availableMs: number;
  allocatedMs: number;
  /** Signed: negative means over capacity, which is shown but never blocked. */
  unallocatedMs: number;
  over: boolean;
}

export interface TopTask {
  taskId: string;
  title: string;
  ms: number;
}

export interface Today {
  workedMs: number;
  breakMs: number;
  /** Time at unanswered checkpoints — neither work nor break (D13).
   *  A sub-view of `idleAwaitingMs`. */
  awayMs: number;
  /** Working-window time that no running block covered (IDLE_TIME §3). */
  idleMs: number;
  /** The three causes; they sum to `idleMs` exactly. Shown in the report, not
   *  in the popover — they are not peers of the total. */
  idleAwaitingMs: number;
  idlePausedMs: number;
  idleUntrackedMs: number;
  /** Work done outside the window. A signal, never subtracted from anything. */
  outsideHoursMs: number;
  tasksCompleted: number;
  tasksPending: number;
  blocksCompleted: number;
  switchedEarly: number;
  top: TopTask[];
}

export interface Summary {
  today: Today;
  capacity: Capacity;
}

/** The weekly report (issue #6), from `core::report`. Fetched by its own
 *  command rather than carried on the snapshot: the snapshot is rebuilt every
 *  second, and a week of interval algebra does not belong there (D38). */
export interface DayReport {
  dayStart: number;
  /** 0 = Monday … 6 = Sunday. */
  weekday: number;
  /** A label only — the day is scored either way. */
  workingDay: boolean;
  /** Capacity on a working day, 0 otherwise. A real zero, not an absent
   *  target: work on a day off is over target (D36). */
  targetMs: number;
  workedMs: number;
  breakMs: number;
  idleMs: number;
  idleAwaitingMs: number;
  idlePausedMs: number;
  idleUntrackedMs: number;
  outsideHoursMs: number;
  tasksCompleted: number;
  blocksCompleted: number;
}

export interface WeekTotals {
  workedMs: number;
  breakMs: number;
  idleMs: number;
  idleAwaitingMs: number;
  idlePausedMs: number;
  idleUntrackedMs: number;
  outsideHoursMs: number;
  tasksCompleted: number;
  blocksCompleted: number;
  /** Over the blocks that *ended* in the week — not the sum of the days
   *  (D35). There is deliberately no per-day equivalent. */
  switchedEarly: number;
  /** Capacity × working weekdays. The whole week's, even mid-week (D36). */
  targetMs: number;
  workingDays: number;
  daysWorked: number;
}

export interface WeekReport {
  weekStart: number;
  /** Exclusive: the following Monday. */
  weekEnd: number;
  /** 0 = current week, -1 = last week. Never positive. */
  offset: number;
  isCurrentWeek: boolean;
  /** Always seven, Monday first, zeros included (D39). */
  days: DayReport[];
  /** Ranked over the week, not merged from the daily rankings (D35). */
  top: TopTask[];
  totals: WeekTotals;
}

export interface Snapshot {
  state: MachineState;
  now: number;
  remainingMs: number;
  stalenessMs: number | null;
  summary: Summary;
  settings: Settings;
  /** Whether macOS actually has the login item registered. Not the same as
   *  `settings.launchAtLogin`, which is only what the user asked for. */
  launchAtLoginActive: boolean;
  /** Ids of daily tasks already ticked off for today. Computed in Rust because
   *  local midnight is a shell concern and the UI does no date arithmetic. */
  doneToday: string[];
  /** Pomodoro mode, or `null` when it is off (issue #15). `remainingMs` counts
   *  down 25 minutes of *running work*, so it parks whenever the task timer
   *  does. Interpolate it against `now` like the task countdown, and never
   *  conclude it is due here — at zero, show zero and wait for the backend. */
  pomodoro: { remainingMs: number } | null;
}

export interface HealthReport {
  databasePath: string;
  schemaVersion: number;
  journalMode: string;
}

/** The complete set of things the UI may ask for. */
export type Action =
  | { kind: "switchTo"; task: string }
  | { kind: "pause" }
  | { kind: "resume" }
  | { kind: "skip" }
  | { kind: "completeCurrentTask" }
  | { kind: "decideComplete" }
  | { kind: "decidePending" }
  | { kind: "decideExtend"; ms: number }
  | { kind: "decideBreak"; ms: number; complete: boolean }
  | { kind: "endBreak" }
  | { kind: "extendBreak"; ms: number }
  | { kind: "startBreak"; ms: number }
  | { kind: "addTask"; title: string; blockMs: number; priority: Priority; daily: boolean }
  | { kind: "editTask"; task: string; title: string; priority: Priority; daily: boolean }
  | { kind: "addTime"; task: string; ms: number }
  | { kind: "decidePomodoroBreak"; ms: number }
  | { kind: "decideSkipPomodoro" }
  | { kind: "setPomodoroMode"; on: boolean }
  | { kind: "removeTask"; task: string }
  | { kind: "reorder"; moved: string; before: string };
