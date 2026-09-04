import { useTimebox, currentBlock, isBreak, parkedFor, queuedMs, taskById } from "../stores/useTimebox";
import { remainingMs } from "../core/format";
import { SectionLabel } from "./ui";

/**
 * The day as a playlist: each queued task is a segment sized by the time it
 * will actually get, and the current one fills as it burns. A parked task shows
 * its remainder, not its full allocation, so the strip never implies time a
 * task no longer has.
 */
export function RotationStrip() {
  const { snap, clockSkew } = useTimebox();
  const block = currentBlock(snap);
  if (!snap) return null;

  // A daily already ticked off today is not part of what is left to do
  // (issue #16), so it is not a segment of the remaining rotation.
  const ids = snap.state.queue.filter((id) => !snap.doneToday.includes(id)).slice(0, 7);
  const onBreak = isBreak(snap);
  const breakMs = onBreak && block ? block.plannedMs + block.extensionMs : 0;
  const total = ids.reduce((n, id) => n + queuedMs(snap, id), 0) + breakMs;
  if (total <= 0) return null;

  const progress = (b: NonNullable<typeof block>) => {
    const alloc = b.plannedMs + b.extensionMs;
    if (alloc <= 0) return 0;
    const left = b.endAt != null && snap.state.timerState === "Running"
      ? remainingMs(b.endAt, clockSkew)
      : snap.remainingMs;
    return Math.min(100, ((alloc - left) / alloc) * 100);
  };

  return (
    <section className="flex flex-col gap-[7px]">
      <SectionLabel>Rotation</SectionLabel>
      <div className="flex h-[26px] overflow-hidden rounded-md border border-line bg-surface-2">
        {onBreak && block && (
          <Segment
            widthPct={(breakMs / total) * 100}
            fillPct={progress(block)}
            tone="rest"
            label={`◔ ${mins(breakMs)}m`}
          />
        )}
        {ids.map((id) => {
          const isCurrent = block?.taskId === id && snap.state.timerState !== "Idle";
          const t = taskById(snap, id);
          const ms = queuedMs(snap, id);
          return (
            <Segment
              key={id}
              widthPct={(ms / total) * 100}
              fillPct={isCurrent && block ? progress(block) : 0}
              tone={isCurrent ? "accent" : "idle"}
              parked={parkedFor(snap, id) != null}
              label={`${t?.title.split(" ")[0] ?? ""} · ${mins(ms)}m`}
            />
          );
        })}
      </div>
    </section>
  );
}

const mins = (ms: number) => Math.round(ms / 60_000);

/** `.seg-t` in docs/mockup.html. Parked is a marker on the segment, not a tone:
 *  a parked task is still an ordinary queued segment, underlined to show it
 *  carries a remainder rather than a full allocation. */
function Segment({
  widthPct, fillPct, tone, label, parked = false,
}: {
  widthPct: number;
  fillPct: number;
  tone: "accent" | "rest" | "idle";
  label: string;
  parked?: boolean;
}) {
  const bg =
    tone === "accent" ? "bg-accent-soft text-accent-ink"
    : tone === "rest" ? "bg-rest-soft text-rest-ink"
    : "text-ink-3";
  const fill = tone === "rest" ? "bg-rest" : "bg-accent";
  return (
    <div
      className={`relative flex min-w-[8px] items-center overflow-hidden whitespace-nowrap border-r border-line px-1.5 last:border-r-0 ${bg} ${
        parked ? "border-b-2 border-b-warn" : ""
      }`}
      style={{ width: `${widthPct}%` }}
      title={label}
    >
      {fillPct > 0 && (
        <div className={`absolute inset-y-0 left-0 opacity-30 ${fill}`} style={{ width: `${fillPct}%` }} />
      )}
      <span className="relative font-mono text-[10px]">{label}</span>
    </div>
  );
}
