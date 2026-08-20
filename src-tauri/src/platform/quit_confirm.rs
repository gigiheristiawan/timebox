//! The quit confirmation (SPEC D14).
//!
//! It prevents nothing. Quitting with a block running is allowed — the window
//! exists so the cost is visible, the same treatment extending and switching
//! get. The content is a React surface like every other window; this module
//! only owns the macOS mechanics.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "quit-confirm";

const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 190.0;

pub fn show(app: &AppHandle) {
    if let Err(e) = open(app) {
        // A confirm that cannot be shown must not trap the user in a running
        // app: quitting is what they asked for.
        eprintln!("[timebox] quit confirm unavailable, quitting directly: {e}");
        app.exit(0);
    }
}

fn open(app: &AppHandle) -> tauri::Result<()> {
    crate::platform::popover::hide(app);

    if let Some(w) = app.get_webview_window(LABEL) {
        w.show()?;
        w.set_focus()?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Quit TimeBox")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .always_on_top(true)
        .center()
        .build()?;

    // Closing the window is Cancel — the safe reading of a dismissed confirm.
    window.set_focus()?;
    Ok(())
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}
