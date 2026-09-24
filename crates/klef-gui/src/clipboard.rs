//! Secret copy, done in Rust so the plaintext never reaches the webview.
//!
//! The value is written with the nspasteboard.org markers that clipboard
//! managers (Maccy, Raycast, Paste, Alfred…) honor: `Concealed` (don't show
//! it) and `Transient` (don't record it in history at all). Without them a
//! copied key lives forever in the manager's database, auto-clear or not.
//!
//! Auto-clear keys off `changeCount`, not content: if anything was copied
//! after us the count moved and we leave the clipboard alone. That also means
//! we never need to read the clipboard back.

use objc2_app_kit::NSPasteboard;
use objc2_foundation::NSString;
use std::sync::Mutex;
use std::time::Duration;

const PLAIN_TEXT: &str = "public.utf8-plain-text";
const MARKERS: [&str; 2] = [
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
];

/// `changeCount` right after our last copy with auto-clear pending, so quitting
/// inside the window can still clear it (see [`clear_pending`]).
static PENDING: Mutex<Option<isize>> = Mutex::new(None);

/// Copy `value`, then clear it after `clear_after` unless something else was
/// copied in the meantime. `None` disables the clear.
pub fn copy_secret(value: &str, clear_after: Option<Duration>) {
    let ours = write(&NSPasteboard::generalPasteboard(), value);
    let Some(delay) = clear_after else {
        *lock() = None;
        return;
    };
    *lock() = Some(ours);
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        clear_if_unchanged(ours);
    });
}

/// Clear a still-pending copy now. Called on app exit.
pub fn clear_pending() {
    let pending = *lock();
    if let Some(ours) = pending {
        clear_if_unchanged(ours);
    }
}

/// Write `value` plus the privacy markers; returns the resulting `changeCount`.
fn write(pb: &NSPasteboard, value: &str) -> isize {
    pb.clearContents();
    pb.setString_forType(&NSString::from_str(value), &NSString::from_str(PLAIN_TEXT));
    for marker in MARKERS {
        pb.setString_forType(&NSString::from_str(""), &NSString::from_str(marker));
    }
    pb.changeCount()
}

/// Clear `pb` only if nothing was written since `ours`.
fn clear_on(pb: &NSPasteboard, ours: isize) {
    if pb.changeCount() == ours {
        pb.clearContents();
    }
}

fn clear_if_unchanged(ours: isize) {
    clear_on(&NSPasteboard::generalPasteboard(), ours);
    let mut pending = lock();
    if *pending == Some(ours) {
        *pending = None;
    }
}

fn lock() -> std::sync::MutexGuard<'static, Option<isize>> {
    PENDING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Run against a private, uniquely named pasteboard — never the user's clipboard.
#[cfg(test)]
mod tests {
    use super::*;

    fn read(pb: &NSPasteboard, ty: &str) -> Option<String> {
        pb.stringForType(&NSString::from_str(ty))
            .map(|s| s.to_string())
    }

    #[test]
    fn copy_sets_value_and_privacy_markers() {
        let pb = NSPasteboard::pasteboardWithUniqueName();
        write(&pb, "sk_live_x");
        assert_eq!(read(&pb, PLAIN_TEXT).as_deref(), Some("sk_live_x"));
        for marker in MARKERS {
            assert!(read(&pb, marker).is_some(), "missing {marker}");
        }
    }

    #[test]
    fn clear_wipes_our_copy() {
        let pb = NSPasteboard::pasteboardWithUniqueName();
        let ours = write(&pb, "sk_live_x");
        clear_on(&pb, ours);
        assert_eq!(read(&pb, PLAIN_TEXT), None);
    }

    #[test]
    fn clear_leaves_a_later_copy_alone() {
        let pb = NSPasteboard::pasteboardWithUniqueName();
        let ours = write(&pb, "sk_live_x");
        pb.clearContents();
        pb.setString_forType(
            &NSString::from_str("user text"),
            &NSString::from_str(PLAIN_TEXT),
        );
        clear_on(&pb, ours);
        assert_eq!(read(&pb, PLAIN_TEXT).as_deref(), Some("user text"));
    }
}
