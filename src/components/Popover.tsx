import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { clockStr, durStr } from "../core/format";
import { openMainWindow, openSettingsWindow, requestQuit } from "../ipc/commands";
import { breakDefaultMs, currentBlock, currentTask, isBreak, parkedFor, taskById, useTimebox } from "../stores/useTimebox";
import { Countdown, PomodoroCountdown } from "./Countdown";
import { PriorityDot } from "./ui";

/** Matches `.popover` in docs/mockup.html — the design reference for this window. */
const WIDTH = 300;
/** How much of the queue fits before the popover becomes a second app. */
const QUEUE_PREVIEW = 5;
/** The same lengths the checkpoint offers, so a break is one control wherever
 *  it is started (IDLE_TIME §6). */
const BREAK_CHOICES = [5, 10, 15, 30];

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
  // null until the user picks one, so the control follows the setting rather
  // than freezing whatever it was on the first snapshot.
  const [chosenBreakMin, setChosenBreakMin] = useState<number | null>(null);

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
  const doneToday = snap?.doneToday ?? [];
  const nextId = queue.find(
    (id) => !(block?.taskId === id && state !== "Idle") && !doneToday.includes(id),
  );
  const next = nextId ? taskById(snap ?? null, nextId) : undefined;
  const breakMin = chosenBreakMin ?? Math.round(breakDefaultMs(snap ?? null) / 60_000);
  // D22: available from IDLE, RUNNING and PAUSED. At a work checkpoint the
  // decision is owed first, and during a break the operation is Extend.
  // Either checkpoint. `!== "AwaitingDecision"` alone left this true at the
  // Pomodoro prompt, offering a control the reducer refuses (POMODORO_MODE
  // §4.7) — a button that looks live and does nothing.
  const atCheckpoint = state === "AwaitingDecision" || state === "AwaitingPomodoro";
  const canBreak = !!snap && !atCheckpoint && !onBreak;
  const pomodoroOn = snap?.pomodoro != null;

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
            state === "AwaitingDecision"
              ? "text-alert"
              : state === "AwaitingPomodoro"
                ? "text-rest"
                : onBreak
                  ? "text-rest"
                  : paused
                    ? "text-ink-3"
                    : ""
          }`}
        />

        {snap?.pomodoro && !onBreak && (
          <div className="flex items-baseline justify-center gap-1 font-mono text-[10.5px] uppercase tracking-[0.14em] text-ink-3">
            break in <PomodoroCountdown className="text-[11px] text-rest" />
          </div>
        )}

        {next && (
          <>
            <div className={`${LABEL} mt-2`}>Next</div>
            <div className="mt-0.5 truncate text-[13px] text-ink-2">{next.title}</div>
          </>
        )}

        <div className="mt-2 flex gap-2">
          {atCheckpoint ? (
            // The checkpoint owns the decision and has no side doors (SPEC §7.4).
            <p
              className={`flex-1 rounded-md px-2.5 py-1.5 text-[12px] leading-snug ${
                state === "AwaitingPomodoro"
                  ? "bg-rest-soft text-rest-ink"
                  : "bg-alert-soft text-alert"
              }`}
            >
              {state === "AwaitingPomodoro"
                ? "Break or keep going — the checkpoint is waiting."
                : "The checkpoint is waiting for your decision."}
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
              <PopButton primary onClick={() => send({ kind: "completeCurrentTask" })}>Complete</PopButton>
            </>
          ) : (
            <PopButton primary disabled={!next} onClick={() => next && send({ kind: "switchTo", task: next.id })}>
              Start next
            </PopButton>
          )}
        </div>
        {/* Take a break (IDLE_TIME D22) ---------------------------------- */}
        {canBreak && (
          <div className="mt-2.5 flex items-center gap-1.5 border-t border-line pt-2.5">
            <button
              type="button"
              onClick={() => send({ kind: "startBreak", ms: breakMin * 60_000 })}
              className="rounded-md border border-rest px-2.5 py-1 text-[12.5px] font-medium text-rest-ink transition-colors hover:bg-rest hover:text-white"
            >
              Take a break
            </button>
            <div className="ml-auto flex gap-1">
              {BREAK_CHOICES.map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => setChosenBreakMin(m)}
                  aria-pressed={m === breakMin}
                  className={`rounded border px-1.5 py-0.5 font-mono text-[10.5px] transition-colors ${
                    m === breakMin ? "border-rest bg-rest text-white" : "border-line-2 text-ink-3 hover:bg-surface-3"
                  }`}
                >
                  {m}m
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Pomodoro mode (issue #15). A quick toggle here as well as in
            Settings, because the mode is meant to be flipped mid-day — "I need
            to get through this one, no interruptions" — and a trip to the
            settings window is friction enough that it would go unused.
            Disabled at either checkpoint for the same reason the break control
            is: switching the mode off must not become a way to dismiss its own
            prompt (D33). */}
        <div className="mt-2.5 flex items-center gap-2 border-t border-line pt-2.5">
          <span className="text-[12.5px] text-ink-2">Pomodoro mode</span>
          <button
            type="button"
            role="switch"
            aria-checked={pomodoroOn}
            aria-label="Pomodoro mode"
            disabled={atCheckpoint}
            onClick={() => send({ kind: "setPomodoroMode", on: !pomodoroOn })}
            className={`ml-auto flex h-[18px] w-[32px] flex-none items-center rounded-full border px-[2px] transition-colors ${
              pomodoroOn ? "border-rest bg-rest" : "border-line-2 bg-surface-3"
            } ${atCheckpoint ? "cursor-not-allowed opacity-40" : ""}`}
          >
            <span
              className={`h-[12px] w-[12px] rounded-full bg-white shadow-sm transition-transform ${
                pomodoroOn ? "translate-x-[14px]" : ""
              }`}
            />
          </button>
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
          // Ticked off for today: still listed, because seeing the dailies you
          // have already done is the point of keeping them in the queue.
          const done = doneToday.includes(id);
          return (
            <button
              key={id}
              type="button"
              disabled={done}
              title={done ? "Done for today — back tomorrow" : undefined}
              onClick={() => send({ kind: "switchTo", task: id })}
              className={`flex w-full items-center gap-2 py-[2.5px] text-left text-[12.5px] ${
                done ? "text-ink-3 opacity-60"
                : current ? "font-semibold text-ink"
                : "text-ink-2 hover:text-ink"
              }`}
            >
              <span className="w-3 flex-none text-accent">{done ? "✓" : current ? "→" : ""}</span>
              <PriorityDot priority={t.priority} />
              <span className={`truncate ${done ? "line-through" : ""}`}>{t.title}</span>
              <span className="tabular ml-auto flex-none pl-2 font-mono text-[11px] text-ink-3">
                {done ? "today"
                  : parked?.remainingWhenPausedMs != null
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
        <MenuItem onClick={() => void openSettingsWindow()}>Settings…</MenuItem>
        <MenuItem onClick={() => void requestQuit()}>Quit TimeBox</MenuItem>
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
