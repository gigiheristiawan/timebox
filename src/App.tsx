import { useEffect } from "react";
import { useTimebox, isBreak } from "./stores/useTimebox";
import { CurrentPanel } from "./components/CurrentPanel";
import { RotationStrip } from "./components/RotationStrip";
import { Queue } from "./components/Queue";
import { AddTask } from "./components/AddTask";
import { PendingDecision } from "./components/PendingDecision";

const BREAK_MINUTES = 10; // From settings in Phase 7.

export default function App() {
  const { snap, error, init, send } = useTimebox();

  useEffect(() => {
    let un: (() => void) | undefined;
    void init().then((u) => { un = u; });
    return () => un?.();
  }, [init]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && /^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName)) return;
      const state = useTimebox.getState().snap?.state.timerState;

      if (state === "AwaitingDecision") {
        const onBreakNow = isBreak(useTimebox.getState().snap);
        const ms = BREAK_MINUTES * 60_000;
        if (onBreakNow) {
          if (e.key === "1" || e.key === "Enter") void send({ kind: "endBreak" });
          if (e.key === "2") void send({ kind: "extendBreak", ms: 5 * 60_000 });
          return;
        }
        if (e.key === "1") void send({ kind: "decideComplete" });
        if (e.key === "2") void send({ kind: "decideBreak", ms, complete: true });
        if (e.key === "3" || e.key === "Enter") void send({ kind: "decidePending" });
        if (e.key === "4") void send({ kind: "decideBreak", ms, complete: false });
        // Escape and everything else are deliberately inert.
        return;
      }

      if (e.key === " ") {
        e.preventDefault();
        void send({ kind: state === "Paused" ? "resume" : "pause" });
      }
      if (e.key === "s" || e.key === "S") void send({ kind: "skip" });
      if (e.key === "d" || e.key === "D") void send({ kind: "completeCurrentTask" });
      if (e.key === "n" || e.key === "N") {
        e.preventDefault();
        document.getElementById("new-task-title")?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [send]);

  return (
    <main className="mx-auto flex min-h-screen max-w-2xl flex-col gap-5 p-6">
      <header className="flex items-baseline gap-3">
        <h1 className="text-base font-semibold tracking-tight">TimeBox</h1>
        <span className="font-mono text-[10px] uppercase tracking-[0.17em] text-ink-3">
          {snap?.state.timerState ?? "…"}
        </span>
      </header>

      {error && <p className="rounded-lg bg-alert-soft px-4 py-3 text-sm text-alert">{error}</p>}

      <PendingDecision breakMinutes={BREAK_MINUTES} />
      <CurrentPanel />
      <RotationStrip />

      <hr className="border-line" />

      <Queue />
      <AddTask />
    </main>
  );
}
