import { invoke } from "@tauri-apps/api/core";
import type { Action, HealthReport, Settings, Snapshot } from "./types";

/** Typed wrappers over the Rust command surface. The UI calls nothing else. */

export function getSnapshot(): Promise<Snapshot> {
  return invoke<Snapshot>("get_snapshot");
}

export function dispatch(action: Action): Promise<Snapshot> {
  return invoke<Snapshot>("dispatch", { action });
}

export function healthCheck(): Promise<HealthReport> {
  return invoke<HealthReport>("health_check");
}

/** Settings are written whole and come back stored — the backend may clamp a
 *  value, and the UI must show what was actually kept, not what it sent. */
export function updateSettings(settings: Settings): Promise<Snapshot> {
  return invoke<Snapshot>("update_settings", { settings });
}

/** Window plumbing for the popover (SPEC §7.2). No domain state involved. */

export function openMainWindow(): Promise<void> {
  return invoke<void>("open_main_window");
}

export function closePopover(): Promise<void> {
  return invoke<void>("close_popover");
}

export function openSettingsWindow(): Promise<void> {
  return invoke<void>("open_settings_window");
}

/**
 * Quitting (IDLE_TIME §9.1). It is never confirmed and never blocked: quitting
 * parks the running block (D16), so there is no cost left to warn about.
 */
export function requestQuit(): Promise<void> {
  return invoke<void>("request_quit");
}
