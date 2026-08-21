//! Launch at login, via `SMAppService`.
//!
//! The obvious implementation — writing a plist into `~/Library/LaunchAgents`,
//! which is what `tauri-plugin-autostart` does — is illegal inside the App
//! Sandbox: that path is outside the container, so the write is denied and the
//! toggle silently does nothing. `SMAppService` (macOS 13+, matching
//! `minimumSystemVersion`) is the public, sandbox-legal replacement: the system
//! registers the *bundle itself* as a login item, no file written by us.
//!
//! Raw `msg_send!` rather than typed bindings because `SMAppService` has no
//! crate in the tree and this is four selectors.

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;

/// `SMAppServiceStatusEnabled`.
const STATUS_ENABLED: isize = 1;

/// `+[SMAppService mainApp]` — the service representing this bundle.
///
/// Returns `None` when there is no bundle to register. `mainApp` raises an
/// Objective-C exception in that case, which Rust cannot catch — it aborts the
/// process — and an unbundled binary is exactly what `tauri dev` and
/// `cargo test` run. The guard is what keeps toggling the setting in a dev
/// build from killing the app.
fn main_app() -> Option<Retained<AnyObject>> {
    unsafe {
        if !is_bundled() {
            return None;
        }
        let cls = objc2::runtime::AnyClass::get(c"SMAppService")?;
        let obj: Option<Retained<AnyObject>> = msg_send![cls, mainApp];
        obj
    }
}

/// Whether this process is running from a real `.app` — i.e. whether the main
/// bundle has an identifier at all.
fn is_bundled() -> bool {
    unsafe {
        let Some(cls) = objc2::runtime::AnyClass::get(c"NSBundle") else {
            return false;
        };
        let bundle: *mut AnyObject = msg_send![cls, mainBundle];
        if bundle.is_null() {
            return false;
        }
        let ident: *mut AnyObject = msg_send![bundle, bundleIdentifier];
        !ident.is_null()
    }
}

/// Register or unregister the login item.
///
/// Fails in development and from `target/`: the system only accepts a service
/// whose bundle is signed and installed (`/Applications`). That is why the
/// caller treats a failure as non-fatal — the stored preference is the truth,
/// and this is the attempt to make the system agree with it.
pub fn set(enabled: bool) -> Result<(), String> {
    let service = main_app().ok_or_else(|| "not running from an installed .app".to_string())?;
    unsafe {
        let mut err: *mut AnyObject = std::ptr::null_mut();
        let ok: bool = if enabled {
            msg_send![&*service, registerAndReturnError: &mut err]
        } else {
            msg_send![&*service, unregisterAndReturnError: &mut err]
        };
        if ok {
            Ok(())
        } else if err.is_null() {
            Err("unknown SMAppService failure".into())
        } else {
            let desc: *mut AnyObject = msg_send![err, localizedDescription];
            let utf8: *const std::ffi::c_char = msg_send![desc, UTF8String];
            Err(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
        }
    }
}

/// Whether the system currently has the login item enabled.
///
/// The stored setting is what the UI shows; this exists so a caller can see
/// that the system disagrees (the user can turn the item off in System
/// Settings, which never reaches us).
#[allow(dead_code)]
pub fn is_enabled() -> bool {
    match main_app() {
        Some(service) => {
            let status: isize = unsafe { msg_send![&*service, status] };
            status == STATUS_ENABLED
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    /// The guard, not the registration, is what is testable here: the test
    /// binary is not a bundle, and reaching `SMAppService` from one aborts the
    /// process rather than returning an error.
    #[test]
    fn unbundled_process_is_refused_rather_than_aborting() {
        assert!(!super::is_bundled());
        assert!(super::set(true).is_err());
        assert!(!super::is_enabled());
    }
}
