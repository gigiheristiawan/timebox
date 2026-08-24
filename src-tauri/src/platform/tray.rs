//! The menu bar item (SPEC §7.1).
//!
//! What the title *says* is decided in `core::menubar`; this module only owns
//! the macOS mechanics: a template image, a click that toggles the popover, and
//! a title push that is skipped when nothing changed.

use crate::core::menubar;
use crate::core::model::TimerState;
use crate::core::timer_machine::MachineState;
use crate::platform::popover;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const ID: &str = "timebox";
/// The only menu item. A break has to be startable without opening the popover
/// first — the moment you decide to stop is the moment you want it recorded
/// (IDLE_TIME D22). It is one item rather than two because it is one subject:
/// the label follows the state, the way the popover's break control does.
const BREAK_ITEM: &str = "break-item";

/// Last pushed title. The tick fires at 1 Hz and every dispatch also refreshes,
/// so this is what keeps the actual menu bar writes down to the once-a-second
/// the spec allows (task 6.3).
static LAST_TITLE: Mutex<String> = Mutex::new(String::new());

/// `menuBarShowTimer`. Read from settings at startup and flipped by the
/// settings window through `set_show_timer`.
static SHOW_TIMER: AtomicBool = AtomicBool::new(true);

/// The break item, kept so its label can follow the state. Mutating it is
/// marshalled to the main thread by Tauri, and runs inline when already there,
/// so `refresh` is safe from the tick thread as well as from a command.
static BREAK_ITEM_HANDLE: Mutex<Option<MenuItem<tauri::Wry>>> = Mutex::new(None);

/// Last pushed label and enabled flag, so the menu is only rewritten when it
/// would actually read differently — `refresh` runs once a second.
static LAST_BREAK_ITEM: Mutex<(&'static str, bool)> = Mutex::new(("", true));

pub fn set_show_timer(on: bool) {
    SHOW_TIMER.store(on, Ordering::Relaxed);
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let item = MenuItem::with_id(app, BREAK_ITEM, "Take a break", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&item])?;
    *BREAK_ITEM_HANDLE.lock() = Some(item);

    TrayIconBuilder::with_id(ID)
        .icon(tray_icon())
        .menu(&menu)
        // Left click is the popover, as it has always been; the menu is the
        // right-click surface. Without this the menu would swallow the click
        // that opens the popover.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id() == BREAK_ITEM {
                toggle_break(app);
            }
        })
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

/// What the break item should say, and whether it can be used.
///
/// During a break the operation is to end it, so the item says so — an item
/// reading *Take a break* while a break runs is a control that does nothing,
/// and the menu is the one surface with no room to explain itself. At a work
/// checkpoint it is shown **disabled** rather than hidden: the checkpoint has
/// no side doors, and a menu that changes shape under the cursor is worse than
/// one that greys an item out.
fn break_item_for(state: &MachineState) -> (&'static str, bool) {
    if state.on_break() {
        ("End break", true)
    } else {
        ("Take a break", state.timer_state != TimerState::AwaitingDecision)
    }
}

/// Push the break item's label for `state`. A no-op when it would not change.
fn refresh_menu(state: &MachineState) {
    let want = break_item_for(state);
    {
        let mut last = LAST_BREAK_ITEM.lock();
        if *last == want {
            return;
        }
        *last = want;
    }
    // Cloned out of the lock: setting the text crosses to the main thread, and
    // holding either lock across that is how a deadlock gets written.
    let item = BREAK_ITEM_HANDLE.lock().clone();
    if let Some(item) = item {
        let _ = item.set_text(want.0);
        let _ = item.set_enabled(want.1);
    }
}

/// Push the title for `state`. A no-op when the text has not changed.
pub fn refresh(app: &AppHandle, state: &MachineState, now: crate::core::model::Millis) {
    refresh_menu(state);
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

/// Start a break of the user's default length, or end the one running — the
/// same two operations the item's label offers.
///
/// The reducer decides whether either is allowed; both are no-ops from the
/// wrong state, so the menu never has to predict the answer, and a click that
/// races a checkpoint cannot slip through.
fn toggle_break(app: &AppHandle) {
    use crate::core::timer_machine::Event;
    let Some(state) = app.try_state::<std::sync::Arc<crate::state::App>>() else { return };
    let now = crate::state::now_ms();
    let event = if state.snapshot().on_break() {
        Event::EndBreak
    } else {
        Event::StartBreak { ms: state.settings().default_break_duration_ms }
    };
    match state.dispatch(event, now) {
        Ok(fx) => {
            crate::platform::checkpoint::apply(app, &fx, &state.settings());
            refresh(app, &state.snapshot(), now);
            let _ = tauri::Emitter::emit(app, "timebox://changed", ());
        }
        Err(e) => eprintln!("[timebox] break action failed: {e}"),
    };
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
