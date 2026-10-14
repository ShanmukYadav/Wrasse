//! AppKit through the Objective-C runtime: asking an app to quit (never forcing it), reading the
//! volume's capacities, revealing a file in Finder, and Wrasse's own login item. No other
//! Objective-C calls.

use super::*;

/// Asks an app to quit the way the Dock's Quit does (it may ask to save documents).
///
/// Uses `NSRunningApplication::terminate`, which sends `kAEQuitApplication` and returns once
/// that request is sent. It does not call `forceTerminate` and does not signal the process.
/// A call that hasn't returned after 10 s is reported as "couldn't ask it to quit"; the app is
/// left running. Already-quit apps succeed without being signalled.
pub fn quit_app(bundle_id: &str) -> io::Result<()> {
    if !valid_quit_bundle_id(bundle_id) {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let id = bundle_id.to_owned();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("wrasse-quit".into())
        .spawn(move || {
            let _ = tx.send(request_quit(&id));
        })
        .map_err(|e| io::Error::other(e.to_string()))?;
    match rx.recv_timeout(QUIT_ASK_TIMEOUT) {
        Ok(result) => result,
        Err(_) => Err(io::Error::other(COULDNT_ASK_QUIT)),
    }
}

pub(super) fn valid_quit_bundle_id(bundle_id: &str) -> bool {
    if bundle_id.is_empty()
        || (!crate::apps::is_bundle_id(bundle_id) && bundle_id.split('.').count() < 2)
    {
        return false;
    }
    bundle_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

pub(super) fn request_quit(bundle_id: &str) -> io::Result<()> {
    let id = CString::new(bundle_id).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: `id` lives until the pool is popped. Only `terminate` is sent.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = request_quit_in_pool(id.as_ptr());
        objc_autoreleasePoolPop(pool);
        result
    }
}

/// `NSRunningApplication` polite quit. `forceTerminate` is intentionally not bound here.
///
/// # Safety
/// `bundle_id` is a valid C string. The caller holds an autorelease pool.
pub(super) unsafe fn request_quit_in_pool(bundle_id: *const c_char) -> io::Result<()> {
    let class = objc_getClass(c"NSRunningApplication".as_ptr());
    let nsstring = objc_getClass(c"NSString".as_ptr());
    if class.is_null() || nsstring.is_null() {
        return Err(io::Error::other(COULDNT_ASK_QUIT));
    }
    let id_str = msg_cstr(nsstring, sel(c"stringWithUTF8String:"), bundle_id);
    if id_str.is_null() {
        return Err(io::Error::other(COULDNT_ASK_QUIT));
    }
    let apps = msg_id(
        class,
        sel(c"runningApplicationsWithBundleIdentifier:"),
        id_str,
    );
    if apps.is_null() {
        return Err(io::Error::other(COULDNT_ASK_QUIT));
    }
    let n = msg_usize(apps, sel(c"count"));
    if n == 0 {
        return Ok(());
    }
    let mut asked = false;
    for i in 0..n {
        let app = msg_index(apps, sel(c"objectAtIndex:"), i);
        if app.is_null() || msg_bool(app, sel(c"isTerminated")) {
            continue;
        }
        // Polite quit. Returns once the request is sent; the app may still show a save dialog.
        if msg_bool(app, sel(c"terminate")) {
            asked = true;
        }
    }
    if asked {
        Ok(())
    } else {
        Err(io::Error::other(COULDNT_ASK_QUIT))
    }
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "ServiceManagement", kind = "framework")]
extern "C" {}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
    pub(super) fn objc_autoreleasePoolPush() -> *mut c_void;
    pub(super) fn objc_autoreleasePoolPop(pool: *mut c_void);
}

pub(super) fn sel(name: &CStr) -> *mut c_void {
    // SAFETY: `name` is a static C string.
    unsafe { sel_registerName(name.as_ptr()) }
}

/// `objc_msgSend` is one C symbol with several signatures. The cast matches the call.
pub(super) unsafe fn msg_id(recv: *mut c_void, sel: *mut c_void, arg: *mut c_void) -> *mut c_void {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, arg)
}

pub(super) unsafe fn msg_cstr(
    recv: *mut c_void,
    sel: *mut c_void,
    arg: *const c_char,
) -> *mut c_void {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, arg)
}

pub(super) unsafe fn msg_usize(recv: *mut c_void, sel: *mut c_void) -> usize {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel)
}

pub(super) unsafe fn msg_index(recv: *mut c_void, sel: *mut c_void, index: usize) -> *mut c_void {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, index)
}

/// `BOOL` is a byte on both Apple architectures. Non-zero is true.
pub(super) unsafe fn msg_bool(recv: *mut c_void, sel: *mut c_void) -> bool {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u8;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel) != 0
}

pub(super) unsafe fn msg_u64(recv: *mut c_void, sel: *mut c_void) -> u64 {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel)
}

pub(super) unsafe fn msg_id_err(
    recv: *mut c_void,
    sel: *mut c_void,
    arg: *mut c_void,
    err: *mut *mut c_void,
) -> *mut c_void {
    type F = unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, arg, err)
}

unsafe fn msg_id_id_bool(
    recv: *mut c_void,
    sel: *mut c_void,
    a: *mut c_void,
    b: *mut c_void,
) -> bool {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> u8;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, a, b) != 0
}

unsafe fn msg_err_bool(recv: *mut c_void, sel: *mut c_void, err: *mut *mut c_void) -> bool {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> u8;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, err) != 0
}

unsafe fn ns_string(nsstring: *mut c_void, text: &CStr) -> *mut c_void {
    msg_cstr(nsstring, sel(c"stringWithUTF8String:"), text.as_ptr())
}

/// Shows `path` selected in a Finder window (`NSWorkspace selectFile:inFileViewerRootedAtPath:`).
/// Read-only: Finder opens a window; nothing is moved or opened.
pub fn reveal(path: &Path) -> io::Result<()> {
    let c = CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: called with live C strings inside an autorelease pool.
    let shown = unsafe {
        let pool = objc_autoreleasePoolPush();
        let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
        let nsstring = objc_getClass(c"NSString".as_ptr());
        let shown = if workspace_class.is_null() || nsstring.is_null() {
            false
        } else {
            let workspace = msg0(workspace_class, sel(c"sharedWorkspace"));
            let file = ns_string(nsstring, &c);
            let root = ns_string(nsstring, c"");
            !workspace.is_null()
                && !file.is_null()
                && msg_id_id_bool(
                    workspace,
                    sel(c"selectFile:inFileViewerRootedAtPath:"),
                    file,
                    root,
                )
        };
        objc_autoreleasePoolPop(pool);
        shown
    };
    if shown {
        Ok(())
    } else {
        Err(io::Error::other("Finder couldn't show it"))
    }
}

unsafe fn msg0(recv: *mut c_void, sel: *mut c_void) -> *mut c_void {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel)
}

/// Whether Wrasse opens at login (`SMAppService.mainAppService`, macOS 13+). `None` when the
/// service isn't available (older macOS, or a build that isn't an app bundle).
pub fn login_item_enabled() -> Option<bool> {
    // SAFETY: class lookups and a status read inside an autorelease pool.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let class = objc_getClass(c"SMAppService".as_ptr());
        let out = if class.is_null() {
            None
        } else {
            let service = msg0(class, sel(c"mainAppService"));
            // SMAppServiceStatusEnabled = 1.
            (!service.is_null()).then(|| msg_u64(service, sel(c"status")) == 1)
        };
        objc_autoreleasePoolPop(pool);
        out
    }
}

/// Turns Wrasse's own login item on or off. Only ever called from the Settings switch.
pub fn set_login_item(on: bool) -> io::Result<()> {
    // SAFETY: as above; the error out-pointer is only read as "set or not".
    let ok = unsafe {
        let pool = objc_autoreleasePoolPush();
        let class = objc_getClass(c"SMAppService".as_ptr());
        let ok = if class.is_null() {
            false
        } else {
            let service = msg0(class, sel(c"mainAppService"));
            let mut err: *mut c_void = std::ptr::null_mut();
            let s = if on {
                sel(c"registerAndReturnError:")
            } else {
                sel(c"unregisterAndReturnError:")
            };
            !service.is_null() && msg_err_bool(service, s, &mut err)
        };
        objc_autoreleasePoolPop(pool);
        ok
    };
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(if on {
            "macOS didn't add Wrasse to your login items"
        } else {
            "macOS didn't remove Wrasse from your login items"
        }))
    }
}

/// NSURL volume capacity keys. The Data volume is where the user's files live; `/` is
/// the sealed system snapshot and reports the same free space.
pub(super) unsafe fn volume_capacities_in_pool() -> Option<(u64, u64)> {
    let nsstring = objc_getClass(c"NSString".as_ptr());
    let nsarray = objc_getClass(c"NSArray".as_ptr());
    let nsurl = objc_getClass(c"NSURL".as_ptr());
    if nsstring.is_null() || nsarray.is_null() || nsurl.is_null() {
        return None;
    }
    for mount in [c"/System/Volumes/Data".as_ptr(), c"/".as_ptr()] {
        if let Some(pair) = capacities_of(nsstring, nsarray, nsurl, mount) {
            return Some(pair);
        }
    }
    None
}

pub(super) unsafe fn capacities_of(
    nsstring: *mut c_void,
    nsarray: *mut c_void,
    nsurl: *mut c_void,
    mount: *const c_char,
) -> Option<(u64, u64)> {
    let path = msg_cstr(nsstring, sel(c"stringWithUTF8String:"), mount);
    if path.is_null() {
        return None;
    }
    let url = msg_id(nsurl, sel(c"fileURLWithPath:"), path);
    if url.is_null() {
        return None;
    }
    let k1 = msg_cstr(
        nsstring,
        sel(c"stringWithUTF8String:"),
        c"NSURLVolumeAvailableCapacityKey".as_ptr(),
    );
    let k2 = msg_cstr(
        nsstring,
        sel(c"stringWithUTF8String:"),
        c"NSURLVolumeAvailableCapacityForImportantUsageKey".as_ptr(),
    );
    if k1.is_null() || k2.is_null() {
        return None;
    }
    let arr = msg_id(nsarray, sel(c"arrayWithObject:"), k1);
    if arr.is_null() {
        return None;
    }
    let keys = msg_id(arr, sel(c"arrayByAddingObject:"), k2);
    if keys.is_null() {
        return None;
    }
    let mut err: *mut c_void = std::ptr::null_mut();
    let dict = msg_id_err(url, sel(c"resourceValuesForKeys:error:"), keys, &mut err);
    if dict.is_null() {
        return None;
    }
    let n1 = msg_id(dict, sel(c"objectForKey:"), k1);
    let n2 = msg_id(dict, sel(c"objectForKey:"), k2);
    if n1.is_null() || n2.is_null() {
        return None;
    }
    Some((
        msg_u64(n1, sel(c"unsignedLongLongValue")),
        msg_u64(n2, sel(c"unsignedLongLongValue")),
    ))
}
