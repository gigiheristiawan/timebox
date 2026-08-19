use crate::core::model::{Millis, Priority};
use crate::core::timer_machine::{Event, MachineState};
use crate::error::AppResult;
use crate::state::{now_ms, App};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub state: MachineState,
    /// The backend's instant. The UI interpolates its countdown against this
    /// rather than trusting the webview's clock, and never concludes expiry on
    /// its own — that is the backend's call (SPEC R7).
    pub now: Millis,
    pub remaining_ms: Millis,
    pub staleness_ms: Option<Millis>,
}

fn snapshot_of(app: &App) -> Snapshot {
    let now = now_ms();
    let state = app.snapshot();
    Snapshot {
        remaining_ms: state.remaining_ms(now),
        staleness_ms: state.staleness_ms(now),
        state,
        now,
    }
}

/// The complete set of things the UI may ask for. Anything not here cannot
/// happen, which is what keeps decision logic out of TypeScript.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    SwitchTo { task: String },
    Pause,
    Resume,
    Skip,
    CompleteCurrentTask,
    DecideComplete,
    DecidePending,
    DecideExtend { ms: Millis },
    DecideBreak { ms: Millis, complete: bool },
    EndBreak,
    ExtendBreak { ms: Millis },
    #[serde(rename_all = "camelCase")]
    AddTask { title: String, block_ms: Millis, priority: String },
    RemoveTask { task: String },
    Reorder { moved: String, before: String },
}

impl From<Action> for Event {
    fn from(a: Action) -> Self {
        match a {
            Action::SwitchTo { task } => Event::SwitchTo { task },
            Action::Pause => Event::Pause,
            Action::Resume => Event::Resume,
            Action::Skip => Event::Skip,
            Action::CompleteCurrentTask => Event::CompleteCurrentTask,
            Action::DecideComplete => Event::DecideComplete,
            Action::DecidePending => Event::DecidePending,
            Action::DecideExtend { ms } => Event::DecideExtend { ms },
            Action::DecideBreak { ms, complete } => Event::DecideBreak { ms, complete },
            Action::EndBreak => Event::EndBreak,
            Action::ExtendBreak { ms } => Event::ExtendBreak { ms },
            Action::AddTask { title, block_ms, priority } => Event::AddTask {
                title,
                block_ms,
                priority: Priority::parse(&priority).unwrap_or(Priority::Medium),
            },
            Action::RemoveTask { task } => Event::RemoveTask { task },
            Action::Reorder { moved, before } => Event::Reorder { moved, before },
        }
    }
}

#[tauri::command]
pub fn get_snapshot(app: State<'_, Arc<App>>) -> AppResult<Snapshot> {
    Ok(snapshot_of(&app))
}

/// Returns the resulting snapshot so the UI updates from the backend's own
/// state rather than predicting the outcome locally.
#[tauri::command]
pub fn dispatch(
    app: State<'_, Arc<App>>,
    handle: tauri::AppHandle,
    action: Action,
) -> AppResult<Snapshot> {
    let fx = app.dispatch(action.into(), now_ms())?;
    crate::platform::checkpoint::apply(&handle, &fx);
    crate::platform::tray::refresh(&handle, &app.snapshot(), now_ms());
    let _ = tauri::Emitter::emit(&handle, "timebox://changed", ());
    Ok(snapshot_of(&app))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub database_path: String,
    pub schema_version: i64,
    pub journal_mode: String,
}

#[tauri::command]
pub fn health_check(app: State<'_, Arc<App>>) -> AppResult<HealthReport> {
    Ok(HealthReport {
        database_path: app.db.path().display().to_string(),
        schema_version: app.db.schema_version()?,
        journal_mode: app.db.journal_mode()?,
    })
}

// --------------------------------------------------------- window plumbing
//
// The popover is the primary surface (SPEC §7.2), so it needs the three ways
// out of it. None of these touch domain state.

/// Bring the main window forward, recreating it if a previous close hid it away
/// or it was never built.
#[tauri::command]
pub fn open_main_window(handle: tauri::AppHandle) -> Result<(), String> {
    crate::platform::popover::hide(&handle);
    let window = match tauri::Manager::get_webview_window(&handle, "main") {
        Some(w) => w,
        None => tauri::WebviewWindowBuilder::new(
            &handle,
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("TimeBox")
        .inner_size(620.0, 640.0)
        .min_inner_size(460.0, 480.0)
        .build()
        .map_err(|e| e.to_string())?,
    };
    window.show().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_popover(handle: tauri::AppHandle) {
    crate::platform::popover::hide(&handle);
}

/// D14's "quit while a block is running" confirmation is task 7.11; until then
/// this quits directly. `end_at` is absolute, so quitting does not lose the
/// block — hydrate resolves it on the next launch.
#[tauri::command]
pub fn quit_app(handle: tauri::AppHandle) {
    handle.exit(0);
}
