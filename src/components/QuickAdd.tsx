import { useEffect, useRef, useState } from "react";
import { MIN } from "../core/format";
import { useTimebox } from "../stores/useTimebox";

/**
 * `Cmd+K` quick add (task 7.6). Title only — it takes the default block
 * duration from settings, which is the whole point of a quick add: one field,
 * one keystroke, back to work. The inline row below stays for the cases where
 * duration or priority matter.
 */
export function QuickAdd({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { snap, send } = useTimebox();
  const [title, setTitle] = useState("");
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setTitle("");
      input.current?.focus();
    }
  }, [open]);

  if (!open || !snap) return null;
  const blockMs = snap.settings.defaultBlockDurationMs;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (title.trim() === "") return;
    void send({ kind: "addTask", title, blockMs, priority: "Medium", daily: false });
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/30 pt-[18vh]"
      onClick={onClose}
    >
      <form
        onSubmit={submit}
        onClick={(e) => e.stopPropagation()}
        className="w-[min(420px,88vw)] rounded-[11px] border border-line bg-surface p-3 shadow-pop"
      >
        <input
          ref={input}
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}
          placeholder="New task…"
          aria-label="New task title"
          className="field w-full"
        />
        <p className="px-1 pt-2 text-[11.5px] text-ink-3">
          {Math.round(blockMs / MIN)}-minute block · <kbd className="font-mono">⏎</kbd> to add ·{" "}
          <kbd className="font-mono">esc</kbd> to close
        </p>
      </form>
    </div>
  );
}
