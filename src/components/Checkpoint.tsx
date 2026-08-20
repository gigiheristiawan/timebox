import { useEffect, useState } from "react";
import { durStr } from "../core/format";
import { useTimebox, breakDefaultMs, currentBlock, currentTask, isBreak, taskById } from "../stores/useTimebox";

const STALENESS_FLOOR_MS = 120_000;

/**
 * The expiration checkpoint. There is deliberately no dismiss, close, later, or
 * continue — and no timeout that resolves it. Escape is inert. Only one of the
 * decisions below leaves this screen (SPEC §7.4).
 */
export function Checkpoint() {
  const { snap, send, init } = useTimebox();

  useEffect(() => {
    let un: (() => void) | undefined;
    void init().then((u) => { un = u; });
    return () => un?.();
  }, [init]);

  // `null` means "follow the setting". The segmented control selects, it does
  // not act (SPEC §7.4), so until the user touches it the pre-set value must
  // track `defaultBreakDurationSeconds` — including the first snapshot, which
  // arrives after this component first renders.
  const [chosenBreakMin, setChosenBreakMin] = useState<number | null>(null);
  const [customMin, setCustomMin] = useState("");

  const awaiting = snap?.state.timerState === "AwaitingDecision";
  const onBreak = isBreak(snap);
  const breakMin = chosenBreakMin ?? Math.round(breakDefaultMs(snap) / 60_000);
  const setBreakMin = setChosenBreakMin;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!awaiting) return;
      const el = e.target as HTMLElement | null;
      if (el && /^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName)) return;
      const ms = breakMin * 60_000;
      if (onBreak) {
        if (e.key === "1" || e.key === "Enter") void send({ kind: "endBreak" });
        if (e.key === "2") void send({ kind: "extendBreak", ms: 5 * 60_000 });
        return;
      }
      if (e.key === "1") void send({ kind: "decideComplete" });
      if (e.key === "2") void send({ kind: "decideBreak", ms, complete: true });
      if (e.key === "3" || e.key === "Enter") void send({ kind: "decidePending" });
      if (e.key === "4") void send({ kind: "decideBreak", ms, complete: false });
      if (e.key === "5") void send({ kind: "decideExtend", ms: 5 * 60_000 });
      // Escape, Cmd+W and everything else are deliberately inert.
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [awaiting, onBreak, breakMin, send]);

  if (!snap || !awaiting) {
    return <div className="min-h-screen bg-surface" />;
  }

  const block = currentBlock(snap);
  const alloc = block ? block.plannedMs + block.extensionMs : 0;
  const stale = snap.stalenessMs ?? 0;

  if (onBreak) {
    const nextId = snap.state.queue[0];
    const next = nextId ? taskById(snap, nextId) : undefined;
    return (
      <Shell tone="rest" kicker="Break's over" heading={next?.title ?? "Nothing queued"}>
        <p className="text-sm text-ink-2">
          {durStr(alloc)} break{block && block.extensionMs > 0 ? ` (extended +${durStr(block.extensionMs)})` : ""} finished.
        </p>
        <p className="mt-5 text-[15px] font-medium">
          {next ? "Ready to pick the queue back up?" : "The queue is empty."}
        </p>
        <div className="mt-6 flex w-full max-w-[380px] flex-col gap-2.5">
          <Action tone="rest" onClick={() => send({ kind: "endBreak" })} num="1 · ⏎">
            <span className="w-4 text-center">▶</span>
            {next ? `Start ${next.title}` : "Finish for now"}
          </Action>
          <div className="flex gap-2">
            {[5, 10, 15].map((m) => (
              <Small key={m} onClick={() => send({ kind: "extendBreak", ms: m * 60_000 })}>+{m} min</Small>
            ))}
          </div>
        </div>
        <Note>Break time is tracked separately — it never counts as worked</Note>
      </Shell>
    );
  }

  const task = currentTask(snap);
  const extendedToday = snap.state.blocks
    .filter((b) => b.taskId === task?.id)
    .reduce((n, b) => n + b.extensionMs, 0);

  return (
    <Shell tone="alert" kicker="Time's up" heading={task?.title ?? ""}>
      <p className="text-sm text-ink-2">Your {durStr(alloc)} block is over.</p>
      <p className="mt-5 text-[15px] font-medium">What do you want to do with this task?</p>

      {stale > STALENESS_FLOOR_MS && (
        <Warn>This block ended {durStr(stale)} ago.</Warn>
      )}
      {extendedToday > 0 && (
        <Warn>
          Original allocation {durStr(block?.plannedMs ?? 0)} · extended +{durStr(extendedToday)} today.
          You are choosing to spend more of the day here.
        </Warn>
      )}

      <div className="mt-6 flex w-full max-w-[460px] flex-col gap-3">
        <div className="grid grid-cols-[auto_1fr_1fr] items-center gap-2">
          <span />
          <Header>then start next</Header>
          <Header>then take a break</Header>

          <RowLabel glyph="✓">Complete</RowLabel>
          <Action onClick={() => send({ kind: "decideComplete" })} num="1">Start Next</Action>
          <Action onClick={() => send({ kind: "decideBreak", ms: breakMin * 60_000, complete: true })} num="2">
            Break {breakMin}m
          </Action>

          <RowLabel glyph="→">Keep pending</RowLabel>
          <Action tone="accent" onClick={() => send({ kind: "decidePending" })} num="3 · ⏎">Start Next</Action>
          <Action tone="rest" onClick={() => send({ kind: "decideBreak", ms: breakMin * 60_000, complete: false })} num="4">
            Break {breakMin}m
          </Action>
        </div>

        <div className="flex items-center justify-center gap-1.5">
          <Header>Break length</Header>
          {[5, 10, 15, 30].map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => setBreakMin(m)}
              aria-pressed={m === breakMin}
              className={`rounded-md border px-2.5 py-0.5 font-mono text-[11.5px] transition-colors ${
                m === breakMin ? "border-rest bg-rest text-white" : "border-line-2 text-ink-2 hover:bg-surface-3"
              }`}
            >
              {m}m
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-center justify-center gap-2 border-t border-line pt-3">
          <Header>Extend — keep working on this</Header>
          {[5, 10, 15].map((m) => (
            <Small key={m} onClick={() => send({ kind: "decideExtend", ms: m * 60_000 })}>+{m} min</Small>
          ))}
          <input
            type="number"
            min={1}
            max={180}
            value={customMin}
            onChange={(e) => setCustomMin(e.target.value)}
            placeholder="min"
            aria-label="Custom extension minutes"
            className="w-16 rounded-md border border-line-2 bg-surface px-2 py-1 text-[13px]"
          />
          <Small
            onClick={() => {
              const m = Number(customMin);
              if (m > 0) { void send({ kind: "decideExtend", ms: m * 60_000 }); setCustomMin(""); }
            }}
          >
            Add
          </Small>
        </div>
      </div>

      <Note>No dismiss · no close · no timeout — the decision is the product</Note>
    </Shell>
  );
}

// ------------------------------------------------------------------- pieces

function Shell({
  tone, kicker, heading, children,
}: { tone: "alert" | "rest"; kicker: string; heading: string; children: React.ReactNode }) {
  const glow = tone === "alert" ? "from-alert-soft" : "from-rest-soft";
  const kick = tone === "alert" ? "text-alert" : "text-rest";
  return (
    <div className={`flex min-h-screen flex-col items-center justify-center bg-gradient-to-b ${glow} via-surface to-surface px-6 py-10 text-center`}>
      <span className={`font-mono text-xs font-semibold uppercase tracking-[0.34em] ${kick}`}>{kicker}</span>
      <h1 className="mt-3.5 max-w-[22ch] text-3xl font-semibold tracking-tight">{heading}</h1>
      {children}
    </div>
  );
}

function Header({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-center font-mono text-[9.5px] uppercase tracking-[0.13em] text-ink-3">{children}</span>
  );
}

function RowLabel({ glyph, children }: { glyph: string; children: React.ReactNode }) {
  return (
    <span className="flex items-center gap-2 whitespace-nowrap pr-2.5 text-sm font-semibold">
      <span className="w-4 text-center text-[15px]">{glyph}</span>
      {children}
    </span>
  );
}

function Action({
  children, onClick, num, tone = "plain",
}: { children: React.ReactNode; onClick: () => void; num: string; tone?: "plain" | "accent" | "rest" }) {
  const look = {
    plain: "border-line-2 bg-surface hover:border-accent",
    accent: "border-accent bg-accent text-white",
    rest: "border-rest bg-rest text-white",
  }[tone];
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex flex-col items-center justify-center rounded-lg border px-3 py-3 text-[13.5px] font-medium leading-tight transition-transform hover:-translate-y-px ${look}`}
    >
      <span className="flex items-center gap-2">{children}</span>
      <span className={`mt-1 font-mono text-[10.5px] ${tone === "plain" ? "text-ink-3" : "text-white/70"}`}>{num}</span>
    </button>
  );
}

function Small({ children, onClick }: { children: React.ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-md border border-line-2 bg-surface px-3 py-1 text-[13px] font-medium hover:bg-surface-3"
    >
      {children}
    </button>
  );
}

function Warn({ children }: { children: React.ReactNode }) {
  return (
    <p className="mt-3.5 flex max-w-[44ch] items-center gap-2 rounded-md bg-warn-soft px-3 py-1.5 text-left text-[12.5px] text-warn">
      <span>⚠</span>
      <span>{children}</span>
    </p>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <p className="mt-5 font-mono text-[11.5px] tracking-wide text-ink-3">{children}</p>;
}
