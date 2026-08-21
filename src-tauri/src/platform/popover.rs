//! The menu bar popover.
//!
//! macOS has a real `NSPopover`; Tauri does not expose one, so this is an
//! undecorated always-on-top window anchored under the tray icon. It behaves
//! like a popover in the way that matters: it closes as soon as it loses focus,
//! so it never becomes a second window to manage.

use std::time::{Duration, Instant};
use tauri::{
    AppHandle, LogicalPosition, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};

pub const LABEL: &str = "popover";

/// Matches `.popover` in docs/mockup.html. The height is a first-paint estimate
/// only — the card measures itself and resizes the window to fit its content.
const WIDTH: f64 = 300.0;
const HEIGHT: f64 = 360.0;
/// Breathing room between the menu bar and the popover's top edge.
const GAP: f64 = 6.0;
/// Fallback drop below the menu bar when there is no icon rectangle yet.
const MENU_BAR_H: f64 = 32.0;

/// The tray icon's rectangle in **physical** pixels, remembered from the last
/// click so that routes without one — the global shortcut, a relaunch — still
/// open under the icon rather than in a corner. Stored raw: the scale factor
/// that converts it belongs to the monitor the icon is on, which is only known
/// once that monitor has been resolved.
static ANCHOR: parking_lot::Mutex<Option<(PhysicalPosition<f64>, PhysicalSize<f64>)>> =
    parking_lot::Mutex::new(None);

/// When the popover last closed. Clicking the tray icon while the popover is
/// open blurs it — closing it — *before* the click event arrives, so without
/// this the click would immediately reopen what the user just dismissed.
static LAST_HIDDEN: parking_lot::Mutex<Option<Instant>> = parking_lot::Mutex::new(None);
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// Record the tray icon's rectangle in physical pixels.
pub fn remember_anchor(pos: PhysicalPosition<f64>, size: PhysicalSize<f64>) {
    *ANCHOR.lock() = Some((pos, size));
}

pub fn toggle(app: &AppHandle) {
    match app.get_webview_window(LABEL) {
        Some(w) if w.is_visible().unwrap_or(false) => hide(app),
        _ if just_hidden() => {}
        _ => show(app),
    }
}

fn just_hidden() -> bool {
    LAST_HIDDEN
        .lock()
        .is_some_and(|t| t.elapsed() < REOPEN_GUARD)
}

pub fn show(app: &AppHandle) {
    if let Err(e) = open(app) {
        eprintln!("[timebox] could not open the popover: {e}");
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
        *LAST_HIDDEN.lock() = Some(Instant::now());
    }
}

fn open(app: &AppHandle) -> tauri::Result<()> {
    let window = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => {
            let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
                .title("TimeBox")
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .minimizable(false)
                .maximizable(false)
                .shadow(true)
                .inner_size(WIDTH, HEIGHT)
                .visible(false)
                .build()?;

            // The card has rounded corners, so the window behind them must not
            // paint square ones. Done through the content view's layer rather
            // than `.transparent(true)`, which needs `macos-private-api` and
            // would rule out the Mac App Store.
            #[cfg(target_os = "macos")]
            super::window_corners::round(&w);

            // Click-outside dismissal. Without it the popover would linger over
            // whatever the user switched to, which is not what a menu bar item
            // is for.
            let this = w.clone();
            w.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    let _ = this.hide();
                    *LAST_HIDDEN.lock() = Some(Instant::now());
                }
            });
            w
        }
    };

    position(app, &window)?;
    window.show()?;
    window.set_always_on_top(true)?;
    window.set_focus()?;
    Ok(())
}

/// Anchor under the tray icon.
///
/// The monitor is resolved from the icon's own coordinates rather than from the
/// window (which reports none until it has been shown at least once) or from the
/// primary monitor (which is the wrong one on a second display). A known anchor
/// is never discarded: if the monitor cannot be resolved the popover still opens
/// under the icon, just without edge clamping.
fn position(app: &AppHandle, window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let anchor = *ANCHOR.lock();

    let monitor = match anchor {
        Some((pos, size)) => app
            .monitor_from_point(pos.x + size.width / 2.0, pos.y)?
            .or(app.primary_monitor()?),
        None => window.current_monitor()?.or(app.primary_monitor()?),
    };
    let scale = monitor.as_ref().map_or(1.0, |m| m.scale_factor());

    let (x, y) = match anchor {
        Some((pos, size)) => (
            (pos.x + size.width / 2.0) / scale - WIDTH / 2.0,
            (pos.y + size.height) / scale + GAP,
        ),
        // Never clicked, no icon rectangle: the menu bar's right end, where the
        // icon lives.
        None => match monitor.as_ref() {
            Some(m) => {
                let origin = m.position().to_logical::<f64>(scale);
                let size = m.size().to_logical::<f64>(scale);
                (origin.x + size.width - WIDTH - GAP, origin.y + MENU_BAR_H)
            }
            None => (GAP, GAP),
        },
    };

    // Keep it on screen: a tray icon near the right edge would otherwise push
    // the popover half out of view.
    let x = match monitor.as_ref() {
        Some(m) => {
            let origin = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            let max = origin.x + size.width - WIDTH - GAP;
            x.clamp(origin.x + GAP, max.max(origin.x + GAP))
        }
        None => x.max(GAP),
    };

    window.set_position(LogicalPosition::new(x, y))?;
    Ok(())
}
