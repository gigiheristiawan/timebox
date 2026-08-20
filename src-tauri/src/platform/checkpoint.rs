//! The expiration checkpoint window.
//!
//! Blocking applies to TimeBox's own UI only. The app never tries to lock the
//! Mac or interfere with other applications — it uses ordinary macOS window
//! activation to demand attention (SPEC §7.4).

use crate::core::model::BlockKind;
use crate::core::timer_machine::Effect;
use crate::db::settings::Settings;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_notification::NotificationExt;

pub const LABEL: &str = "checkpoint";

/// Apply the effects a transition produced. Called from both the tick loop and
/// the command surface so a checkpoint reached by either path behaves the same.
/// Settings gate the *announcement* only. The window itself is not optional —
/// no setting makes an expired block resolve without a decision (SPEC §7.4).
pub fn apply(app: &AppHandle, fx: &[Effect], settings: &Settings) {
    for e in fx {
        match e {
            Effect::EnterCheckpoint { .. } => {
                if let Err(err) = show(app) {
                    eprintln!("[timebox] could not show the checkpoint: {err}");
                }
            }
            Effect::LeaveCheckpoint => hide(app),
            Effect::PlayExpirySound if settings.expiration_sound => play_sound(),
            Effect::Notify { kind, task_title, allocated_minutes }
                if settings.system_notification =>
            {
                notify(app, *kind, task_title.as_deref(), *allocated_minutes)
            }
            _ => {}
        }
    }
}

/// Fill the display the user is actually looking at — the one under the cursor —
/// rather than always the primary.
fn show(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.show()?;
        w.set_always_on_top(true)?;
        w.set_focus()?;
        return Ok(());
    }

    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());

    let mut builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Time's up")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .visible(true);

    if let Some(m) = monitor {
        let scale = m.scale_factor();
        let pos = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        builder = builder
            .position(pos.x, pos.y)
            .inner_size(size.width, size.height);
    }

    let window = builder.build()?;

    // There is no exit path from a checkpoint. Cmd+W and any other close route
    // are refused; only a decision dismisses it (SPEC §7.4).
    let w = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = w.set_focus();
        }
    });

    window.set_focus()?;
    Ok(())
}

fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}

/// A subtle system sound. Spawned rather than linked so a missing or muted
/// sound can never delay or fail a state transition.
fn play_sound() {
    let _ = std::process::Command::new("afplay")
        .arg("/System/Library/Sounds/Glass.aiff")
        .spawn();
}

/// Best-effort. If the user denied notification permission the checkpoint
/// window and the sound still fire, so the app stays fully functional.
fn notify(app: &AppHandle, kind: BlockKind, task: Option<&str>, minutes: i64) {
    let (title, body) = match kind {
        BlockKind::Break => (
            "BREAK'S OVER".to_string(),
            format!("Your {minutes}-minute break has ended. Pick up the queue when you're ready."),
        ),
        BlockKind::Work => (
            "TIME'S UP".to_string(),
            format!(
                "{}\nYour {minutes}-minute time block has ended. Choose what to do next.",
                task.unwrap_or("Current task")
            ),
        ),
    };
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        eprintln!("[timebox] notification unavailable: {e}");
    }
}
