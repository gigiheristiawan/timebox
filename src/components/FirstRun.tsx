import { useTimebox } from "../stores/useTimebox";

/**
 * SPEC D12. `LSUIElement` means no Dock icon and no Cmd-Tab entry, so a user
 * who closes this window has nothing to click unless they know where the app
 * lives. Shown once, dismissed forever — the flag is stored, not remembered
 * in React state, so a relaunch does not bring it back.
 */
export function FirstRun() {
  const { snap, saveSettings } = useTimebox();
  if (!snap || snap.settings.firstRunDone) return null;

  return (
    <aside className="flex items-start gap-3 rounded-lg border border-accent bg-accent-soft px-4 py-3 text-accent-ink">
      <span aria-hidden className="text-[15px] leading-none">↑</span>
      <div className="flex flex-col gap-1 text-[12.5px] leading-snug">
        <strong className="text-[13px] font-semibold">TimeBox lives in the menu bar.</strong>
        <span>
          There is no Dock icon and no app switcher entry. Click the ◉ at the top of the screen — or
          press <kbd className="font-mono">⌘⇧T</kbd> — to reach it any time. Closing this window
          leaves the timer running.
        </span>
      </div>
      <button
        type="button"
        onClick={() => void saveSettings({ ...snap.settings, firstRunDone: true })}
        className="ml-auto flex-none rounded-[7px] border border-accent px-[11px] py-1 text-[12.5px] font-medium hover:bg-surface"
      >
        Got it
      </button>
    </aside>
  );
}
