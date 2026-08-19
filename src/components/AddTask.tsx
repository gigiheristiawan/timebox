import { useState } from "react";
import { MIN } from "../core/format";
import { useTimebox } from "../stores/useTimebox";
import type { Priority } from "../ipc/types";

export function AddTask() {
  const send = useTimebox((s) => s.send);
  const [title, setTitle] = useState("");
  const [minutes, setMinutes] = useState(30);
  const [priority, setPriority] = useState<Priority>("Medium");
  const [touched, setTouched] = useState(false);

  const invalid = touched && title.trim() === "";

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (title.trim() === "") return;
    void send({ kind: "addTask", title, blockMs: minutes * MIN, priority });
    setTitle("");
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
          className={`w-full rounded-md border bg-surface px-2.5 py-1.5 text-[13px] focus:outline-none focus:ring-2 focus:ring-accent ${
            invalid ? "border-alert" : "border-line-2"
          }`}
        />
        {invalid && <span className="text-[11px] text-alert">A task needs a title.</span>}
      </div>
      <select
        value={minutes}
        onChange={(e) => setMinutes(Number(e.target.value))}
        aria-label="Block duration"
        className="rounded-md border border-line-2 bg-surface px-2 py-1.5 text-[13px]"
      >
        {[15, 25, 30, 45, 60].map((m) => <option key={m} value={m}>{m} min</option>)}
      </select>
      <select
        value={priority}
        onChange={(e) => setPriority(e.target.value as Priority)}
        aria-label="Priority"
        className="rounded-md border border-line-2 bg-surface px-2 py-1.5 text-[13px]"
      >
        <option value="High">High</option>
        <option value="Medium">Medium</option>
        <option value="Low">Low</option>
      </select>
      <button
        type="submit"
        className="rounded-md border border-line-2 bg-surface px-3 py-1.5 text-sm font-medium hover:bg-surface-3"
      >
        Add
      </button>
    </form>
  );
}
