mod commands;
mod platform;
pub mod core;
mod state;
mod db;
mod error;

use db::Db;
use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        // A second launch focuses the running instance rather than starting a
        // rival timer against the same database.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let db = Db::open(&dir)?;

            // Expiry is resolved here, before any window can render. A block
            // that ended while the app was closed or the Mac was asleep must
            // appear as a checkpoint, never as a running or reset timer.
            let state = state::App::hydrate(db, state::now_ms())?;

            // Effects beyond ticking (checkpoint window, sound, notification)
            // are wired in Phases 5 and 6; the tick loop already drives the
            // state machine correctly without them.
            // The tick loop nudges the UI once a second while a block runs.
            // Between nudges the UI interpolates locally, so the countdown is
            // smooth without a command per frame.
            let handle = app.handle().clone();
            state.start_ticking(move |fx| {
                platform::checkpoint::apply(&handle, fx);
                let _ = handle.emit("timebox://changed", ());
            });

            app.manage(state);

            // Menu-bar utility: no Dock icon, no app switcher entry. The main
            // window is opened on demand and closing it must not quit the app.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::get_snapshot,
            commands::dispatch
        ])
        .run(tauri::generate_context!())
        .expect("error while running TimeBox");
}
