import { useRef, useState } from "react";
import { clockStr, durStr } from "../core/format";
import { useTimebox, currentBlock, parkedFor, taskById } from "../stores/useTimebox";
import { SectionLabel } from "./ui";

const PRIORITY_DOT = { High: "bg-alert", Medium: "bg-warn", Low: "bg-ink-3" } as const;

/**
 * Up Next. Clicking a row switches to that task immediately — the running block
 * is set down keeping its remaining time, not thrown away (SPEC D10). A task
 * holding a parked block shows that remainder rather than its full allocation.
 */
export function Queue() {
  const { snap, send } = useTimebox();
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  if (!snap) return null;
  const block = currentBlock(snap);
  const upNext = snap.state.queue.filter(
    (id) => !(block?.taskId === id && snap.state.timerState !== "Idle"),
  );

  const focusSibling = (from: HTMLElement, delta: number) => {
    const rows = Array.from(listRef.current?.querySelectorAll<HTMLElement>("[data-row]") ?? []);
    const i = rows.indexOf(from);
    rows[Math.max(0, Math.min(rows.length - 1, i + delta))]?.focus();
  };

  return (
    <section className="flex flex-col gap-2">
      <SectionLabel>Up next — click to switch, drag to reorder</SectionLabel>
      <div ref={listRef} className="flex flex-col gap-[5px]">
        {upNext.length === 0 && (
          <p className="py-2 text-sm text-ink-3">Nothing queued behind the current task.</p>
        )}
        {upNext.map((id, i) => {
          const task = taskById(snap, id);
          if (!task) return null;
          const parked = parkedFor(snap, id);
          const left = parked?.remainingWhenPausedMs ?? null;

          return (
            <div
              key={id}
              data-row
              tabIndex={0}
              role="button"
              draggable
              title={
                left != null
                  ? `Resume this block — ${clockStr(left)} of its allocation is left`
                  : "Start this task now — the running block is set down, keeping its remaining time"
              }
              onClick={() => send({ kind: "switchTo", task: id })}
              onKeyDown={(e) => {
                if (e.key === "Enter") { e.preventDefault(); void send({ kind: "switchTo", task: id }); }
                if (e.key === "ArrowDown") { e.preventDefault(); focusSibling(e.currentTarget, 1); }
                if (e.key === "ArrowUp") { e.preventDefault(); focusSibling(e.currentTarget, -1); }
              }}
              onDragStart={() => setDragging(id)}
              onDragOver={(e) => { e.preventDefault(); setOver(id); }}
              onDragLeave={() => setOver((o) => (o === id ? null : o))}
              onDrop={(e) => {
                e.preventDefault();
                if (dragging && dragging !== id) void send({ kind: "reorder", moved: dragging, before: id });
                setDragging(null); setOver(null);
              }}
              className={`group flex cursor-grab items-center gap-2.5 rounded-lg border px-[9px] py-[7px] focus:outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                over === id ? "border-accent bg-accent-soft"
                : left != null ? "border-transparent bg-warn-soft"
                : "border-transparent bg-surface-2 hover:border-line-2"
              }`}
            >
              <span className="shrink-0 text-ink-3">⠿</span>
              <span className="w-4 shrink-0 font-mono text-[11px] text-ink-3">{i + 1}</span>
              <span className={`h-[5px] w-[5px] shrink-0 rounded-full ${PRIORITY_DOT[task.priority]}`} />
              <span className="min-w-0 flex-1 truncate text-[13.5px]">{task.title}</span>
              <span className={`font-mono text-[11.5px] ${left != null ? "font-medium text-warn" : "text-ink-2"}`}>
                {left != null ? `${clockStr(left)} left` : durStr(task.blockDurationMs)}
              </span>
              <span className="rounded-[5px] bg-accent-soft px-[7px] py-[2px] font-mono text-[10.5px] text-accent-ink opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
                {left != null ? "Resume ▶" : "Start ▶"}
              </span>
              <button
                type="button"
                title="Remove"
                onClick={(e) => { e.stopPropagation(); void send({ kind: "removeTask", task: id }); }}
                className="px-0.5 text-[15px] leading-none text-ink-3 opacity-0 transition-opacity hover:text-alert group-hover:opacity-100"
              >
                ×
              </button>
            </div>
          );
        })}
      </div>
    </section>
  );
}
