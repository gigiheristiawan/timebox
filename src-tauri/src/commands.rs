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
