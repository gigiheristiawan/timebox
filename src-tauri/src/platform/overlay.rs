//! The floating timer overlay (issue #23).
//!
//! A small always-on-top card on the main display showing what the current
//! block is and how much of it is left, so the timer is readable without
//! leaving the app being worked in.
//!
//! It is a *display*, not a second control surface: it ignores the cursor
//! entirely, so it can never take focus, swallow a click meant for the window
//! underneath, or become another thing to manage. Everything about it is
//! presentation — no setting here changes whether a checkpoint appears.

use crate::core::model::TimerState;
use crate::core::timer_machine::MachineState;
use crate::db::settings::{OverlayPosition, Settings};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "overlay";

/// The card's footprint, in logical points. Fixed rather than measured: the
/// window has to be re-anchored to a corner every time it changes size, and a
/// webview that resizes itself while Rust re-positions it is a loop with a
/// visible wobble. A long task title truncates instead.
const WIDTH: f64 = 200.0;
const HEIGHT_WITH_TITLE: f64 = 96.0;
const HEIGHT: f64 = 68.0;
/// Distance from the screen's edges. Clear of the menu bar at the top and of
/// the Dock's usual resting place at the bottom.
const MARGIN: f64 = 28.0;

/// Everything about the window that the settings decide. Compared rather than
/// re-applied blindly: `reconcile` runs once a second from the tick loop, and
/// re-sizing and re-positioning a visible window every second is both wasted
/// work and a visible wobble.
#[derive(PartialEq, Eq, Clone, Copy)]
struct Look {
    with_title: bool,
    position: OverlayPosition,
    opacity_pct: u8,
}

impl Look {
    fn of(s: &Settings) -> Self {
        Look {
            with_title: s.overlay_show_task_title,
            position: s.overlay_position,
            opacity_pct: s.overlay_opacity_pct,
        }
    }
}

/// The look last written to the window, or `None` if it has never been placed.
static APPLIED: parking_lot::Mutex<Option<Look>> = parking_lot::Mutex::new(None);

/// Bring the overlay into line with the settings and the current state.
///
/// Called from every path that can change either — the settings write, each
/// dispatch, and the tick loop — rather than from whichever is convenient, so
/// the window cannot be left showing a state the app has left. A hidden
/// overlay is never built at all.
pub fn reconcile(app: &AppHandle, settings: &Settings, state: &MachineState) {
    let want = settings.overlay_show && !at_checkpoint(state);

    let Some(window) = app.get_webview_window(LABEL) else {
        if want {
            if let Err(e) = build(app, settings) {
                eprintln!("[timebox] could not open the overlay: {e}");
            }
        }
        return;
    };

    if !want {
        let _ = window.hide();
        return;
    }

    let look = Look::of(settings);
    let was_visible = window.is_visible().unwrap_or(false);
    if !was_visible || *APPLIED.lock() != Some(look) {
        place(app, &window, settings);
        let _ = window.show();
        // Re-asserted whenever it comes back: another app going full screen can
        // win the level, and an overlay behind the window it describes is no
        // overlay.
        let _ = window.set_always_on_top(true);
    }
}

/// The checkpoint owns the screen while it is open (SPEC §7.4) and fills the
/// same display the overlay sits on. A card repeating "time's up" on top of the
/// window already saying so is noise, and the decision is the only thing that
/// should be readable at that moment.
fn at_checkpoint(state: &MachineState) -> bool {
    matches!(
        state.timer_state,
        TimerState::AwaitingDecision | TimerState::AwaitingPomodoro
    )
}

fn build(app: &AppHandle, settings: &Settings) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("TimeBox timer")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .closable(false)
        .shadow(true)
        // Never takes focus, and never steals the click the user meant for the
        // editor underneath. `focused(false)` covers the moment it is built;
        // `set_ignore_cursor_events` below covers every moment after.
        .focused(false)
        // Built hidden and shown once placed, so it never flashes at the
        // default size in the middle of the screen.
        .visible(false)
        .inner_size(WIDTH, HEIGHT_WITH_TITLE)
        .build()?;

    // The card is rounded, so the window behind it must not paint square
    // corners — the same public-API route the popover takes rather than
    // `transparent(true)`, which needs `macos-private-api`.
    #[cfg(target_os = "macos")]
    super::window_corners::round(&window);

    // Follows the user across Spaces. Without this the overlay stays on the
    // desktop it was opened on, which is the one desktop where the timer was
    // already visible.
    let _ = window.set_visible_on_all_workspaces(true);
    let _ = window.set_ignore_cursor_events(true);

    place(app, &window, settings);
    window.show()?;
    Ok(())
}

/// Size, position and opacity, in that order: a resize holds the top-left
/// corner, so the move is what lands the card in its chosen corner.
fn place(app: &AppHandle, window: &tauri::WebviewWindow, settings: &Settings) {
    let size = LogicalSize::new(
        WIDTH,
        if settings.overlay_show_task_title { HEIGHT_WITH_TITLE } else { HEIGHT },
    );
    if let Err(e) = window.set_size(size) {
        eprintln!("[timebox] could not size the overlay: {e}");
    }
    // Logical points throughout, and the monitor's *own* scale factor is what
    // converts its frame — the setters convert with the window's, which is the
    // display it currently sits on (issue #20).
    if let Some((origin, screen)) = super::checkpoint::main_monitor_frame(app) {
        let x = match settings.overlay_position {
            OverlayPosition::TopLeft | OverlayPosition::BottomLeft => origin.x + MARGIN,
            OverlayPosition::TopRight | OverlayPosition::BottomRight => {
                origin.x + screen.width - size.width - MARGIN
            }
            OverlayPosition::Center => origin.x + (screen.width - size.width) / 2.0,
        };
        let y = match settings.overlay_position {
            OverlayPosition::TopLeft | OverlayPosition::TopRight => origin.y + MARGIN,
            OverlayPosition::BottomLeft | OverlayPosition::BottomRight => {
                origin.y + screen.height - size.height - MARGIN
            }
            OverlayPosition::Center => origin.y + (screen.height - size.height) / 2.0,
        };
        if let Err(e) = window.set_position(LogicalPosition::new(x, y)) {
            eprintln!("[timebox] could not position the overlay: {e}");
        }
    }
    set_alpha(window, settings.overlay_opacity_pct);
    *APPLIED.lock() = Some(Look::of(settings));
}

/// The opacity slider, applied as the *window's* alpha.
///
/// The card's own background could be drawn semi-transparent instead, but only
/// the window alpha is certain to show the desktop through it: making the
/// webview itself transparent needs the private API the App Store rules out
/// (see `platform/window_corners.rs`). The whole card fades, text included,
/// which is what a translucent overlay looks like anyway.
#[cfg(target_os = "macos")]
fn set_alpha(window: &tauri::WebviewWindow, pct: u8) {
    use objc2::runtime::AnyObject;
    use objc2::msg_send;

    let Ok(ptr) = window.ns_window() else { return };
    unsafe {
        let ns_window = ptr as *mut AnyObject;
        let alpha = f64::from(pct.min(100)) / 100.0;
        let _: () = msg_send![ns_window, setAlphaValue: alpha];
    }
}

#[cfg(not(target_os = "macos"))]
fn set_alpha(_window: &tauri::WebviewWindow, _pct: u8) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 106. The overlay yields to the checkpoint, and to nothing else.
    /// Both waiting states, because a Pomodoro prompt puts the same full-screen
    /// window up as an expiry does — `AwaitingDecision` alone would leave the
    /// card floating over it (POMODORO_MODE's `at_any_checkpoint` lesson).
    /// IDLE is deliberately *not* on this list: nothing running is the state
    /// the overlay is most useful in (D47).
    #[test]
    fn t106_the_overlay_yields_to_either_checkpoint_and_stays_up_when_idle() {
        let hides = |ts| at_checkpoint(&MachineState { timer_state: ts, ..Default::default() });
        assert!(hides(TimerState::AwaitingDecision));
        assert!(hides(TimerState::AwaitingPomodoro));
        assert!(!hides(TimerState::Idle));
        assert!(!hides(TimerState::Running));
        assert!(!hides(TimerState::Paused));
    }
}
