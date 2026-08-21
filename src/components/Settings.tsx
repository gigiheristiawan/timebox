import { useEffect } from "react";
import { useTimebox } from "../stores/useTimebox";
import type { Settings as SettingsData, Theme } from "../ipc/types";
import { MIN } from "../core/format";
import { SectionLabel } from "./ui";

const BLOCK_CHOICES = [15, 25, 30, 45, 60];
const BREAK_CHOICES = [5, 10, 15, 30];
const HOUR = 60 * MIN;

/**
 * The settings window (SPEC §4.4, task 7.4). Every change writes immediately —
 * there is no Save button and nothing to lose by closing the window.
 *
 * Nothing here can switch off the checkpoint. Settings choose defaults and
 * presentation; expiry always requires a decision.
 */
export function Settings() {
  const { snap, error, init, saveSettings } = useTimebox();

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

      <p className="border-t border-line pt-3 text-[12px] text-ink-3">
        Nothing here changes whether a checkpoint appears. Expiry always requires a decision.
      </p>
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
