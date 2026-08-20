/**
 * Theme application (task 7.7). `styles.css` defines all three viewer states:
 * the system preference via `prefers-color-scheme`, and the explicit choices
 * via `data-theme`. Choosing System means removing the attribute entirely so
 * the media query governs again — leaving a stale attribute behind is what
 * makes a "System" setting stop following the system.
 */
import type { Theme } from "./ipc/types";

export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "System") root.removeAttribute("data-theme");
  else root.dataset.theme = theme === "Dark" ? "dark" : "light";
}
