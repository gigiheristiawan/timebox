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

            // The login item is system state, not ours, and it can drift from
            // the stored preference — the user disables it in System Settings,
            // or the app was moved into /Applications after the toggle was set.
            // Reconciling here is the only thing that ever notices.
            // Before reconciling: 0.1.0 registered launch-at-login by writing a
            // LaunchAgent plist, which would otherwise keep starting the app
            // behind the setting's back.
            #[cfg(target_os = "macos")]
            {
                platform::login_item::remove_legacy_launch_agent();
                platform::login_item::reconcile(state.settings().launch_at_login);
            }

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
            commands::get_report,
            commands::dispatch,
            commands::update_settings,
            commands::open_main_window,
            commands::open_settings_window,
            commands::close_popover,
            commands::request_quit
        ])
        .build(tauri::generate_context!())
        .expect("error while building TimeBox")
        // Quitting has to park the running block (D16) whichever door it came
        // through, so the hook lives here as well as on the popover's Quit
        // item, which goes through `commands::request_quit`.
        .run(|app, event| {
            // Launch Services will not start a second copy of an installed
            // bundle; it reopens the running one. That path is what a sandboxed
            // build gets instead of `single-instance`, whose /tmp socket the
            // sandbox denies — so the popover has to be reachable from here
            // too (D12, acceptance test 21).
            if let tauri::RunEvent::Reopen { .. } = &event {
                platform::popover::show(app);
            }
            // A window coming forward is showing whatever snapshot it last
            // received. The tick loop is the only thing that pushes fresh
            // numbers, and it parks whenever the timer is not RUNNING — which
            // is exactly when idle accrues (IDLE_TIME §3). Windows are hidden
            // rather than destroyed, so the webview never remounts and never
            // refetches on its own. Nudging on focus is what makes reopening a
            // window show the truth rather than the last thing it was told.
            if let tauri::RunEvent::WindowEvent { event: WindowEvent::Focused(true), .. } = &event {
                let _ = app.emit("timebox://changed", ());
            }
            // Quitting *is* a pause (IDLE_TIME D16): the block is parked so the
            // interval the app is closed reads as idle rather than as work.
            //
            // This must hang off `Exit`, not `ExitRequested`. `Cmd+Q` is muda's
            // predefined Quit item, which sends `terminate:` straight to
            // NSApplication; tao sees that as `applicationWillTerminate` and
            // emits `Exit` alone, with no `ExitRequested` anywhere in the
            // sequence. Hooked on `ExitRequested` the park silently never ran
            // and the block came back RUNNING — the clock had kept going across
            // the quit, which is the exact thing D16 removes.
            //
            // `Exit` is the one event every path reaches, `handle.exit(0)`
            // included, and `Event::Pause` is a no-op unless the timer is
            // RUNNING, so arriving here already parked costs nothing.
            // `dispatch` writes the whole state before it returns, so the park
            // is durable by the time the process goes away.
            if let tauri::RunEvent::Exit = &event {
                if let Some(s) = app.try_state::<std::sync::Arc<state::App>>() {
                    commands::park_for_quit(&s);
                }
            }
        });
}
