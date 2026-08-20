import { durStr } from "../core/format";
import { useTimebox } from "../stores/useTimebox";
import { SectionLabel } from "./ui";

/**
 * `.cap` in docs/mockup.html. Over-capacity is *shown*, never blocked — the
 * app's job is to make the day's arithmetic visible, not to refuse work
 * (SPEC §7.3). Every figure comes from `core::summary`.
 */
export function Capacity() {
  const snap = useTimebox((s) => s.snap);
  if (!snap) return null;
  const { availableMs, allocatedMs, unallocatedMs, over } = snap.summary.capacity;

  return (
    <section className="flex flex-col gap-[9px]">
      <SectionLabel>Capacity</SectionLabel>
      <div className="flex flex-wrap items-baseline gap-5">
        <Figure value={durStr(availableMs)} label="Available today" />
        <Figure value={durStr(allocatedMs)} label="Allocated" />
        <Figure
          value={`${over ? "+" : ""}${durStr(Math.abs(unallocatedMs))}`}
          label={over ? "Over capacity" : "Unallocated"}
          alert={over}
        />
      </div>
    </section>
  );
}

function Figure({ value, label, alert = false }: { value: string; label: string; alert?: boolean }) {
  return (
    <div className="flex flex-col gap-px">
      <span className={`font-mono text-[15px] font-medium ${alert ? "text-alert" : ""}`}>{value}</span>
      <span className="text-[10.5px] tracking-[0.04em] text-ink-3">{label}</span>
    </div>
  );
}
