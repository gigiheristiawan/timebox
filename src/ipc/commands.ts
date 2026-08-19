import { invoke } from "@tauri-apps/api/core";
import type { Action, HealthReport, Snapshot } from "./types";

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
