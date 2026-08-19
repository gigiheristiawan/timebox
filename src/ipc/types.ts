/** Mirrors the serde types on the Rust side. Data shapes only, never behavior. */

export type TaskStatus = "Todo" | "InProgress" | "Done" | "Cancelled";
export type Priority = "Low" | "Medium" | "High";
export type TimerState = "Idle" | "Running" | "Paused" | "AwaitingDecision";
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

export interface Snapshot {
  state: MachineState;
  now: number;
  remainingMs: number;
  stalenessMs: number | null;
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
  | { kind: "addTask"; title: string; blockMs: number; priority: Priority }
  | { kind: "removeTask"; task: string }
  | { kind: "reorder"; moved: string; before: string };
