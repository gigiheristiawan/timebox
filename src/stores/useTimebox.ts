import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { dispatch as ipcDispatch, getSnapshot, updateSettings } from "../ipc/commands";
import type { Action, Settings, Snapshot, Task, TimeBlock } from "../ipc/types";
import { applyTheme } from "../theme";

/** How often to refetch while the timer is stopped and the backend is silent.
 *  Slow on purpose: the numbers it moves are minutes, not seconds. */
const STOPPED_REFRESH_MS = 10_000;

interface Store {
  snap: Snapshot | null;
  error: string | null;
  /** backendNow - clientNow, so the UI can interpolate against the backend's
   *  clock instead of the webview's. */
  clockSkew: number;
  refresh: () => Promise<void>;
  send: (action: Action) => Promise<void>;
  /** Written whole; the stored result comes back, which may be clamped. */
  saveSettings: (settings: Settings) => Promise<void>;
  init: () => Promise<() => void>;
}

export const useTimebox = create<Store>((set, get) => ({
  snap: null,
  error: null,
  clockSkew: 0,

  refresh: async () => {
    try {
      const snap = await getSnapshot();
      applyTheme(snap.settings.theme);
      set({ snap, clockSkew: snap.now - Date.now(), error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  send: async (action) => {
    try {
      const snap = await ipcDispatch(action);
      set({ snap, clockSkew: snap.now - Date.now(), error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  saveSettings: async (settings) => {
    try {
      const snap = await updateSettings(settings);
      applyTheme(snap.settings.theme);
      set({ snap, clockSkew: snap.now - Date.now(), error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  init: async () => {
    await get().refresh();
    // The backend nudges once a second while a block runs; between nudges the
    // countdown is interpolated locally.
    const un = await listen("timebox://changed", () => void get().refresh());

    // While the timer is NOT running there are no nudges at all: the tick
    // thread parks on a condvar in IDLE / PAUSED / AWAITING_DECISION, which is
    // what keeps idle CPU at nil. But that is precisely when `idleMs` and
    // `stalenessMs` grow, so an open window would sit on a frozen number until
    // the user's next action. This is a refresh cadence, not a rule — the UI
    // still derives nothing; it asks Rust again (SPEC R6/R7). Interpolating
    // locally is not an option either way: idle is a set difference over
    // `idle_spans`, which the snapshot deliberately does not carry.
    const poll = window.setInterval(() => {
      if (get().snap?.state.timerState !== "Running") void get().refresh();
    }, STOPPED_REFRESH_MS);

    return () => {
      un();
      window.clearInterval(poll);
    };
  },
}));

// ------------------------------------------------------------- selectors
// Derived views only. Nothing here decides anything.

export function currentBlock(s: Snapshot | null): TimeBlock | null {
  if (!s?.state.currentBlockId) return null;
  return s.state.blocks.find((b) => b.id === s.state.currentBlockId) ?? null;
}

export function currentTask(s: Snapshot | null): Task | null {
  const b = currentBlock(s);
  if (!b?.taskId) return null;
  return s!.state.tasks.find((t) => t.id === b.taskId) ?? null;
}

export function taskById(s: Snapshot | null, id: string): Task | undefined {
  return s?.state.tasks.find((t) => t.id === id);
}

/** The block holding a task's remainder after a switch (SPEC D10). */
export function parkedFor(s: Snapshot | null, taskId: string): TimeBlock | null {
  if (!s) return null;
  return (
    s.state.blocks.find(
      (b) =>
        b.taskId === taskId &&
        b.status === "Paused" &&
        b.id !== s.state.currentBlockId,
    ) ?? null
  );
}

/** What a queued task will actually get: its remainder if parked, else a full block. */
export function queuedMs(s: Snapshot | null, taskId: string): number {
  const parked = parkedFor(s, taskId);
  if (parked?.remainingWhenPausedMs != null) return parked.remainingWhenPausedMs;
  return taskById(s, taskId)?.blockDurationMs ?? 0;
}

export function isBreak(s: Snapshot | null): boolean {
  return currentBlock(s)?.kind === "Break";
}

/** Defaults for a surface that renders before the first snapshot arrives. */
export const FALLBACK_BREAK_MS = 10 * 60_000;

export function breakDefaultMs(s: Snapshot | null): number {
  return s?.settings.defaultBreakDurationMs ?? FALLBACK_BREAK_MS;
}
