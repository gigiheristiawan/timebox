import { durStr } from "../core/format";
import { useTimebox, currentBlock, currentTask, isBreak, taskById } from "../stores/useTimebox";
import { Countdown } from "./Countdown";
import { Button, Chip, SectionLabel } from "./ui";

export function CurrentPanel() {
  const { snap, send } = useTimebox();
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
        <div className="flex items-start gap-4">
          <div className="flex flex-col gap-2">
            <h2 className="text-lg font-semibold tracking-tight text-rest-ink">Break</h2>
            <div className="flex flex-wrap gap-1.5">
              <Chip tone="rest">{durStr(block.plannedMs)} break</Chip>
              {block.extensionMs > 0 && <Chip tone="warn">Extended +{durStr(block.extensionMs)}</Chip>}
              {next && <Chip>Next up · {next.title}</Chip>}
            </div>
          </div>
          <Countdown className="ml-auto text-3xl font-medium tracking-tight text-rest" />
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
      <div className="flex items-start gap-4">
        <div className="flex flex-col gap-2">
          <h2 className="text-lg font-semibold leading-tight tracking-tight">{task.title}</h2>
          <div className="flex flex-wrap gap-1.5">
            <Chip tone="accent">Block {blockNumber} · {durStr(block.plannedMs)}</Chip>
            {block.extensionMs > 0 && <Chip tone="warn">Extended +{durStr(block.extensionMs)}</Chip>}
            {block.interruptions > 0 && (
              <Chip tone="warn">Set down {block.interruptions}× · resuming its remainder</Chip>
            )}
            {paused && <Chip>Paused — allocation held</Chip>}
          </div>
        </div>
        <Countdown className={`ml-auto text-3xl font-medium tracking-tight ${paused ? "text-ink-3" : ""}`} />
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
