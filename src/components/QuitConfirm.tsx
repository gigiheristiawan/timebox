import { useEffect } from "react";
import { durStr } from "../core/format";
import { cancelQuit, confirmQuit } from "../ipc/commands";
import { currentBlock, currentTask, isBreak, useTimebox } from "../stores/useTimebox";

/**
 * SPEC D14. This confirm prevents nothing — quitting with a block running is
 * allowed. It exists because `end_at` is absolute: the clock keeps consuming
 * the allocation while the app is closed, and that cost should be visible
 * rather than discovered tomorrow.
 *
 * `Pause & Quit` is the default, so `Return` is the safe answer.
 */
export function QuitConfirm() {
  const { snap, init } = useTimebox();

  useEffect(() => {
    let un: (() => void) | undefined;
    void init().then((u) => { un = u; });
    return () => un?.();
  }, [init]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter") void confirmQuit(true);
      if (e.key === "Escape") void cancelQuit();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const block = currentBlock(snap);
  const task = currentTask(snap);
  const what = snap && isBreak(snap) ? "break" : "block";
  const length = block ? durStr(block.plannedMs + block.extensionMs) : "";

  return (
    <main className="flex min-h-screen flex-col gap-3 bg-surface px-5 py-[18px]">
      <h1 className="text-[15px] font-semibold">
        A {length} {what} is running. Quitting won&apos;t pause it.
      </h1>
      <p className="text-[13px] leading-snug text-ink-2">
        {task ? `“${task.title}” ` : ""}keeps consuming its allocation while TimeBox is closed, exactly
        as it does while the Mac sleeps. Pausing first holds the remainder.
      </p>
      <div className="mt-auto flex justify-end gap-2">
        <button
          type="button"
          onClick={() => void cancelQuit()}
          className="rounded-[7px] border border-transparent px-[13px] py-1.5 text-[13px] font-medium text-ink-2 hover:bg-surface-3"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={() => void confirmQuit(false)}
          className="rounded-[7px] border border-line-2 bg-surface px-[13px] py-1.5 text-[13px] font-medium hover:bg-surface-3"
        >
          Quit
        </button>
        <button
          type="button"
          autoFocus
          onClick={() => void confirmQuit(true)}
          className="rounded-[7px] border border-accent bg-accent px-[13px] py-1.5 text-[13px] font-medium text-white hover:brightness-110"
        >
          Pause &amp; Quit
          <span className="ml-[5px] font-mono text-[10px] opacity-55">⏎</span>
        </button>
      </div>
    </main>
  );
}
