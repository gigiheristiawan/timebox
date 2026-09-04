import { useEffect, useState } from "react";
import { clockStr, remainingMs } from "../core/format";
import { useTimebox, currentBlock, isBreak } from "../stores/useTimebox";

/**
 * Interpolates between the backend's once-a-second nudges so the digits move
 * smoothly. It never decides that time is up — when it reaches zero it simply
 * shows zero and waits for the backend's transition.
 */
export function Countdown({ className = "" }: { className?: string }) {
  const { snap, clockSkew } = useTimebox();
  const block = currentBlock(snap);
  const [, tick] = useState(0);

  useEffect(() => {
    if (snap?.state.timerState !== "Running") return;
    const id = setInterval(() => tick((n) => n + 1), 250);
    return () => clearInterval(id);
  }, [snap?.state.timerState]);

  if (!snap || snap.state.timerState === "Idle" || !block) {
    return <span className={`tabular ${className}`}>--:--</span>;
  }

  const ms =
    snap.state.timerState === "Running" && block.endAt != null
      ? remainingMs(block.endAt, clockSkew)
      : snap.remainingMs;

  return <span className={`tabular ${className}`}>{clockStr(ms)}</span>;
}

/**
 * The Pomodoro clock, beside the task clock (issue #15). Renders nothing when
 * the mode is off.
 *
 * It counts *running work*, so unlike the task countdown it has no `endAt` to
 * interpolate against — it parks whenever the timer is not RUNNING, which is
 * exactly when the backend stops sending fresh values. Interpolating a parked
 * clock would show it draining while no work is happening, so this ticks only
 * while RUNNING and otherwise shows the backend's number as given.
 *
 * Nothing is rendered during a break: "break in 24:40" while a break is
 * running is redundant at best, and actively wrong at worst — the clock resets
 * when the break *ends* (D29), so the number shown would be the stale pre-break
 * one, counting down to a break already being taken.
 */
export function PomodoroCountdown({ className = "" }: { className?: string }) {
  const { snap } = useTimebox();
  const [, tick] = useState(0);
  const running = snap?.state.timerState === "Running";

  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [running]);

  if (!snap?.pomodoro || isBreak(snap)) return null;
  const drift = running ? Date.now() - snap.now : 0;
  const ms = Math.max(0, snap.pomodoro.remainingMs - drift);
  return <span className={`tabular ${className}`}>{clockStr(ms)}</span>;
}
