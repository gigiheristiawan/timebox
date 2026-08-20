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

/// `menuBarShowTimer`. Read from settings at startup and flipped by the
/// settings window through `set_show_timer`.
static SHOW_TIMER: AtomicBool = AtomicBool::new(true);

pub fn set_show_timer(on: bool) {
    SHOW_TIMER.store(on, Ordering::Relaxed);
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(ID)
        .icon(tray_icon())
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

/// Push the title even if the text is unchanged. Turning `menuBarShowTimer`
/// off produces an empty title, which the cached value would otherwise suppress
/// on the way back — the tray would keep the last countdown it was given.
pub fn refresh_forced(app: &AppHandle, state: &MachineState, now: crate::core::model::Millis) {
    LAST_TITLE.lock().clear();
    refresh(app, state, now);
}

/// The menu bar mark, exported artwork rather than drawn here — see
/// `docs/RELEASE.md` §1. 36×36 (18pt at 2×) with the shape inset to 20×20, so
/// it has the breathing room a menu bar glyph needs.
///
/// As a template image only the alpha channel survives; macOS repaints the
/// shape for the active theme. That is why the asset is pure black with its
/// cutouts as real transparency: any white would become an opaque blob.
fn tray_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../../icons/tray.png"))
        .expect("tray.png is compiled in and must decode")
}
