/**
 * The ONLY logic permitted on the TypeScript side (SPEC §10.1, R7).
 * Formatting and countdown interpolation. No transitions, no queue mutation,
 * no decision rules — those live in Rust alone so the product's subtle
 * guarantees cannot drift between two languages.
 */

/** mm:ss, or h:mm:ss past an hour. Never negative. */
export function clockStr(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

/** Human duration for allocations: "45m", "1h", "1h 30m". */
export function durStr(ms: number): string {
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.round((total % 3600) / 60);
  if (h === 0) return `${m}m`;
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/**
 * Remaining milliseconds against an absolute instant supplied by the backend.
 * Interpolation only — the backend remains the authority on whether a block has
 * expired. A UI that reaches 00:00 waits for the backend's transition rather
 * than concluding expiry itself.
 */
export function remainingMs(endAt: number, skew: number, now = Date.now()): number {
  return Math.max(0, endAt - (now + skew));
}

export const MIN = 60_000;

/** "Mon", from an instant the backend supplied. Formatting, not arithmetic —
 *  which day it *is* was decided in Rust (WEEKLY_REPORT D37). */
export function weekdayStr(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, { weekday: "short" });
}

/** "Sep 1". */
export function dateStr(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
