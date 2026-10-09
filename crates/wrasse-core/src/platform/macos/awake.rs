//! Keeps the Mac from idle-sleeping while a cleanup runs (PLAN.md §4.3 rule 9).

use std::ffi::c_void;

type CFRef = *const c_void;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMAssertionCreateWithName(kind: CFRef, level: u32, name: CFRef, id: *mut u32) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(alloc: CFRef, s: *const std::ffi::c_char, encoding: u32) -> CFRef;
    fn CFRelease(cf: CFRef);
}

const UTF8: u32 = 0x0800_0100;
const LEVEL_ON: u32 = 255;

/// Released when dropped.
pub struct Awake(Option<u32>);

impl Drop for Awake {
    fn drop(&mut self) {
        if let Some(id) = self.0 {
            // SAFETY: releases the assertion created below.
            unsafe {
                IOPMAssertionRelease(id);
            }
        }
    }
}

pub fn keep_awake(reason: &str) -> Awake {
    let kind = c"PreventUserIdleSystemSleep";
    let Ok(name) = std::ffi::CString::new(reason) else {
        return Awake(None);
    };
    // SAFETY: valid C strings; the CF strings are released after the call.
    unsafe {
        let kind = CFStringCreateWithCString(std::ptr::null(), kind.as_ptr(), UTF8);
        let name = CFStringCreateWithCString(std::ptr::null(), name.as_ptr(), UTF8);
        let mut id = 0u32;
        let rc = IOPMAssertionCreateWithName(kind, LEVEL_ON, name, &mut id);
        CFRelease(kind);
        CFRelease(name);
        Awake((rc == 0).then_some(id))
    }
}
