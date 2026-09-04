import { useState } from "react";
import { durStr } from "../core/format";
import { useTimebox, currentBlock, currentTask, isBreak, taskById } from "../stores/useTimebox";
import { Countdown, PomodoroCountdown } from "./Countdown";
import { Button, Chip, PriorityDot, SectionLabel } from "./ui";
import { TaskEditor } from "./TaskEditor";

export function CurrentPanel() {
  const { snap, send } = useTimebox();
  const [editing, setEditing] = useState(false);
  if (!snap) return null;

  const block = currentBlock(snap);
  const task = currentTask(snap);
  const onBreak = isBreak(snap);
  const paused = snap.state.timerState === "Paused";
  const nextId = snap.state.queue[0];

  if (onBreak && block) {
    const next = nextId ? taskById(snap, nextId) : undefined;
    return (
      <section className="flex flex-col gap-3">
        <SectionLabel>On a break</SectionLabel>
        <div className="flex items-start gap-[14px]">
          <div className="flex flex-col gap-[7px]">
            <h2 className="text-[19px] font-semibold leading-[1.25] tracking-[-0.01em] text-rest-ink">Break</h2>
            <div className="flex flex-wrap gap-1.5">
              <Chip tone="rest">{durStr(block.plannedMs)} break</Chip>
              {block.extensionMs > 0 && <Chip tone="warn">Extended +{durStr(block.extensionMs)}</Chip>}
              {next && <Chip>Next up · {next.title}</Chip>}
            </div>
          </div>
          <Countdown className="ml-auto flex-none text-[30px] font-medium tracking-[-0.02em] text-rest" />
        </div>
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => send({ kind: "extendBreak", ms: 5 * 60_000 })}>+5 min</Button>
          <Button variant="primary" onClick={() => send({ kind: "endBreak" })}>
            End break{next ? ` & start ${next.title.split(" ")[0]}` : ""}
          </Button>
        </div>
      </section>
    );
  }

  if (!task || !block) {
    const next = nextId ? taskById(snap, nextId) : undefined;
    return (
      <section className="flex flex-col gap-3">
        <SectionLabel>Current task</SectionLabel>
        <p className="text-sm text-ink-3">
          {next ? "Nothing running. Start the top of the queue whenever you are ready." : "Queue is empty. Add a task to plan the day."}
        </p>
        {next && (
          <div>
            <Button variant="primary" onClick={() => send({ kind: "switchTo", task: next.id })}>
              Start {next.title}
            </Button>
          </div>
        )}
      </section>
    );
  }

  const blockNumber = snap.state.blocks.filter((b) => b.taskId === task.id).length;

  return (
    <section className="flex flex-col gap-3">
      <SectionLabel>Current task</SectionLabel>
      <div className="flex items-start gap-[14px]">
        <div className="flex min-w-0 flex-1 flex-col gap-[7px]">
          {editing ? (
            <TaskEditor task={task} onDone={() => setEditing(false)} />
          ) : (
            <h2 className="group flex items-center gap-2 text-[19px] font-semibold leading-[1.25] tracking-[-0.01em]">
              <PriorityDot priority={task.priority} />
              <span className="min-w-0 truncate">{task.title}</span>
              <button
                type="button"
                title="Rename or re-prioritise"
                onClick={() => setEditing(true)}
                className="text-[13px] font-normal text-ink-3 opacity-0 transition-opacity hover:text-accent group-hover:opacity-100"
              >
                ✎
              </button>
            </h2>
          )}
          <div className="flex flex-wrap gap-1.5">
            <Chip tone="accent">Block {blockNumber} · {durStr(block.plannedMs)}</Chip>
            {block.extensionMs > 0 && <Chip tone="warn">Extended +{durStr(block.extensionMs)}</Chip>}
            {block.interruptions > 0 && (
              <Chip tone="warn">Set down {block.interruptions}× · resuming its remainder</Chip>
            )}
            {paused && <Chip>Paused — allocation held</Chip>}
          </div>
        </div>
        <div className="ml-auto flex flex-none flex-col items-end">
          <Countdown className={`text-[30px] font-medium tracking-[-0.02em] ${paused ? "text-ink-3" : ""}`} />
          {/* The second clock, when Pomodoro mode is on. Labelled, because two
              bare countdowns side by side say nothing about which is which. */}
          {snap?.pomodoro && !onBreak && (
            <span className="mt-0.5 flex items-baseline gap-1 font-mono text-[11px] uppercase tracking-[0.14em] text-ink-3">
              break in <PomodoroCountdown className="text-[11.5px] text-rest" />
            </span>
          )}
        </div>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => send({ kind: paused ? "resume" : "pause" })} hint="Space">
          {paused ? "Resume" : "Pause"}
        </Button>
        <Button onClick={() => send({ kind: "skip" })} hint="S">Skip block</Button>
        <Button variant="primary" onClick={() => send({ kind: "completeCurrentTask" })} hint="D">
          Complete task
        </Button>
      </div>
    </section>
  );
}
