//! The menu bar item (SPEC §7.1).
//!
//! What the title *says* is decided in `core::menubar`; this module only owns
//! the macOS mechanics: a template image, a click that toggles the popover, and
//! a title push that is skipped when nothing changed.

use crate::core::menubar;
use crate::core::timer_machine::MachineState;
use crate::platform::popover;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::image::Image;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

const ID: &str = "timebox";

/// Last pushed title. The tick fires at 1 Hz and every dispatch also refreshes,
/// so this is what keeps the actual menu bar writes down to the once-a-second
/// the spec allows (task 6.3).
static LAST_TITLE: Mutex<String> = Mutex::new(String::new());

/// `menuBarShowTimer`. Read from settings at startup; the settings UI lands in
/// Phase 7 and will flip it through `set_show_timer`.
static SHOW_TIMER: AtomicBool = AtomicBool::new(true);

pub fn set_show_timer(on: bool) {
    SHOW_TIMER.store(on, Ordering::Relaxed);
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(ID)
        .icon(ring_icon())
        // A template image is monochrome + alpha; macOS recolors it for the
        // active menu bar. Without this the icon is invisible in one theme.
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            // Act on the release only; a press and a release both arrive.
            if let TrayIconEvent::Click { rect, button_state: MouseButtonState::Up, .. } = event {
                // The rectangle is already physical and top-left origin; the
                // popover converts it against the monitor it resolves.
                popover::remember_anchor(rect.position.to_physical(1.0), rect.size.to_physical(1.0));
                popover::toggle(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Push the title for `state`. A no-op when the text has not changed.
pub fn refresh(app: &AppHandle, state: &MachineState, now: crate::core::model::Millis) {
    let title = menubar::title(state, now, SHOW_TIMER.load(Ordering::Relaxed));
    {
        let mut last = LAST_TITLE.lock();
        if *last == title {
            return;
        }
        *last = title.clone();
    }
    if let Some(tray) = app.tray_by_id(ID) {
        let _ = tray.set_title(if title.is_empty() { None } else { Some(title) });
    }
}

/// A ring — the `◉` of the title, drawn rather than shipped as an asset so the
/// placeholder cannot drift out of sync with the real icon work in task 8.1.
/// Black with an anti-aliased alpha edge; as a template image only the alpha is
/// used, so the colour is irrelevant.
fn ring_icon() -> Image<'static> {
    const SIDE: u32 = 36; // 18pt at 2×, the usual menu bar glyph size.
    const OUTER: f64 = 8.0;
    const INNER: f64 = 4.6;
    let center = (SIDE as f64 - 1.0) / 2.0;

    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (dx, dy) = (x as f64 - center, y as f64 - center);
            let d = (dx * dx + dy * dy).sqrt();
            let alpha = ((OUTER - d).clamp(0.0, 1.0) * (d - INNER).clamp(0.0, 1.0) * 255.0) as u8;
            rgba.extend_from_slice(&[0, 0, 0, alpha]);
        }
    }
    Image::new_owned(rgba, SIDE, SIDE)
}
