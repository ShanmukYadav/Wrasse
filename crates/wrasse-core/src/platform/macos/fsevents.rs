//! FSEvents: which folders changed since an event ID (PLAN.md §5.4, incremental rescans).
//!
//! macOS keeps a persistent per-volume log of folder-level changes. A scan stores the current event
//! ID; the next scan replays the log from there (history only, no live watching) and re-reads just
//! the folders it names. When the log can't answer (events dropped, IDs wrapped, the volume's log
//! was reset, too many changes, or no answer in time) the caller falls back to a full scan.
//! Read-only: this only reads the event log.

use std::ffi::{c_char, c_void, CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

type CFRef = *const c_void;

// FSEvents.h
const FLAG_MUST_SCAN_SUBDIRS: u32 = 0x0000_0001;
const FLAG_USER_DROPPED: u32 = 0x0000_0002;
const FLAG_KERNEL_DROPPED: u32 = 0x0000_0004;
const FLAG_EVENT_IDS_WRAPPED: u32 = 0x0000_0008;
const FLAG_HISTORY_DONE: u32 = 0x0000_0010;
const FLAG_ROOT_CHANGED: u32 = 0x0000_0020;
const CREATE_FLAG_NONE: u32 = 0;
// CFString.h
const UTF8: u32 = 0x0800_0100;

/// More changed folders than this and a full scan is quicker than replaying them.
const MAX_CHANGES: usize = 20_000;

#[repr(C)]
struct StreamContext {
    version: isize,
    info: *mut c_void,
    retain: *const c_void,
    release: *const c_void,
    copy_description: *const c_void,
}

type Callback = extern "C" fn(
    stream: CFRef,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const u32,
    ids: *const u64,
);

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn FSEventsGetCurrentEventId() -> u64;
    fn FSEventsCopyUUIDForDevice(dev: i32) -> CFRef;
    fn FSEventStreamCreate(
        allocator: CFRef,
        callback: Callback,
        context: *mut StreamContext,
        paths: CFRef,
        since: u64,
        latency: f64,
        flags: u32,
    ) -> *mut c_void;
    fn FSEventStreamSetDispatchQueue(stream: *mut c_void, queue: *mut c_void);
    fn FSEventStreamStart(stream: *mut c_void) -> u8;
    fn FSEventStreamStop(stream: *mut c_void);
    fn FSEventStreamInvalidate(stream: *mut c_void);
    fn FSEventStreamRelease(stream: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFTypeArrayCallBacks: c_void;
    fn CFStringCreateWithCString(alloc: CFRef, s: *const c_char, encoding: u32) -> CFRef;
    fn CFStringGetCString(s: CFRef, buf: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFArrayCreate(
        alloc: CFRef,
        values: *const CFRef,
        count: isize,
        callbacks: *const c_void,
    ) -> CFRef;
    fn CFUUIDCreateString(alloc: CFRef, uuid: CFRef) -> CFRef;
    fn CFRelease(cf: CFRef);
}

extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
}

/// The newest event ID on this Mac (store it just before a scan starts).
pub(crate) fn current_event_id() -> u64 {
    // SAFETY: no arguments, returns a plain integer.
    unsafe { FSEventsGetCurrentEventId() }
}

/// Identifies a volume's event log; if it differs from the stored one, the log was reset.
pub(crate) fn volume_uuid(dev: u64) -> Option<String> {
    // SAFETY: CF calls with valid arguments; every created object is released.
    unsafe {
        let uuid = FSEventsCopyUUIDForDevice(dev as i32);
        if uuid.is_null() {
            return None;
        }
        let s = CFUUIDCreateString(std::ptr::null(), uuid);
        CFRelease(uuid);
        if s.is_null() {
            return None;
        }
        let mut buf = [0 as c_char; 64];
        let ok = CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8);
        CFRelease(s);
        (ok != 0).then(|| CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

#[derive(Default)]
struct Collected {
    /// (path, recursive)
    changes: Vec<(String, bool)>,
    full_scan: bool,
    done: bool,
}

struct Shared {
    state: Mutex<Collected>,
    signal: Condvar,
}

extern "C" fn on_events(
    _stream: CFRef,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const u32,
    _ids: *const u64,
) {
    // SAFETY: `info` is the `Shared` passed at creation and outlives the stream; without
    // UseCFTypes, `paths` is an array of `count` C strings and `flags` has `count` entries.
    let shared = unsafe { &*(info as *const Shared) };
    let paths = paths as *const *const c_char;
    let Ok(mut state) = shared.state.lock() else {
        return;
    };
    for i in 0..count {
        let (path, f) = unsafe {
            (
                CStr::from_ptr(*paths.add(i)).to_string_lossy().into_owned(),
                *flags.add(i),
            )
        };
        if f & FLAG_HISTORY_DONE != 0 {
            state.done = true;
            continue;
        }
        if f & (FLAG_USER_DROPPED
            | FLAG_KERNEL_DROPPED
            | FLAG_EVENT_IDS_WRAPPED
            | FLAG_ROOT_CHANGED)
            != 0
        {
            state.full_scan = true;
            continue;
        }
        state.changes.push((path, f & FLAG_MUST_SCAN_SUBDIRS != 0));
        if state.changes.len() > MAX_CHANGES {
            state.full_scan = true;
        }
    }
    if state.done || state.full_scan {
        shared.signal.notify_all();
    }
}

/// Folders under `watch` that changed since `since`, as (path, recursive). `None` means the log
/// can't say reliably and a full scan is needed.
pub(crate) fn changes_since(
    watch: &Path,
    since: u64,
    timeout: Duration,
) -> Option<Vec<(PathBuf, bool)>> {
    if since == 0 || since > current_event_id() {
        return None;
    }
    let shared = Box::new(Shared {
        state: Mutex::new(Collected::default()),
        signal: Condvar::new(),
    });
    let cpath = CString::new(watch.as_os_str().as_bytes()).ok()?;
    // SAFETY: CF/FSEvents/dispatch calls with valid arguments. The stream is stopped, invalidated
    // and released before `shared` is dropped, so the callback never sees a dangling pointer.
    unsafe {
        let path = CFStringCreateWithCString(std::ptr::null(), cpath.as_ptr(), UTF8);
        if path.is_null() {
            return None;
        }
        let values = [path];
        let array = CFArrayCreate(std::ptr::null(), values.as_ptr(), 1, &kCFTypeArrayCallBacks);
        CFRelease(path);
        let mut context = StreamContext {
            version: 0,
            info: &*shared as *const Shared as *mut c_void,
            retain: std::ptr::null(),
            release: std::ptr::null(),
            copy_description: std::ptr::null(),
        };
        let stream = FSEventStreamCreate(
            std::ptr::null(),
            on_events,
            &mut context,
            array,
            since,
            0.0,
            CREATE_FLAG_NONE,
        );
        CFRelease(array);
        if stream.is_null() {
            return None;
        }
        let label = CString::new("com.shanmukyadav.wrasse.fsevents").ok()?;
        let queue = dispatch_queue_create(label.as_ptr(), std::ptr::null());
        FSEventStreamSetDispatchQueue(stream, queue);
        let started = FSEventStreamStart(stream) != 0;
        let mut finished = false;
        if started {
            if let Ok(guard) = shared.state.lock() {
                if let Ok((state, _)) = shared
                    .signal
                    .wait_timeout_while(guard, timeout, |s| !s.done && !s.full_scan)
                {
                    finished = state.done && !state.full_scan;
                }
            }
            FSEventStreamStop(stream);
        }
        FSEventStreamInvalidate(stream);
        FSEventStreamRelease(stream);
        dispatch_release(queue);
        if !finished {
            return None;
        }
    }
    let state = shared.state.into_inner().ok()?;
    let mut out: Vec<(PathBuf, bool)> = state
        .changes
        .into_iter()
        .map(|(p, r)| (PathBuf::from(p.trim_end_matches('/')), r))
        .collect();
    out.sort();
    out.dedup();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_folder_changed_after_an_event_id() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = std::env::temp_dir().join(format!("wrasse-fsevents-{nanos}"));
        std::fs::create_dir_all(&scratch).unwrap();
        let dir = std::fs::canonicalize(&scratch).unwrap();
        let before = current_event_id();
        std::fs::write(dir.join("new.txt"), b"hello").unwrap();
        // fseventsd writes its log asynchronously; give it a moment.
        let mut found = false;
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(250));
            if let Some(changes) = changes_since(&dir, before, Duration::from_secs(5)) {
                if changes.iter().any(|(p, _)| p == &dir) {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "the changed folder should be reported");
        crate::platform::remove_test_dir(&scratch);
    }
}
