import { useCallback, useEffect, useState } from "react";
import { getReport } from "../ipc/commands";
import type { WeekReport } from "../ipc/types";
import { dateStr, durStr, weekdayStr } from "../core/format";
import { Button, SectionLabel } from "./ui";

/** How often the *current* week refetches while the tab is open. A past week
 *  is settled history and never refetches (WEEKLY_REPORT D40); the current one
 *  moves in minutes, not seconds, so this is deliberately slow — the report is
 *  not on the snapshot precisely so a week is not recomputed once a second
 *  (D38). */
const REFRESH_MS = 15_000;

/**
 * The weekly report (issue #6). Every figure, total and ranking arrives
 * decided; this file formats, and takes the ratio of two numbers the backend
 * already supplied to draw a bar. It decides nothing (SPEC R6/R7).
 */
export function Report() {
  const [offset, setOffset] = useState(0);
  const [week, setWeek] = useState<WeekReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (n: number) => {
    try {
      setWeek(await getReport(n));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load(offset);
    if (offset !== 0) return;
    const id = window.setInterval(() => void load(0), REFRESH_MS);
    return () => window.clearInterval(id);
  }, [offset, load]);

  // ← / → step through weeks. The tab is a view, not a mode: every other
  // shortcut still reaches the timer.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && /^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName)) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "ArrowLeft") setOffset((n) => n - 1);
      if (e.key === "ArrowRight") setOffset((n) => Math.min(0, n + 1));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (error) return <p className="rounded-lg bg-alert-soft px-4 py-3 text-sm text-alert">{error}</p>;
  if (!week) return null;

  const t = week.totals;

  return (
    <div className="flex flex-col gap-[18px]">
      <header className="flex items-center gap-2">
        <Button variant="ghost" onClick={() => setOffset(offset - 1)}>‹</Button>
        <div className="min-w-0">
          <div className="text-[14px] font-medium">
            {dateStr(week.weekStart)} – {dateStr(week.weekEnd - 1)}
          </div>
          <div className="text-[10.5px] text-ink-3">
            {week.isCurrentWeek ? "This week" : `${-week.offset} week${week.offset === -1 ? "" : "s"} ago`}
          </div>
        </div>
        <Button variant="ghost" onClick={() => setOffset(Math.min(0, offset + 1))} disabled={offset >= 0}>
          ›
        </Button>
        {!week.isCurrentWeek && (
          <Button className="ml-auto" onClick={() => setOffset(0)}>
            This week
          </Button>
        )}
      </header>

      <section className="flex flex-col gap-[9px]">
        <SectionLabel>Week</SectionLabel>
        <div className="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(96px,1fr))]">
          <Stat value={durStr(t.workedMs)} label={`Worked of ${durStr(t.targetMs)}`} />
          <Stat value={attainment(t.workedMs, t.targetMs)} label="Of target" />
          <Stat value={String(t.tasksCompleted)} label="Tasks completed" />
          <Stat value={String(t.blocksCompleted)} label="Blocks completed" />
          <Stat value={`${t.switchedEarly}×`} label="Switched early" />
          <Stat value={durStr(t.breakMs)} label="On break" className="text-rest-ink" />
          <Stat value={durStr(t.idleMs)} label="Idle in working hours" className="text-ink-2" />
          <Stat value={`${t.daysWorked}/${t.workingDays}`} label="Days worked" />
        </div>
      </section>

      <section className="flex flex-col gap-[7px]">
        <SectionLabel>Days</SectionLabel>
        {week.days.map((d) => (
          <div key={d.dayStart} className="flex items-center gap-3 text-[12.5px]">
            <div className="w-[62px] flex-none">
              <div className={d.workingDay ? "" : "text-ink-3"}>{weekdayStr(d.dayStart)}</div>
              <div className="text-[10px] text-ink-3">{dateStr(d.dayStart)}</div>
            </div>
            <div className="h-[7px] min-w-0 flex-1 overflow-hidden rounded-full bg-surface-3">
              {/* A ratio of two numbers Rust supplied. Nothing is decided here. */}
              <div
                className={`h-full rounded-full ${d.targetMs > 0 ? "bg-accent" : "bg-warn"}`}
                style={{ width: `${barPct(d.workedMs, d.targetMs)}%` }}
              />
            </div>
            <div className="w-[56px] flex-none text-right font-mono text-[11.5px]">
              {d.workedMs > 0 ? durStr(d.workedMs) : <span className="text-ink-3">—</span>}
            </div>
            <div className="w-[46px] flex-none text-right font-mono text-[10.5px] text-ink-3">
              {attainment(d.workedMs, d.targetMs)}
            </div>
          </div>
        ))}
      </section>

      {week.top.length > 0 && (
        <section className="flex flex-col gap-[5px]">
          <SectionLabel>Top tasks</SectionLabel>
          {week.top.map((task, i) => (
            <div key={task.taskId} className="flex gap-2 text-[12.5px] text-ink-2">
              <span className="font-mono text-ink-3">{i + 1}</span>
              <span className="min-w-0 truncate">{task.title}</span>
              <span className="ml-auto flex-none font-mono text-[11.5px]">{durStr(task.ms)}</span>
              <span className="w-[38px] flex-none text-right font-mono text-[10.5px] text-ink-3">
                {share(task.ms, t.workedMs)}
              </span>
            </div>
          ))}
        </section>
      )}

      {/* IDLE_TIME §3. The causes, in the surface they were always meant for —
          they are causes of idle, not peers of it, which is why the Today strip
          shows only the total. */}
      <section className="flex flex-col gap-[9px]">
        <SectionLabel>Where the window went</SectionLabel>
        <div className="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(96px,1fr))]">
          <Stat value={durStr(t.idleAwaitingMs)} label="At a checkpoint" className="text-ink-2" />
          <Stat value={durStr(t.idlePausedMs)} label="Paused" className="text-ink-2" />
          <Stat value={durStr(t.idleUntrackedMs)} label="Nothing running" className="text-ink-2" />
          <Stat value={durStr(t.outsideHoursMs)} label="Worked outside hours" className="text-ink-2" />
        </div>
      </section>
    </div>
  );
}

/** Bar width. A zero target has nothing to be a fraction of: any work on such a
 *  day fills the bar, in the over-target colour (D36). */
function barPct(worked: number, target: number): number {
  if (target <= 0) return worked > 0 ? 100 : 0;
  return Math.min(100, (worked / target) * 100);
}

/** A percentage against a zero target is undefined, so a day off reads "over"
 *  when it has work and "—" when it does not (D36). */
function attainment(worked: number, target: number): string {
  if (target > 0) return `${Math.round((worked / target) * 100)}%`;
  return worked > 0 ? "over" : "—";
}

function share(ms: number, total: number): string {
  return total > 0 ? `${Math.round((ms / total) * 100)}%` : "—";
}

function Stat({ value, label, className = "" }: { value: string; label: string; className?: string }) {
  return (
    <div>
      <div className={`font-mono text-[17px] font-medium ${className}`}>{value}</div>
      <div className="text-[10.5px] text-ink-3">{label}</div>
    </div>
  );
}
