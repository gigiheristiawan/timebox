import { durStr } from "../core/format";
import { useTimebox } from "../stores/useTimebox";
import { SectionLabel } from "./ui";

/** Switching this often in a day is worth noticing, not scolding. */
const CHURN_THRESHOLD = 2;

/**
 * `.today` in docs/mockup.html. Break time and away time are separate columns
 * from worked time on purpose: a block ending is not a task finishing, and a
 * checkpoint nobody answered is neither work nor rest (SPEC D7, D13).
 */
export function Today() {
  const snap = useTimebox((s) => s.snap);
  if (!snap) return null;
  const t = snap.summary.today;
  const churn = t.switchedEarly > CHURN_THRESHOLD;

  return (
    <section className="flex flex-col gap-[9px]">
      <SectionLabel>Today</SectionLabel>
      <div className="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(96px,1fr))]">
        <Stat value={durStr(t.workedMs)} label="Worked" />
        <Stat value={String(t.tasksCompleted)} label="Tasks completed" />
        <Stat value={String(t.tasksPending)} label="Tasks pending" />
        <Stat value={String(t.blocksCompleted)} label="Blocks completed" />
        <Stat value={durStr(t.breakMs)} label="On break" className="text-rest-ink" />
        {/* IDLE_TIME §3. Named as what it measures — window time no block
            covered — never as "time you were not working"; the app does not
            observe the human. Away is a *cause* of it (§3.1), not a peer, so
            the causes belong in the report rather than beside the total. */}
        <Stat value={durStr(t.idleMs)} label="Idle in working hours" className="text-ink-2" />
        <Stat
          value={`${t.switchedEarly}×${churn ? " ⚠" : ""}`}
          label="Switched early"
          className={churn ? "text-warn" : ""}
        />
      </div>

      {t.top.length > 0 && (
        <div className="flex flex-col gap-[3px] text-[12.5px] text-ink-2">
          {t.top.map((task, i) => (
            <div key={task.taskId} className="flex gap-2">
              <span className="font-mono text-ink-3">{i + 1}</span>
              <span className="min-w-0 truncate">{task.title}</span>
              <span className="ml-auto flex-none font-mono text-[11.5px]">{durStr(task.ms)}</span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function Stat({ value, label, className = "" }: { value: string; label: string; className?: string }) {
  return (
    <div>
      <div className={`font-mono text-[17px] font-medium ${className}`}>{value}</div>
      <div className="text-[10.5px] text-ink-3">{label}</div>
    </div>
  );
}
