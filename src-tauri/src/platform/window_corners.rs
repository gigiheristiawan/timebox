//! Rounded corners for the popover, without private API.
//!
//! The popover card is rounded, so the window behind it must not paint square
//! corners. Tauri's `transparent(true)` would do it, but only with the
//! `macos-private-api` feature — which makes the webview's background
//! transparent through a private `WKWebView` key and is grounds for App Store
//! rejection.
//!
//! Everything used here is public: an `NSWindow` may be non-opaque with a clear
//! background colour, and clipping the content view's layer to a corner radius
//! rounds the opaque webview inside it. The visible result is the same rounded
//! card; only the mechanism differs.

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use tauri::WebviewWindow;

/// Matches the `border-radius` of `.popover` in docs/mockup.html.
const RADIUS: f64 = 12.0;

/// Clip `window` to a rounded rectangle. Best-effort: a failure here costs
/// square corners, never a working popover.
pub fn round(window: &WebviewWindow) {
    let Ok(ptr) = window.ns_window() else {
        eprintln!("[timebox] no NSWindow to round");
        return;
    };
    unsafe {
        let ns_window = ptr as *mut AnyObject;

        // Without both of these the system paints an opaque square window
        // background outside the clipped layer.
        let _: () = msg_send![ns_window, setOpaque: false];
        let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![ns_window, setBackgroundColor: clear];

        let content: *mut AnyObject = msg_send![ns_window, contentView];
        if content.is_null() {
            return;
        }
        let _: () = msg_send![content, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![content, layer];
        if layer.is_null() {
            return;
        }
        let _: () = msg_send![layer, setCornerRadius: RADIUS];
        let _: () = msg_send![layer, setMasksToBounds: true];

        // The drop shadow is computed from the window's opaque shape, which we
        // just changed.
        let _: () = msg_send![ns_window, invalidateShadow];
    }
}
