use crate::core::model::{Millis, Priority, TimerState};
use crate::core::summary::{summarize, Summary};
use crate::core::timer_machine::{Event, MachineState};
use crate::db::settings::Settings;
use crate::error::AppResult;
use crate::state::{day_start_ms, now_ms, App};
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
    /// Today's numbers and the capacity strip, computed by `core::summary`.
    /// Carried on the snapshot so the UI has one source of truth and no
    /// arithmetic of its own (SPEC R7).
    pub summary: Summary,
    /// Settings ride along for the same reason: one channel, no second store
    /// for the UI to keep in sync.
    pub settings: Settings,
}

fn snapshot_of(app: &App) -> Snapshot {
    let now = now_ms();
    let state = app.snapshot();
    let settings = app.settings();
    Snapshot {
        remaining_ms: state.remaining_ms(now),
        staleness_ms: state.staleness_ms(now),
        summary: summarize(&state, day_start_ms(now), now, settings.available_work_ms_per_day),
        settings,
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
    crate::platform::checkpoint::apply(&handle, &fx, &app.settings());
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

// ------------------------------------------------------------------ settings

/// Writing settings can change three things outside the database — the menu bar
/// title, the login item, and the theme every window paints — so they are all
/// applied here rather than left for a caller to remember.
#[tauri::command]
pub fn update_settings(
    app: State<'_, Arc<App>>,
    handle: tauri::AppHandle,
    settings: Settings,
) -> AppResult<Snapshot> {
    let stored = app.set_settings(&settings)?;

    crate::platform::tray::set_show_timer(stored.menu_bar_show_timer);
    crate::platform::tray::refresh_forced(&handle, &app.snapshot(), now_ms());
    apply_launch_at_login(&handle, stored.launch_at_login);

    let _ = tauri::Emitter::emit(&handle, "timebox://changed", ());
    Ok(snapshot_of(&app))
}

/// Best-effort: a login item that cannot be registered must not fail the save.
/// The switch reflects the stored preference; a failure is logged, not hidden.
fn apply_launch_at_login(_handle: &tauri::AppHandle, on: bool) {
    #[cfg(target_os = "macos")]
    if let Err(e) = crate::platform::login_item::set(on) {
        eprintln!("[timebox] launch at login could not be {}: {e}", if on { "enabled" } else { "disabled" });
    }
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

/// Settings is its own window rather than a panel in the main one, matching
/// the prototype and keeping `Cmd+,` meaningful from the popover too.
#[tauri::command]
pub fn open_settings_window(handle: tauri::AppHandle) -> Result<(), String> {
    crate::platform::popover::hide(&handle);
    let window = match tauri::Manager::get_webview_window(&handle, "settings") {
        Some(w) => w,
        None => tauri::WebviewWindowBuilder::new(
            &handle,
            "settings",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("Settings")
        .inner_size(376.0, 460.0)
        .resizable(false)
        .maximizable(false)
        .build()
        .map_err(|e| e.to_string())?,
    };
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_popover(handle: tauri::AppHandle) {
    crate::platform::popover::hide(&handle);
}

/// Quitting is never blocked — the confirm exists to make the cost visible, not
/// to prevent it (SPEC D14). `end_at` is absolute, so a plain quit keeps the
/// block running; hydrate resolves it on the next launch.
///
/// From IDLE, PAUSED, or a checkpoint there is nothing to warn about: the first
/// two hold no live allocation and a checkpoint is restored intact.
#[tauri::command]
pub fn request_quit(app: State<'_, Arc<App>>, handle: tauri::AppHandle) {
    if app.snapshot().timer_state == TimerState::Running {
        crate::platform::quit_confirm::show(&handle);
    } else {
        handle.exit(0);
    }
}

/// The answer to that confirm. `pause` holds the remainder first, which is the
/// only way to stop the clock consuming the block while the app is closed.
#[tauri::command]
pub fn confirm_quit(app: State<'_, Arc<App>>, handle: tauri::AppHandle, pause: bool) {
    if pause {
        if let Err(e) = app.dispatch(Event::Pause, now_ms()) {
            eprintln!("[timebox] could not pause before quitting: {e}");
        }
    }
    handle.exit(0);
}

#[tauri::command]
pub fn cancel_quit(handle: tauri::AppHandle) {
    crate::platform::quit_confirm::hide(&handle);
}
