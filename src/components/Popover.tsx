import { useEffect, useRef } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { clockStr, durStr } from "../core/format";
import { openMainWindow, quitApp } from "../ipc/commands";
import { currentBlock, currentTask, isBreak, parkedFor, taskById, useTimebox } from "../stores/useTimebox";
import { Countdown } from "./Countdown";

/** Matches `.popover` in docs/mockup.html — the design reference for this window. */
const WIDTH = 300;
/** How much of the queue fits before the popover becomes a second app. */
const QUEUE_PREVIEW = 5;

const LABEL = "font-mono text-[9.5px] uppercase tracking-[0.15em] text-ink-3";
const SECTION = "px-3.5 py-3";

/**
 * The menu bar popover (SPEC §7.2, prototype `docs/mockup.html`). A normal day
 * must be possible from here alone — start, pause, skip, switch — so every
 * action is the same `Action` the main window sends. Nothing is decided here.
 */
export function Popover() {
  const { snap, error, init, send } = useTimebox();
  const card = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let un: (() => void) | undefined;
    void init().then((u) => { un = u; });
    return () => un?.();
  }, [init]);

  // The card is the window: a fixed height would leave dead space under a short
  // queue and clip a long one, so the window follows the content.
  useEffect(() => {
    const el = card.current;
    if (!el) return;
    const fit = () => {
      const h = Math.ceil(el.getBoundingClientRect().height);
      if (h > 0) void getCurrentWindow().setSize(new LogicalSize(WIDTH, h));
    };
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const state = snap?.state.timerState;
  const block = currentBlock(snap ?? null);
  const task = currentTask(snap ?? null);
  const onBreak = isBreak(snap ?? null);
  const paused = state === "Paused";
  const queue = snap?.state.queue ?? [];
  const nextId = queue.find((id) => !(block?.taskId === id && state !== "Idle"));
  const next = nextId ? taskById(snap ?? null, nextId) : undefined;

  return (
    <div
      ref={card}
      className="w-full overflow-hidden rounded-[11px] border border-line bg-surface shadow-pop"
    >
      {error && <p className="bg-alert-soft px-3.5 py-2 text-xs text-alert">{error}</p>}

      {/* Current -------------------------------------------------------- */}
      <section className={SECTION}>
        <div className={LABEL}>{onBreak ? "On a break" : "Current"}</div>
        <div
          className={`mt-1 truncate text-[13.5px] font-semibold leading-[1.3] ${onBreak ? "text-rest-ink" : ""}`}
          title={task?.title}
        >
          {onBreak ? "Break" : (task?.title ?? "Nothing running")}
        </div>

        <Countdown
          className={`block pb-0.5 pt-1.5 text-center text-[34px] font-medium leading-none tracking-[-0.02em] ${
            state === "AwaitingDecision" ? "text-alert" : onBreak ? "text-rest" : paused ? "text-ink-3" : ""
          }`}
        />

        {next && (
          <>
            <div className={`${LABEL} mt-2`}>Next</div>
            <div className="mt-0.5 truncate text-[13px] text-ink-2">{next.title}</div>
          </>
        )}

        <div className="mt-2 flex gap-2">
          {state === "AwaitingDecision" ? (
            // The checkpoint owns the decision and has no side doors (SPEC §7.4).
            <p className="flex-1 rounded-md bg-alert-soft px-2.5 py-1.5 text-[12px] leading-snug text-alert">
              The checkpoint is waiting for your decision.
            </p>
          ) : onBreak ? (
            <>
              <PopButton onClick={() => send({ kind: "extendBreak", ms: 5 * 60_000 })}>+5 min</PopButton>
              <PopButton primary onClick={() => send({ kind: "endBreak" })}>End break</PopButton>
            </>
          ) : task ? (
            <>
              <PopButton onClick={() => send({ kind: paused ? "resume" : "pause" })}>
                {paused ? "Resume" : "Pause"}
              </PopButton>
              <PopButton onClick={() => send({ kind: "skip" })}>Skip</PopButton>
            </>
          ) : (
            <PopButton primary disabled={!next} onClick={() => next && send({ kind: "switchTo", task: next.id })}>
              Start next
            </PopButton>
          )}
        </div>
      </section>

      {/* Today's tasks --------------------------------------------------- */}
      <section className={`border-t border-line ${SECTION}`}>
        <div className={LABEL}>Today&apos;s tasks</div>
        {queue.length === 0 && <p className="py-1 text-[12.5px] text-ink-3">Queue empty</p>}
        {queue.slice(0, QUEUE_PREVIEW).map((id) => {
          const t = taskById(snap ?? null, id);
          if (!t) return null;
          const current = block?.taskId === id && state !== "Idle";
          const parked = parkedFor(snap ?? null, id);
          return (
            <button
              key={id}
              type="button"
              onClick={() => send({ kind: "switchTo", task: id })}
              className={`flex w-full items-baseline gap-2 py-[2.5px] text-left text-[12.5px] ${
                current ? "font-semibold text-ink" : "text-ink-2 hover:text-ink"
              }`}
            >
              <span className="w-3 flex-none text-accent">{current ? "→" : ""}</span>
              <span className="truncate">{t.title}</span>
              <span className="tabular ml-auto flex-none pl-2 font-mono text-[11px] text-ink-3">
                {parked?.remainingWhenPausedMs != null
                  ? `${clockStr(parked.remainingWhenPausedMs)} left`
                  : durStr(t.blockDurationMs)}
              </span>
            </button>
          );
        })}
      </section>

      {/* Menu ------------------------------------------------------------ */}
      <nav className="border-t border-line py-1.5">
        <MenuItem onClick={() => void openMainWindow()}>Open App</MenuItem>
        <MenuItem onClick={() => void quitApp()}>Quit TimeBox</MenuItem>
      </nav>
    </div>
  );
}

function PopButton({
  children, onClick, primary, disabled,
}: {
  children: React.ReactNode;
  onClick?: () => void;
  primary?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`flex-1 rounded-md px-3 py-1.5 text-[13px] font-medium transition-colors disabled:opacity-40 ${
        primary
          ? "border border-accent bg-accent text-white hover:brightness-110"
          : "border border-line-2 bg-surface hover:bg-surface-3"
      }`}
    >
      {children}
    </button>
  );
}

function MenuItem({ children, onClick }: { children: React.ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="block w-full px-3.5 py-[5px] text-left text-[13px] text-ink-2 hover:bg-accent hover:text-white"
    >
      {children}
    </button>
  );
}
