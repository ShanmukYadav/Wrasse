//! Spotlight and LaunchServices lookups (read-only, in-process: no programs started).
//! - when an app was last opened and when it was added (`kMDItemLastUsedDate`, `kMDItemDateAdded`);
//! - how many apps with a bundle id LaunchServices knows anywhere
//!   (`LSCopyApplicationURLsForBundleIdentifier`), to tell whether a data folder's app is gone.

use std::ffi::c_void;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;

const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8
/// Seconds between the Unix epoch and the Core Foundation epoch (2001-01-01).
const CF_EPOCH: f64 = 978_307_200.0;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        len: isize,
        encoding: u32,
        external: u8,
    ) -> CFStringRef;
    fn CFRelease(cf: CFTypeRef);
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFDateGetTypeID() -> usize;
    fn CFDateGetAbsoluteTime(date: CFTypeRef) -> f64;
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn MDItemCreate(alloc: CFTypeRef, path: CFStringRef) -> CFTypeRef;
    fn MDItemCopyAttribute(item: CFTypeRef, name: CFStringRef) -> CFTypeRef;
    static kMDItemLastUsedDate: CFStringRef;
    static kMDItemDateAdded: CFStringRef;
    fn LSCopyApplicationURLsForBundleIdentifier(
        id: CFStringRef,
        error: *mut CFTypeRef,
    ) -> CFTypeRef;
}

/// An owned Core Foundation object, released on drop.
struct Owned(CFTypeRef);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: we own one reference (from a Create/Copy call).
            unsafe { CFRelease(self.0) }
        }
    }
}

fn cf_string(bytes: &[u8]) -> Option<Owned> {
    // SAFETY: bytes is a valid slice; CF copies it.
    let s = unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            UTF8,
            0,
        )
    };
    (!s.is_null()).then_some(Owned(s))
}

fn date_attribute(path: &Path, name: CFStringRef) -> Option<i64> {
    let p = cf_string(path.as_os_str().as_bytes())?;
    // SAFETY: p is a valid CFString; MDItemCreate returns an owned item or null.
    let item = Owned(unsafe { MDItemCreate(std::ptr::null(), p.0) });
    if item.0.is_null() {
        return None;
    }
    // SAFETY: item and name are valid; the copy is owned.
    let value = Owned(unsafe { MDItemCopyAttribute(item.0, name) });
    if value.0.is_null() {
        return None;
    }
    // SAFETY: value is a valid CF object; we check it's a CFDate before reading it.
    unsafe {
        if CFGetTypeID(value.0) != CFDateGetTypeID() {
            return None;
        }
        Some((CFDateGetAbsoluteTime(value.0) + CF_EPOCH) as i64)
    }
}

/// When the app was last opened (Unix seconds), if Spotlight knows.
pub fn last_used(app: &Path) -> Option<i64> {
    // SAFETY: reading an immutable framework constant.
    date_attribute(app, unsafe { kMDItemLastUsedDate })
}

/// When the item was added to the Mac (Unix seconds), if Spotlight knows.
pub(crate) fn date_added(app: &Path) -> Option<i64> {
    // SAFETY: reading an immutable framework constant.
    date_attribute(app, unsafe { kMDItemDateAdded })
}

/// How many apps with this bundle id LaunchServices has registered, anywhere.
pub(crate) fn apps_with_bundle_id(id: &str) -> usize {
    let Some(s) = cf_string(id.as_bytes()) else {
        return 0;
    };
    // SAFETY: s is a valid CFString; the result is an owned CFArray or null.
    let urls =
        Owned(unsafe { LSCopyApplicationURLsForBundleIdentifier(s.0, std::ptr::null_mut()) });
    if urls.0.is_null() {
        return 0;
    }
    // SAFETY: urls is a valid CFArray.
    unsafe { CFArrayGetCount(urls.0).max(0) as usize }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finder_is_registered_and_dates_read_without_crashing() {
        assert!(apps_with_bundle_id("com.apple.finder") >= 1);
        assert_eq!(apps_with_bundle_id("com.wrasse-test.no-such-app"), 0);
        // Spotlight may be off; these only must not crash and, if set, be sane.
        if let Some(t) = date_added(Path::new("/System/Applications/Calculator.app")) {
            assert!(t > 946_684_800, "after 2000");
        }
        let _ = last_used(Path::new("/nonexistent-wrasse"));
    }
}
