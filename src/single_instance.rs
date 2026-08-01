#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;

use crate::logging::log_stderr;

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(
        lpMutexAttributes: *mut std::ffi::c_void,
        bInitialOwner: i32,
        lpName: *const u16,
    ) -> *mut std::ffi::c_void;
    fn GetLastError() -> u32;
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(
        hWnd: *mut std::ffi::c_void,
        lpText: *const u16,
        lpCaption: *const u16,
        uType: u32,
    ) -> i32;
}

const ERROR_ALREADY_EXISTS: u32 = 183;

/// RAII guard that holds the single-instance mutex for the process lifetime.
/// When dropped, the mutex handle is closed automatically by Windows on
/// process exit — but we store it here to prevent premature release.
pub(crate) struct SingleInstanceGuard {
    #[cfg(target_os = "windows")]
    _handle: *mut std::ffi::c_void,
}

// The handle is process-wide and never sent across threads in practice,
// but we need Send so it can live in main()'s stack frame.
unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

/// Attempt to acquire a system-wide single-instance mutex.
///
/// Returns `Some(guard)` on success — hold the guard until `main()` returns.
/// Returns `None` if another instance already owns the mutex (shows a MessageBox)
/// or if the OS call fails.
#[cfg(target_os = "windows")]
pub(crate) fn acquire_single_instance() -> Option<SingleInstanceGuard> {
    let mutex_name = "Global\\fk_kuro_launcher_single_instance";
    let wide_name: Vec<u16> = std::ffi::OsStr::new(mutex_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 1, wide_name.as_ptr()) };

    if handle.is_null() {
        let err = unsafe { GetLastError() };
        log_stderr(&format!(
            "[ERROR] CreateMutexW failed with OS error {}",
            err
        ));
        return None;
    }

    let err = unsafe { GetLastError() };
    if err == ERROR_ALREADY_EXISTS {
        // Another instance owns the mutex — show a user-facing message.
        let text: Vec<u16> = "fk_kuro_launcher is already running."
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let caption: Vec<u16> = "fk_kuro_launcher"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                0x00000040, // MB_ICONINFORMATION
            );
        }
        return None;
    }

    Some(SingleInstanceGuard { _handle: handle })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn acquire_single_instance() -> Option<SingleInstanceGuard> {
    // No-op on non-Windows — always succeed.
    Some(SingleInstanceGuard {})
}
