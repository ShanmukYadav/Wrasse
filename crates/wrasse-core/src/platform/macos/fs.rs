//! macOS file-system queries used by the scanner. All read-only.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::macos::fs::MetadataExt as MacMetadataExt;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// `SF_DATALESS` (sys/stat.h): an iCloud placeholder whose data is not stored locally.
const SF_DATALESS: u32 = 0x4000_0000;
/// `EF_MAY_SHARE_BLOCKS` (sys/stat.h): the file may share blocks with an APFS clone.
const EF_MAY_SHARE_BLOCKS: u64 = 0x1;
/// `ATTR_CMNEXT_EXT_FLAGS` (sys/attr.h); libc does not export it yet.
const ATTR_CMNEXT_EXT_FLAGS: u32 = 0x0000_0200;
/// `QOS_CLASS_UTILITY` (sys/qos.h).
const QOS_CLASS_UTILITY: u32 = 0x11;

extern "C" {
    fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
}

pub fn is_dataless(md: &fs::Metadata) -> bool {
    md.st_flags() & SF_DATALESS != 0
}

/// Runs the calling thread at utility QoS, so scans yield to everything the user is doing.
pub fn lower_thread_priority() {
    // SAFETY: plain FFI call on the current thread with a valid QoS class.
    unsafe {
        pthread_set_qos_class_self_np(QOS_CLASS_UTILITY, 0);
    }
}

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

fn read_u64(buf: &[u8], at: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[at..at + 8]);
    u64::from_ne_bytes(b)
}

/// For an APFS clone, the bytes it shares with other files: allocated size minus its private
/// size (`ATTR_CMNEXT_PRIVATESIZE`). `None` when the file shares nothing or the volume can't say.
pub fn clone_shared_bytes(path: &Path, allocated: u64) -> Option<u64> {
    let cpath = c_path(path).ok()?;
    let mut attrs = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: libc::ATTR_CMNEXT_PRIVATESIZE | ATTR_CMNEXT_EXT_FLAGS,
    };
    // u32 length, then off_t private size, then u64 extended flags (packed, 4-byte aligned).
    let mut buf = [0u8; 4 + 8 + 8];
    // SAFETY: valid NUL-terminated path, attrlist and buffer of the stated size.
    let rc = unsafe {
        libc::getattrlist(
            cpath.as_ptr(),
            (&mut attrs as *mut libc::attrlist).cast(),
            buf.as_mut_ptr().cast(),
            buf.len(),
            (libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED) as _,
        )
    };
    if rc != 0 {
        return None;
    }
    let private = read_u64(&buf, 4);
    let ext_flags = read_u64(&buf, 12);
    if ext_flags & EF_MAY_SHARE_BLOCKS == 0 {
        return None;
    }
    Some(allocated.saturating_sub(private))
}

/// Space used by one APFS volume (`ATTR_VOL_SPACEUSED`). Volumes in a container share free space,
/// so this is the only way to tell the sealed system volume from the data volume.
pub fn volume_space_used(path: &Path) -> Option<u64> {
    let cpath = c_path(path).ok()?;
    let mut attrs = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: libc::ATTR_VOL_INFO | libc::ATTR_VOL_SPACEUSED,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut buf = [0u8; 4 + 8];
    // SAFETY: as above.
    let rc = unsafe {
        libc::getattrlist(
            cpath.as_ptr(),
            (&mut attrs as *mut libc::attrlist).cast(),
            buf.as_mut_ptr().cast(),
            buf.len(),
            0,
        )
    };
    (rc == 0).then(|| read_u64(&buf, 4))
}

/// Total size and space available to the user on the volume holding `path` (statvfs).
pub fn container_space(path: &Path) -> Option<(u64, u64)> {
    let cpath = c_path(path).ok()?;
    // SAFETY: statvfs fills the struct we pass for a valid path.
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(cpath.as_ptr(), &mut st) };
    (rc == 0).then(|| {
        let unit = st.f_frsize as u64;
        (st.f_blocks as u64 * unit, st.f_bavail as u64 * unit)
    })
}

/// Full Disk Access: folders macOS protects (TCC) can be listed only with it.
pub fn full_disk_access(home: &Path) -> bool {
    let probes = [
        home.join("Library/Safari"),
        home.join("Library/Mail"),
        "/Library/Application Support/com.apple.TCC".into(),
    ];
    let mut saw_any = false;
    for p in probes {
        match fs::read_dir(&p) {
            Ok(_) => return true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => saw_any = true,
        }
    }
    // Nothing to probe (unusual): don't nag.
    !saw_any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volumes_report_space() {
        let (total, avail) = container_space(Path::new("/")).unwrap();
        assert!(total > 0 && avail <= total);
        assert!(volume_space_used(Path::new("/")).unwrap() > 0);
    }

    #[test]
    fn clones_report_shared_bytes() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("wrasse-clone-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let original = dir.join("original.bin");
        let clone = dir.join("clone.bin");
        let mut f = fs::File::create(&original).unwrap();
        f.write_all(&vec![3u8; 4_000_000]).unwrap();
        f.sync_all().unwrap();
        // APFS clonefile via the standard library's copy (it clones on APFS).
        let _ = fs::copy(&original, &clone);
        let allocated = fs::metadata(&clone)
            .map(|m| std::os::unix::fs::MetadataExt::blocks(&m) * 512)
            .unwrap();
        if let Some(shared) = clone_shared_bytes(&clone, allocated) {
            assert!(shared > 0 && shared <= allocated);
        }
        crate::platform::remove_test_dir(&dir);
    }
}
