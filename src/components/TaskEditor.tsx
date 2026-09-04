import { useState } from "react";
import { durStr, MIN } from "../core/format";
import { useTimebox } from "../stores/useTimebox";
import type { Priority, Task } from "../ipc/types";

/** The same grants the checkpoint offers, so more time is one vocabulary. */
const ADD_CHOICES = [5, 10, 15];

/**
 * Rename and re-prioritise, in place. Shared by the queue row and the current
 * task so an edit is the same control wherever it is reached.
 *
 * Time is *added*, never set. An allocation that could be typed over would let
 * a running block be shortened into an early checkpoint, or a parked one be cut
 * below what was already promised; `+5/+10/+15` can only ever grant.
 */
export function TaskEditor({ task, onDone }: { task: Task; onDone: () => void }) {
  const { send } = useTimebox();
  const [title, setTitle] = useState(task.title);
  const [priority, setPriority] = useState<Priority>(task.priority);
  const [daily, setDaily] = useState(task.daily);
  // Held as a draft rather than dispatched per click, so Cancel means cancel.
  const [addedMs, setAddedMs] = useState(0);
  const blank = title.trim() === "";

  const save = () => {
    if (blank) return;
    void send({ kind: "editTask", task: task.id, title, priority, daily });
    if (addedMs > 0) void send({ kind: "addTime", task: task.id, ms: addedMs });
    onDone();
  };

  return (
    <div
      className="flex flex-1 flex-wrap items-center gap-2"
      // The row underneath switches task on click and drags on grab; neither
      // should fire while the editor has the pointer.
      onClick={(e) => e.stopPropagation()}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <input
        autoFocus
        value={title}
        aria-label="Task title"
        aria-invalid={blank}
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") { e.preventDefault(); save(); }
          if (e.key === "Escape") { e.preventDefault(); onDone(); }
        }}
        className={`field min-w-[120px] flex-1 ${blank ? "!border-alert" : ""}`}
      />
      <select
        value={priority}
        aria-label="Priority"
        onChange={(e) => setPriority(e.target.value as Priority)}
        className="field"
      >
        <option value="High">High</option>
        <option value="Medium">Medium</option>
        <option value="Low">Low</option>
      </select>
      <label
        title="Recurs every day: ticking it off never removes it from the queue"
        className="flex cursor-pointer items-center gap-1.5 text-[12.5px] text-ink-2"
      >
        <input
          type="checkbox"
          checked={daily}
          onChange={(e) => setDaily(e.target.checked)}
          className="accent-accent"
        />
        Daily
      </label>
      <div className="flex items-center gap-1">
        {ADD_CHOICES.map((m) => (
          <button
            key={m}
            type="button"
            onClick={() => setAddedMs((a) => a + m * MIN)}
            title={`Give this task ${m} more minutes`}
            className="rounded-[6px] border border-line-2 px-[7px] py-1 font-mono text-[11px] text-ink-2 transition-colors hover:border-accent hover:text-accent-ink"
          >
            +{m}m
          </button>
        ))}
        {addedMs > 0 && (
          <button
            type="button"
            onClick={() => setAddedMs(0)}
            title="Undo the added time"
            className="rounded-[5px] bg-warn-soft px-[7px] py-[3px] font-mono text-[10.5px] text-warn"
          >
            {durStr(task.blockDurationMs)} → {durStr(task.blockDurationMs + addedMs)} ×
          </button>
        )}
      </div>
      <button
        type="button"
        onClick={save}
        disabled={blank}
        title="Save (Return)"
        className="rounded-[7px] border border-accent bg-accent px-[11px] py-1.5 text-[12.5px] font-medium text-white transition-[filter] hover:brightness-[1.08] disabled:opacity-40"
      >
        Save
      </button>
      <button
        type="button"
        onClick={onDone}
        title="Cancel (Esc)"
        className="rounded-[7px] border border-line-2 px-[11px] py-1.5 text-[12.5px] text-ink-2 transition-colors hover:bg-surface-3"
      >
        Cancel
      </button>
    </div>
  );
}
