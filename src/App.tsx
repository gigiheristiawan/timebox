import { useEffect, useState } from "react";
import { useTimebox, breakDefaultMs, isBreak } from "./stores/useTimebox";
import { openSettingsWindow } from "./ipc/commands";
import { CurrentPanel } from "./components/CurrentPanel";
import { RotationStrip } from "./components/RotationStrip";
import { Queue } from "./components/Queue";
import { AddTask } from "./components/AddTask";
import { PendingDecision } from "./components/PendingDecision";
import { Capacity } from "./components/Capacity";
import { Today } from "./components/Today";
import { FirstRun } from "./components/FirstRun";
import { QuickAdd } from "./components/QuickAdd";

export default function App() {
  const { error, init, send } = useTimebox();
  const [quickAdd, setQuickAdd] = useState(false);

  useEffect(() => {
    let un: (() => void) | undefined;
    void init().then((u) => { un = u; });
    return () => un?.();
  }, [init]);

  // SPEC §9. Every shortcut sends the same Action a click would; none of them
  // is a second path to a decision the backend does not already own.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      const typing = el && /^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName);

      // Command shortcuts work while typing; the plain letters below do not.
      if (e.metaKey && e.key === ",") { e.preventDefault(); void openSettingsWindow(); return; }
      if (e.metaKey && e.key.toLowerCase() === "k") { e.preventDefault(); setQuickAdd(true); return; }
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return;

      const snap = useTimebox.getState().snap;
      const state = snap?.state.timerState;

      if (state === "AwaitingDecision") {
        const ms = breakDefaultMs(snap);
        if (isBreak(snap)) {
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

      // ↑/↓ move into the queue, where each row owns the rest of the
      // navigation and `Return` switches to it (D10).
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        const rows = document.querySelectorAll<HTMLElement>("[data-row]");
        const target = e.key === "ArrowDown" ? rows[0] : rows[rows.length - 1];
        if (!target) return;
        e.preventDefault();
        target.focus();
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
    // `.win-body` in docs/mockup.html. No in-window title: the native title bar
    // already names the app, and the prototype's body starts at Current task.
    <main className="flex min-h-screen flex-col gap-[18px] px-5 pb-5 pt-[18px]">
      {error && <p className="rounded-lg bg-alert-soft px-4 py-3 text-sm text-alert">{error}</p>}

      <FirstRun />
      <PendingDecision />
      <CurrentPanel />
      <RotationStrip />

      <hr className="border-line" />

      <Queue />
      <AddTask />

      <hr className="border-line" />

      <Capacity />

      <hr className="border-line" />

      <Today />

      <QuickAdd open={quickAdd} onClose={() => setQuickAdd(false)} />
    </main>
  );
}
