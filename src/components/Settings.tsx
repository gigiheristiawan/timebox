import { useEffect } from "react";
import { useTimebox } from "../stores/useTimebox";
import type { Settings as SettingsData, Theme } from "../ipc/types";
import { MIN } from "../core/format";
import { SectionLabel } from "./ui";

const BLOCK_CHOICES = [15, 25, 30, 45, 60];
const BREAK_CHOICES = [5, 10, 15, 30];
const HOUR = 60 * MIN;
/** Monday = bit 0, matching the stored bitmask. */
const WEEKDAYS = ["M", "T", "W", "T", "F", "S", "S"];

/**
 * The settings window (SPEC §4.4, task 7.4). Every change writes immediately —
 * there is no Save button and nothing to lose by closing the window.
 *
 * Nothing here can switch off the checkpoint. Settings choose defaults and
 * presentation; expiry always requires a decision.
 */
export function Settings() {
  const { snap, error, init, saveSettings, send } = useTimebox();

  useEffect(() => {
    let un: (() => void) | undefined;
    void init().then((u) => { un = u; });
    return () => un?.();
  }, [init]);

  if (!snap) return <main className="min-h-screen bg-surface" />;
  const s = snap.settings;
  const set = (patch: Partial<SettingsData>) => void saveSettings({ ...s, ...patch });

  return (
    <main className="flex min-h-screen flex-col gap-5 px-5 py-[18px]">
      {error && <p className="rounded-lg bg-alert-soft px-3 py-2 text-xs text-alert">{error}</p>}

      <div className="flex flex-col gap-[9px]">
        <SectionLabel>General</SectionLabel>
        <Toggle
          label="Launch at login"
          on={s.launchAtLogin}
          onChange={(v) => set({ launchAtLogin: v })}
          note={
            s.launchAtLogin && !snap.launchAtLoginActive
              ? "macOS refused to register it. Move TimeBox to your Applications folder, then switch this off and on again."
              : undefined
          }
        />
        <Row label="Theme">
          <select
            className="field"
            aria-label="Theme"
            value={s.theme}
            onChange={(e) => set({ theme: e.target.value as Theme })}
          >
            <option value="System">System</option>
            <option value="Light">Light</option>
            <option value="Dark">Dark</option>
          </select>
        </Row>
        <Toggle
          label="Show the timer in the menu bar"
          on={s.menuBarShowTimer}
          onChange={(v) => set({ menuBarShowTimer: v })}
        />
      </div>

      <div className="flex flex-col gap-[9px]">
        <SectionLabel>Timer</SectionLabel>
        <Toggle
          label="Pomodoro mode"
          // Not a settings write: the mode and the instant it counts from are
          // one change, and the reducer refuses the flip while a checkpoint is
          // open (POMODORO_MODE D33, §4.6).
          on={snap.pomodoro !== null}
          onChange={(v) => void send({ kind: "setPomodoroMode", on: v })}
          note={`Offers a break every 25 minutes of work, on top of each task's own block. Breaks use the ${Math.round(s.defaultBreakDurationMs / 60_000)}-minute default below.`}
        />
        <Row label="Default block duration">
          <MinuteSelect
            label="Default block duration"
            ms={s.defaultBlockDurationMs}
            choices={BLOCK_CHOICES}
            onChange={(ms) => set({ defaultBlockDurationMs: ms })}
          />
        </Row>
        <Row label="Default break duration">
          <MinuteSelect
            label="Default break duration"
            ms={s.defaultBreakDurationMs}
            choices={BREAK_CHOICES}
            onChange={(ms) => set({ defaultBreakDurationMs: ms })}
          />
        </Row>
        <Row label="Available working time / day">
          <input
            type="number"
            aria-label="Available working time per day, in hours"
            className="field w-[76px]"
            min={1}
            max={16}
            step={0.5}
            value={s.availableWorkMsPerDay / HOUR}
            onChange={(e) => {
              const hours = Number(e.target.value);
              if (Number.isFinite(hours) && hours > 0) set({ availableWorkMsPerDay: Math.round(hours * HOUR) });
            }}
          />
        </Row>
        <Toggle
          label="Expiration sound"
          on={s.expirationSound}
          onChange={(v) => set({ expirationSound: v })}
        />
        <Toggle
          label="macOS notification"
          on={s.systemNotification}
          onChange={(v) => set({ systemNotification: v })}
        />
      </div>

      <div className="flex flex-col gap-[9px]">
        <SectionLabel>Working hours</SectionLabel>
        {/* When you are at the desk — not how much of the day you intend to
            give, which is the capacity setting above. 09:00–18:00 with 7h of
            capacity is a normal configuration (IDLE_TIME §6). Idle is only
            measured inside this window; work outside it is still recorded. */}
        <Row label="From">
          <TimeField
            label="Working hours start"
            ms={s.workStartMs}
            onChange={(ms) => ms < s.workEndMs && set({ workStartMs: ms })}
          />
        </Row>
        <Row label="To">
          <TimeField
            label="Working hours end"
            ms={s.workEndMs}
            onChange={(ms) => ms > s.workStartMs && set({ workEndMs: ms })}
          />
        </Row>
        <Row label="Days">
          <div className="flex gap-1">
            {WEEKDAYS.map((d, i) => {
              const on = (s.workingWeekdays & (1 << i)) !== 0;
              return (
                <button
                  key={i}
                  type="button"
                  role="switch"
                  aria-checked={on}
                  aria-label={`Working day ${i + 1}`}
                  onClick={() => set({ workingWeekdays: s.workingWeekdays ^ (1 << i) })}
                  className={`h-[22px] w-[22px] rounded text-[11px] font-medium transition-colors ${
                    on ? "bg-accent text-white" : "bg-line-2 text-ink-3 hover:bg-surface-3"
                  }`}
                >
                  {d}
                </button>
              );
            })}
          </div>
        </Row>
        <p className="text-[11.5px] leading-snug text-ink-3">
          Time inside this window that no block covered counts as idle. Work outside it is still
          recorded — it is never treated as an error.
        </p>
      </div>

      <div className="flex items-baseline gap-2 border-t border-line pt-3 text-[12px] text-ink-3">
        <p>Nothing here changes whether a checkpoint appears. Expiry always requires a decision.</p>
        {/* Inlined from tauri.conf.json at build time, so it is the version of
            the bundle actually running — not a second copy to keep in step. */}
        <span className="ml-auto flex-none font-mono text-[11px]">v{__APP_VERSION__}</span>
      </div>
    </main>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2.5 text-[13px]">
      <span className="text-ink-2">{label}</span>
      <span className="ml-auto">{children}</span>
    </div>
  );
}

function MinuteSelect({
  label, ms, choices, onChange,
}: {
  label: string;
  ms: number;
  choices: number[];
  onChange: (ms: number) => void;
}) {
  // A stored value outside the list (hand-edited, or a future choice) must not
  // silently snap to something else the moment the window opens.
  const minutes = Math.round(ms / MIN);
  const options = choices.includes(minutes) ? choices : [minutes, ...choices].sort((a, b) => a - b);
  return (
    <select
      className="field"
      aria-label={label}
      value={minutes}
      onChange={(e) => onChange(Number(e.target.value) * MIN)}
    >
      {options.map((m) => <option key={m} value={m}>{m} min</option>)}
    </select>
  );
}

/** A wall-clock time of day, stored as milliseconds from local midnight.
 *  `<input type="time">` gives the platform picker and the user's own 12/24h
 *  formatting for free, and can only produce a valid time of day. */
function TimeField({ label, ms, onChange }: { label: string; ms: number; onChange: (ms: number) => void }) {
  const hh = String(Math.floor(ms / HOUR)).padStart(2, "0");
  const mm = String(Math.floor((ms % HOUR) / MIN)).padStart(2, "0");
  return (
    <input
      type="time"
      aria-label={label}
      className="field"
      value={`${hh}:${mm}`}
      onChange={(e) => {
        // An empty or half-typed field yields undefined; leave the stored
        // value alone rather than writing a window of zero length.
        const [h, m] = e.target.value.split(":");
        const [hours, minutes] = [Number(h), Number(m)];
        if (Number.isFinite(hours) && Number.isFinite(minutes) && h !== undefined && m !== undefined) {
          onChange(hours * HOUR + minutes * MIN);
        }
      }}
    />
  );
}

/** `.switch` in docs/mockup.html. */
/** `note` carries a reason the switch did not take effect. It is shown only
 *  when the backend reports that the system disagrees with the setting, so a
 *  toggle can never sit there claiming something that is not true. */
function Toggle({ label, on, onChange, note }: { label: string; on: boolean; onChange: (v: boolean) => void; note?: string | undefined }) {
  return (
    <div className="flex flex-col gap-1">
    <div className="flex items-center gap-2.5 text-[13px]">
      <span className="text-ink-2">{label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={on}
        aria-label={label}
        onClick={() => onChange(!on)}
        className={`relative ml-auto h-[22px] w-[38px] flex-none rounded-full transition-colors ${
          on ? "bg-accent" : "bg-line-2"
        }`}
      >
        <span
          className={`absolute left-0.5 top-0.5 h-[18px] w-[18px] rounded-full bg-white shadow transition-transform ${
            on ? "translate-x-4" : ""
          }`}
        />
      </button>
    </div>
    {note && <p className="text-[11px] leading-snug text-alert">{note}</p>}
    </div>
  );
}
