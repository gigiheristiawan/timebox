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

use objc2::{msg_send, sel};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use std::sync::atomic::{AtomicBool, Ordering};

/// `SMAppServiceStatusEnabled`.
const STATUS_ENABLED: isize = 1;

/// What the system last told us, cached because the snapshot is rebuilt every
/// second while a block runs and `status` crosses an XPC boundary to the
/// background-task daemon. Written only by `reconcile`, which is the only place
/// the answer can change.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// `+[SMAppService mainAppService]` — the service representing this bundle.
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

        // `mainApp` is the *Swift* name; the selector is `mainAppService`.
        // Sending one the class does not implement raises, and an ObjC
        // exception cannot be caught here — it aborts the process. So ask
        // first: this module reaches AppKit through raw `msg_send`, where a
        // wrong selector is a crash rather than a compile error.
        let responds: bool = msg_send![cls, respondsToSelector: sel!(mainAppService)];
        if !responds {
            eprintln!("[timebox] SMAppService does not respond to mainAppService");
            return None;
        }
        let obj: Option<Retained<AnyObject>> = msg_send![cls, mainAppService];
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

/// Whether the system currently has the login item enabled — a live query.
///
/// The user can turn the item off in System Settings, which never reaches us,
/// so the stored preference is a wish and this is the fact.
pub fn is_enabled() -> bool {
    match main_app() {
        Some(service) => {
            let status: isize = unsafe { msg_send![&*service, status] };
            status == STATUS_ENABLED
        }
        None => false,
    }
}

/// The cached answer to `is_enabled`, for callers on a hot path.
pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Make the system agree with the stored preference, and record what it
/// actually says afterwards.
///
/// Called on every launch, not only when the setting changes. `SMAppService`
/// refuses an app that is not installed in `/Applications`, so a user who
/// enables the toggle from `~/Downloads` and *later* moves the app would
/// otherwise never get registered — the preference did not change, so nothing
/// would retry. Reconciling at startup is what closes that gap.
pub fn reconcile(desired: bool) {
    if is_enabled() != desired {
        if let Err(e) = set(desired) {
            eprintln!(
                "[timebox] launch at login could not be {}: {e}",
                if desired { "enabled" } else { "disabled" }
            );
        }
    }
    // Re-read rather than assume: a failed `set` must leave the cache showing
    // what is true, or the UI would report a login item that does not exist.
    ACTIVE.store(is_enabled(), Ordering::Relaxed);
}

// ------------------------------------------- migration away from the plist

/// `tauri-plugin-autostart` labelled its job with the product name, so the file
/// it wrote is `~/Library/LaunchAgents/TimeBox.plist`.
const LEGACY_LAUNCH_AGENT: &str = "Library/LaunchAgents/TimeBox.plist";

/// Delete the launch agent that 0.1.0 installed, if it is ours.
///
/// Every user who enabled launch at login before this build has that file, and
/// nothing else will ever remove it: the new code registers through
/// `SMAppService` and has no idea the plist exists. Left in place it launches
/// the app at login *independently* of the setting — so turning the toggle off
/// would unregister the service, leave the plist running, and report "off"
/// while the app still starts itself. That is worse than the bug this module
/// was written to fix.
///
/// Removing the file is enough. `RunAtLoad` fires when launchd loads the job at
/// login; with the file gone there is nothing to load next time, and the
/// already-loaded job does not relaunch anything on its own.
///
/// Silently does nothing in a sandboxed build — `~/Library/LaunchAgents` is
/// outside the container. That is correct: a Mac App Store install is a fresh
/// install and never had the plist.
pub fn remove_legacy_launch_agent() {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let path = std::path::Path::new(&home).join(LEGACY_LAUNCH_AGENT);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return; // absent, unreadable, or sandboxed away — all the same to us
    };
    if !is_ours(&contents) {
        eprintln!("[timebox] left {} alone: not ours", path.display());
        return;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => eprintln!("[timebox] removed the 0.1.0 launch agent; login at start is now SMAppService's job"),
        Err(e) => eprintln!("[timebox] could not remove {}: {e}", path.display()),
    }
}

/// Whether a launch agent found at that path was written for *this* app.
///
/// The filename alone is not enough to justify deleting a file out of the
/// user's `LaunchAgents` directory — anything could be called `TimeBox.plist`.
/// The program it runs is the evidence.
fn is_ours(contents: &str) -> bool {
    contents.contains("TimeBox.app/Contents/MacOS/timebox")
}

#[cfg(test)]
mod tests {
    /// The selectors this module sends are resolved at runtime, so a wrong
    /// name is not a compile error — it is an unrecognised-selector abort the
    /// moment the app runs from a real bundle. `mainApp` (the Swift name) was
    /// exactly that bug. This is the only check that catches it without a
    /// signed, installed build.
    #[test]
    fn smappservice_exposes_every_selector_we_send() {
        let cls = objc2::runtime::AnyClass::get(c"SMAppService")
            .expect("ServiceManagement is linked and SMAppService exists");
        let responds: bool =
            unsafe { objc2::msg_send![cls, respondsToSelector: objc2::sel!(mainAppService)] };
        assert!(responds, "+[SMAppService mainAppService] is missing");
        for sel in [
            objc2::sel!(registerAndReturnError:),
            objc2::sel!(unregisterAndReturnError:),
            objc2::sel!(status),
        ] {
            assert!(cls.instance_method(sel).is_some(), "-[SMAppService {sel:?}] is missing");
        }
    }

    /// The guard, not the registration, is what is testable here: the test
    /// binary is not a bundle, and reaching `SMAppService` from one aborts the
    /// process rather than returning an error.
    #[test]
    fn unbundled_process_is_refused_rather_than_aborting() {
        assert!(!super::is_bundled());
        assert!(super::set(true).is_err());
        assert!(!super::is_enabled());
    }

    /// The point of the cache is that the UI can trust it. A registration the
    /// system refused must not leave it claiming success, or the settings
    /// toggle would show a login item that will never run.
    #[test]
    fn a_refused_registration_is_not_cached_as_active() {
        super::reconcile(true);
        assert!(!super::is_active());
    }

    /// Exactly what 0.1.0's `tauri-plugin-autostart` wrote.
    const LEGACY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>Label</key><string>TimeBox</string>
  <key>ProgramArguments</key>
  <array><string>/Applications/TimeBox.app/Contents/MacOS/timebox</string></array>
  <key>RunAtLoad</key><true/>
</dict></plist>"#;

    #[test]
    fn the_0_1_0_launch_agent_is_recognised() {
        assert!(super::is_ours(LEGACY));
    }

    /// The filename is not evidence. Deleting a file out of the user's
    /// LaunchAgents directory because it happens to be called TimeBox.plist
    /// would be destroying someone else's job on a name collision.
    #[test]
    fn a_foreign_launch_agent_of_the_same_name_is_left_alone() {
        let foreign = LEGACY.replace(
            "/Applications/TimeBox.app/Contents/MacOS/timebox",
            "/usr/local/bin/some-other-timebox",
        );
        assert!(!super::is_ours(&foreign));
        assert!(!super::is_ours(""));
    }
}
