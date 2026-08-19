import { useEffect, useState } from "react";
import { clockStr, remainingMs } from "../core/format";
import { useTimebox, currentBlock } from "../stores/useTimebox";

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
