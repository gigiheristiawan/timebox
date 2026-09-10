import { useEffect } from "react";
import { durStr } from "../core/format";
import { currentTask, isBreak, useTimebox } from "../stores/useTimebox";
import { Countdown } from "./Countdown";

/**
 * The floating timer overlay (issue #23, `docs/features/TIMER_OVERLAY.md`).
 *
 * A read-only card: it shows what is running and how much is left, and offers
 * no control at all — the window ignores the cursor, so a button here could
 * never be pressed. Which states it appears in, where it sits and how solid it
 * looks are all decided in Rust from the stored settings; this only paints.
 *
 * The card fills the window, whose size Rust fixes from
 * `overlayShowTaskTitle`. Nothing here may change its own height: the window is
 * anchored to a screen corner, so a card that grew would drift out of it.
 */
export function Overlay() {
  const { snap, init } = useTimebox();

  useEffect(() => {
    let un: (() => void) | undefined;
    void init().then((u) => { un = u; });
    return () => un?.();
  }, [init]);

  const state = snap?.state.timerState;
  const onBreak = isBreak(snap ?? null);
  const task = currentTask(snap ?? null);
  const paused = state === "Paused";
  // Nothing running. The overlay stays up and says so rather than vanishing:
  // the state it is most useful in is the one where the user has drifted off
  // the queue entirely, and a card that disappeared would take the reminder
  // with it (D47).
  //
  // `!task` alone is not "nothing running": a break block deliberately carries
  // no task, so a running break read as idle and the card showed today's idle
  // total in place of the break countdown (issue #27).
  const idle = !snap || state === "Idle" || (!task && !onBreak);

  const label = onBreak ? "Break" : paused ? "Paused" : idle ? "Idle" : "Focus";
  const tone = onBreak ? "text-rest" : paused || idle ? "text-ink-3" : "text-ink";
  const showTitle = snap?.settings.overlayShowTaskTitle ?? true;
  // Today's idle total, computed by `core::summary` like every other figure the
  // UI shows. The clock is not interpolated here — it moves in minutes, and the
  // store refetches every 10s while the timer is stopped.
  const idleMs = snap?.summary.today.idleMs ?? 0;
  const sub = onBreak ? "Break time" : idle || !task ? "Nothing running" : task.title;

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center overflow-hidden rounded-[12px] border border-line bg-surface px-3">
      <div className="font-mono text-[9px] uppercase tracking-[0.18em] text-ink-3">{label}</div>
      {idle ? (
        <span className={`tabular text-[30px] font-medium leading-none tracking-[-0.02em] ${tone}`}>
          {durStr(idleMs)}
        </span>
      ) : (
        <Countdown
          className={`text-[30px] font-medium leading-none tracking-[-0.02em] ${tone}`}
        />
      )}
      {showTitle && (
        <div className="mt-1.5 w-full truncate text-center text-[12px] text-ink-2" title={sub}>
          {sub}
        </div>
      )}
    </div>
  );
}
