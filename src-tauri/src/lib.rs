mod commands;
mod platform;
pub mod core;
mod state;
mod db;
mod error;

use db::Db;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// SPEC §9. Registered app-wide so the popover is reachable without hunting for
/// the menu bar icon, which the notch can hide entirely (D12).
fn toggle_popover_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyT)
}

pub fn run() {
    tauri::Builder::default()
        // A second launch opens the popover rather than starting a rival timer
        // against the same database. Merely focusing an invisible accessory app
        // reads as a broken launch (D12, acceptance test 21).
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            platform::popover::show(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let db = Db::open(&dir)?;

            // Expiry is resolved here, before any window can render. A block
            // that ended while the app was closed or the Mac was asleep must
            // appear as a checkpoint, never as a running or reset timer.
            let state = state::App::hydrate(db, state::now_ms())?;
            platform::tray::set_show_timer(state.settings().menu_bar_show_timer);


            platform::tray::init(app.handle())?;
            platform::tray::refresh(app.handle(), &state.snapshot(), state::now_ms());

            // Closing the main window hides it; the app lives in the menu bar
            // and only Quit ends it (SPEC §7.3).
            if let Some(main) = app.get_webview_window("main") {
                let w = main.clone();
                main.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }

            let handle = app.handle().clone();
            app.global_shortcut()
                .on_shortcut(toggle_popover_shortcut(), move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        platform::popover::toggle(app);
                    }
                })
                .unwrap_or_else(|e| eprintln!("[timebox] Cmd+Shift+T unavailable: {e}"));

            // The tick loop nudges the UI once a second while a block runs, and
            // is also what paces the menu bar title (task 6.3). Between nudges
            // the UI interpolates locally, so the countdown is smooth without a
            // command per frame.
            let ticker_handle = handle.clone();
            let app_state = state.clone();
            state.start_ticking(move |fx| {
                platform::checkpoint::apply(&ticker_handle, fx, &app_state.settings());
                platform::tray::refresh(&ticker_handle, &app_state.snapshot(), state::now_ms());
                let _ = ticker_handle.emit("timebox://changed", ());
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
            commands::dispatch,
            commands::update_settings,
            commands::open_main_window,
            commands::open_settings_window,
            commands::close_popover,
            commands::request_quit,
            commands::confirm_quit,
            commands::cancel_quit
        ])
        .build(tauri::generate_context!())
        .expect("error while building TimeBox")
        // Cmd+Q reaches the app as an exit request rather than a command, so
        // D14's confirmation has to be hooked here as well as on the popover's
        // Quit item. `handle.exit(0)` carries a code, which is how an answered
        // confirm gets through without re-asking.
        .run(|app, event| {
            // Launch Services will not start a second copy of an installed
            // bundle; it reopens the running one. That path is what a sandboxed
            // build gets instead of `single-instance`, whose /tmp socket the
            // sandbox denies — so the popover has to be reachable from here
            // too (D12, acceptance test 21).
            if let tauri::RunEvent::Reopen { .. } = &event {
                platform::popover::show(app);
            }
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = &event {
                let running = app
                    .try_state::<std::sync::Arc<state::App>>()
                    .map(|s| s.snapshot().timer_state == core::model::TimerState::Running)
                    .unwrap_or(false);
                if running {
                    api.prevent_exit();
                    platform::quit_confirm::show(app);
                }
            }
        });
}
