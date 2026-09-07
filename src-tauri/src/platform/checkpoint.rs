//! The expiration checkpoint window.
//!
//! Blocking applies to TimeBox's own UI only. The app never tries to lock the
//! Mac or interfere with other applications — it uses ordinary macOS window
//! activation to demand attention (SPEC §7.4).

use crate::core::model::CheckpointKind;
use crate::core::timer_machine::Effect;
use crate::db::settings::Settings;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
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

/// Fill the main display — the one carrying the menu bar, which is what macOS's
/// "Main display" setting names.
fn show(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        // The window is reused across checkpoints, so its geometry is whatever
        // the display it was last shown on dictated. Displays come and go —
        // unplug the external monitor a checkpoint was sized for and the stored
        // frame no longer describes any screen, which is how the window came
        // back covering half of the built-in one (issue #20). Re-fit on every
        // show rather than trusting the frame.
        fit_to_main_monitor(app, &w);
        w.show()?;
        w.set_always_on_top(true)?;
        w.set_focus()?;
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Time's up")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        // Built hidden and shown only once it has been fitted, so the window
        // never flashes at the default size on the wrong display.
        .visible(false);

    let window = builder.build()?;
    fit_to_main_monitor(app, &window);
    window.show()?;

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

/// The frame of the main display, in **logical** points, falling back to the
/// one under the cursor.
///
/// The cursor's display was the original rule (SPEC §8) on the argument that it
/// is where the user is looking. It is not, reliably: the pointer is wherever
/// it was abandoned, which on a two-monitor desk is routinely a screen nobody
/// is facing, and the checkpoint would then open somewhere different each time
/// for no reason the user can see. The main display is the one macOS itself
/// treats as the front of the desk, and it is a *setting* — so the checkpoint
/// always lands in the same, chosen place.
///
/// Units: a monitor reports its frame in physical pixels, and the conversion
/// has to use *that monitor's* scale factor — but the setters below convert
/// whatever they are handed using the **window's** scale factor, which is the
/// display it currently sits on. Hand them physical pixels and a Retina window
/// moving to a 1× display (or the reverse) is sized by the wrong factor: that
/// is how the window came out twice the screen with its content centred off the
/// bottom-right corner. Converting here and passing logical makes their
/// conversion an identity, so only the monitor's own factor is ever used.
fn main_monitor_frame(app: &AppHandle) -> Option<(LogicalPosition<f64>, LogicalSize<f64>)> {
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| {
            app.cursor_position()
                .ok()
                .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        })?;
    let scale = monitor.scale_factor();
    Some((
        monitor.position().to_logical(scale),
        monitor.size().to_logical(scale),
    ))
}

/// Best-effort: a checkpoint that is mispositioned is still a checkpoint, and
/// failing to place it must never stop it being shown.
fn fit_to_main_monitor(app: &AppHandle, window: &WebviewWindow) {
    let Some((pos, size)) = main_monitor_frame(app) else {
        return;
    };
    // Size first, then position: resizing keeps the top-left corner, so the
    // move is what lands the window on the right screen.
    if let Err(e) = window.set_size(size).and_then(|_| window.set_position(pos)) {
        eprintln!("[timebox] could not fit the checkpoint to the display: {e}");
    }
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
fn notify(app: &AppHandle, kind: CheckpointKind, task: Option<&str>, minutes: i64) {
    let (title, body) = match kind {
        CheckpointKind::Pomodoro => (
            "TIME FOR A BREAK".to_string(),
            format!(
                "{}\nYou've worked {minutes} minutes straight. Take a break, or keep going.",
                task.unwrap_or("Current task")
            ),
        ),
        CheckpointKind::Break => (
            "BREAK'S OVER".to_string(),
            format!("Your {minutes}-minute break has ended. Pick up the queue when you're ready."),
        ),
        CheckpointKind::Work => (
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
