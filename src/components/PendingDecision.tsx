import { durStr } from "../core/format";
import { useTimebox, breakDefaultMs, currentBlock, currentTask, isBreak, taskById } from "../stores/useTimebox";
import { Button, SectionLabel } from "./ui";

/**
 * Phase 4 stand-in for the expiration checkpoint. The full-screen, always-on-top,
 * no-exit window — plus activation, sound, and notification — is Phase 5. This
 * renders the same decisions inline so the app is usable end to end in the
 * meantime; the actions themselves are already the real ones.
 */
export function PendingDecision() {
  const { snap, send } = useTimebox();
  const pomodoro = snap?.state.timerState === "AwaitingPomodoro";
  if (!snap || (snap.state.timerState !== "AwaitingDecision" && !pomodoro)) return null;

  const breakMs = breakDefaultMs(snap);
  const breakMinutes = Math.round(breakMs / 60_000);

  const block = currentBlock(snap);
  const stale = snap.stalenessMs ?? 0;

  // The Pomodoro prompt decides the break, never the task — so this offers the
  // same two answers and nothing else. Without it the panel below would render
  // Pause/Skip/Complete, which the reducer refuses here (POMODORO_MODE §4.7):
  // controls that look live and do nothing.
  if (pomodoro) {
    const task = currentTask(snap);
    return (
      <section className="flex flex-col gap-3 rounded-xl border border-rest bg-rest-soft p-4">
        <SectionLabel>Time for a break</SectionLabel>
        <p className="text-sm text-ink-2">
          You&apos;ve worked 25 minutes straight on {task?.title ?? "this task"}.
          {block?.remainingWhenPausedMs
            ? ` Your block keeps its ${durStr(block.remainingWhenPausedMs)} either way.`
            : ""}
        </p>
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" onClick={() => send({ kind: "decidePomodoroBreak", ms: breakMs })}>
            Take a {breakMinutes}m break
          </Button>
          <Button onClick={() => send({ kind: "decideSkipPomodoro" })}>Skip break &amp; continue</Button>
        </div>
        {stale > 120_000 && (
          <p className="text-xs text-ink-3">Waiting {durStr(stale)}.</p>
        )}
      </section>
    );
  }

  if (isBreak(snap)) {
    const nextId = snap.state.queue[0];
    const next = nextId ? taskById(snap, nextId) : undefined;
    return (
      <section className="flex flex-col gap-3 rounded-xl border border-rest bg-rest-soft p-4">
        <SectionLabel>Break&apos;s over</SectionLabel>
        <p className="text-sm text-ink-2">
          {block ? durStr(block.plannedMs + block.extensionMs) : ""} break finished.
          {next ? " Ready to pick the queue back up?" : " The queue is empty."}
        </p>
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" onClick={() => send({ kind: "endBreak" })}>
            {next ? `▶ Start ${next.title}` : "▶ Finish for now"}
          </Button>
          {[5, 10, 15].map((m) => (
            <Button key={m} onClick={() => send({ kind: "extendBreak", ms: m * 60_000 })}>+{m} min</Button>
          ))}
        </div>
      </section>
    );
  }

  const task = currentTask(snap);
  const extendedToday = snap.state.blocks
    .filter((b) => b.taskId === task?.id)
    .reduce((n, b) => n + b.extensionMs, 0);

  return (
    <section className="flex flex-col gap-3 rounded-xl border border-alert bg-alert-soft p-4">
      <SectionLabel>Time&apos;s up</SectionLabel>
      <div>
        <p className="text-base font-semibold">{task?.title}</p>
        <p className="text-sm text-ink-2">
          Your {block ? durStr(block.plannedMs + block.extensionMs) : ""} block is over.
          What do you want to do with this task?
        </p>
      </div>

      {stale > 120_000 && (
        <p className="rounded-md bg-warn-soft px-2.5 py-1.5 text-[12.5px] text-warn">
          ⚠ This block ended {durStr(stale)} ago.
        </p>
      )}
      {extendedToday > 0 && (
        <p className="rounded-md bg-warn-soft px-2.5 py-1.5 text-[12.5px] text-warn">
          ⚠ Extended +{durStr(extendedToday)} today. You are choosing to spend more of the day here.
        </p>
      )}

      <div className="grid grid-cols-[auto_1fr_1fr] items-center gap-2">
        <span />
        <span className="text-center font-mono text-[9.5px] uppercase tracking-[0.13em] text-ink-3">then start next</span>
        <span className="text-center font-mono text-[9.5px] uppercase tracking-[0.13em] text-ink-3">then take a break</span>

        <span className="whitespace-nowrap pr-2 text-sm font-semibold">✓ Complete</span>
        <Button onClick={() => send({ kind: "decideComplete" })} hint="1">Start Next</Button>
        <Button onClick={() => send({ kind: "decideBreak", ms: breakMs, complete: true })} hint="2">
          Break {breakMinutes}m
        </Button>

        <span className="whitespace-nowrap pr-2 text-sm font-semibold">→ Keep pending</span>
        <Button variant="primary" onClick={() => send({ kind: "decidePending" })} hint="3 · ⏎">Start Next</Button>
        <Button onClick={() => send({ kind: "decideBreak", ms: breakMs, complete: false })} hint="4">
          Break {breakMinutes}m
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <span className="font-mono text-[9.5px] uppercase tracking-[0.13em] text-ink-3">Extend</span>
        {[5, 10, 15].map((m) => (
          <Button key={m} onClick={() => send({ kind: "decideExtend", ms: m * 60_000 })}>+{m} min</Button>
        ))}
      </div>

      <p className="font-mono text-[11px] text-ink-3">
        No dismiss · no close · no timeout — the decision is the product
      </p>
    </section>
  );
}
