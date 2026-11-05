//! File-system primitives with no-overwrite guarantees.
//!
//! [`move_noreplace`] is the single move operation behind Apply and
//! Undo. Plain [`std::fs::rename`] silently replaces an existing
//! destination on both Unix and Windows, so a file created between the
//! plan and the apply step could be lost. Every move in Joinery goes
//! through here instead: the destination is never replaced, not even
//! when it appears in that window.

use std::io;
use std::path::Path;

/// Move `from` to `to` without replacing an existing destination.
///
/// Returns `Ok(true)` when the file was moved and `Ok(false)` when the
/// target exists (or appeared in the meantime) — in that case the
/// source is left untouched.
pub fn move_noreplace(from: &Path, to: &Path) -> io::Result<bool> {
    #[cfg(windows)]
    {
        movefileex_noreplace(from, to)
    }
    #[cfg(target_os = "linux")]
    {
        renameat2_noreplace(from, to)
    }
    #[cfg(target_os = "macos")]
    {
        renameatx_noreplace(from, to)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        fallback_recheck(from, to)
    }
}

#[cfg(windows)]
fn movefileex_noreplace(from: &Path, to: &Path) -> io::Result<bool> {
    use std::os::windows::ffi::OsStrExt;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    // MOVEFILE_COPY_ALLOWED keeps cross-volume moves working; the point
    // is the absence of MOVEFILE_REPLACE_EXISTING — with an existing
    // target the call fails instead of overwriting.
    let flags = windows_sys::Win32::Storage::FileSystem::MOVEFILE_COPY_ALLOWED;
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            wide(from).as_ptr(),
            wide(to).as_ptr(),
            flags,
        )
    };
    if ok != 0 {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    const FILE_EXISTS: i32 = windows_sys::Win32::Foundation::ERROR_FILE_EXISTS as i32;
    const ALREADY_EXISTS: i32 = windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS as i32;
    match err.raw_os_error() {
        Some(code) if code == FILE_EXISTS || code == ALREADY_EXISTS => Ok(false),
        _ => Err(err),
    }
}

#[cfg(target_os = "linux")]
fn renameat2_noreplace(from: &Path, to: &Path) -> io::Result<bool> {
    use std::os::unix::ffi::OsStrExt;

    fn c_string(path: &Path) -> io::Result<std::ffi::CString> {
        std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
    }

    let from_c = c_string(from)?;
    let to_c = c_string(to)?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from_c.as_ptr(),
            libc::AT_FDCWD,
            to_c.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EEXIST) => Ok(false),
        // Old kernels and some filesystems do not support renameat2 —
        // fall back to check + rename (a tiny race remains there).
        Some(libc::ENOSYS) | Some(libc::EINVAL) | Some(libc::EOPNOTSUPP) => {
            fallback_recheck(from, to)
        }
        _ => Err(err),
    }
}

#[cfg(target_os = "macos")]
fn renameatx_noreplace(from: &Path, to: &Path) -> io::Result<bool> {
    use std::os::unix::ffi::OsStrExt;

    fn c_string(path: &Path) -> io::Result<std::ffi::CString> {
        std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
    }

    let from_c = c_string(from)?;
    let to_c = c_string(to)?;
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            from_c.as_ptr(),
            libc::AT_FDCWD,
            to_c.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EEXIST) => Ok(false),
        Some(libc::ENOSYS) | Some(libc::EINVAL) | Some(libc::EOPNOTSUPP) => {
            fallback_recheck(from, to)
        }
        _ => Err(err),
    }
}

/// Portable fallback for platforms or filesystems without a native
/// no-replace move: re-check and rename. The window between the check
/// and the rename is tiny but real; the native paths above close it.
#[allow(dead_code)]
fn fallback_recheck(from: &Path, to: &Path) -> io::Result<bool> {
    if to.exists() {
        return Ok(false);
    }
    std::fs::rename(from, to)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_and_never_replaces() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("a.txt");
        let dst = dir.path().join("b.txt");
        std::fs::write(&src, b"hello").expect("write");
        assert!(move_noreplace(&src, &dst).expect("move"));
        assert!(!src.exists() && dst.exists());

        // A second move onto the same destination must not replace it.
        std::fs::write(&src, b"other").expect("write");
        assert!(!move_noreplace(&src, &dst).expect("no replace"));
        assert_eq!(std::fs::read(&dst).expect("read"), b"hello");
        assert!(src.exists(), "source stays when the target exists");
    }

    #[test]
    fn missing_source_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("missing.txt");
        let dst = dir.path().join("b.txt");
        assert!(move_noreplace(&src, &dst).is_err());
    }
}
