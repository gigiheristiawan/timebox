import { useState } from "react";
import { MIN } from "../core/format";
import { useTimebox } from "../stores/useTimebox";
import type { Priority } from "../ipc/types";

export function AddTask() {
  const { snap, send } = useTimebox();
  const [title, setTitle] = useState("");
  // `null` follows the configured default; picking a value pins it for the
  // rest of the session, so a one-off 45m task does not change the setting.
  const [chosen, setChosen] = useState<number | null>(null);
  const [priority, setPriority] = useState<Priority>("Medium");
  const [daily, setDaily] = useState(false);
  const [touched, setTouched] = useState(false);

  const defaultMinutes = Math.round((snap?.settings.defaultBlockDurationMs ?? 30 * MIN) / MIN);
  const minutes = chosen ?? defaultMinutes;
  const invalid = touched && title.trim() === "";

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (title.trim() === "") return;
    void send({ kind: "addTask", title, blockMs: minutes * MIN, priority, daily });
    setTitle("");
    setDaily(false);
    setTouched(false);
  };

  return (
    <form onSubmit={submit} className="flex flex-wrap items-start gap-2">
      <div className="flex min-w-[170px] flex-1 flex-col gap-1">
        <input
          id="new-task-title"
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Add a task…  (N)"
          aria-label="Task title"
          aria-invalid={invalid}
          className={`field w-full ${invalid ? "!border-alert" : ""}`}
        />
        {invalid && <span className="text-[11px] text-alert">A task needs a title.</span>}
      </div>
      <select
        value={minutes}
        onChange={(e) => setChosen(Number(e.target.value))}
        aria-label="Block duration"
        className="field"
      >
        {/* The configured default may sit outside this list; showing it keeps
            the select from silently changing the duration on first render. */}
        {Array.from(new Set([15, 25, 30, 45, 60, defaultMinutes]))
          .sort((a, b) => a - b)
          .map((m) => <option key={m} value={m}>{m} min</option>)}
      </select>
      <select
        value={priority}
        onChange={(e) => setPriority(e.target.value as Priority)}
        aria-label="Priority"
        className="field"
      >
        <option value="High">High</option>
        <option value="Medium">Medium</option>
        <option value="Low">Low</option>
      </select>
      <label
        title="Recurs every day: ticking it off never removes it from the queue"
        className="flex cursor-pointer items-center gap-1.5 px-1 py-1.5 text-[12.5px] text-ink-2"
      >
        <input
          type="checkbox"
          checked={daily}
          onChange={(e) => setDaily(e.target.checked)}
          className="accent-accent"
        />
        Daily
      </label>
      <button
        type="submit"
        className="rounded-[7px] border border-line-2 bg-surface px-[13px] py-1.5 text-[13px] font-medium transition-colors hover:bg-surface-3"
      >
        Add
      </button>
    </form>
  );
}
