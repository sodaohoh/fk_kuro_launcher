use std::fs;
use std::io;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::Duration;

use crate::logging::{get_appdata_dir, log_stderr, log_stdout};
use crate::paths::get_game_root_dir;

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;

#[cfg(target_os = "windows")]
#[repr(C)]
#[allow(non_snake_case)]
struct SHELLEXECUTEINFOW {
    cbSize: u32,
    fMask: u32,
    hwnd: *mut std::ffi::c_void,
    lpVerb: *const u16,
    lpFile: *const u16,
    lpParameters: *const u16,
    lpDirectory: *const u16,
    nShow: i32,
    hInstApp: *mut std::ffi::c_void,
    lpIDList: *mut std::ffi::c_void,
    lpClass: *const u16,
    hkeyClass: *mut std::ffi::c_void,
    dwHotKey: u32,
    hIconOrMonitor: *mut std::ffi::c_void,
    hProcess: *mut std::ffi::c_void,
}

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteExW(pExecInfo: *mut SHELLEXECUTEINFOW) -> i32;
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

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn GetProcessId(hProcess: *mut std::ffi::c_void) -> u32;
    fn GetExitCodeProcess(hProcess: *mut std::ffi::c_void, lpExitCode: *mut u32) -> i32;
    fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
}

#[derive(Debug)]
pub(crate) struct Win32Child {
    handle: *mut std::ffi::c_void,
    pid: u32,
}

unsafe impl Send for Win32Child {}
unsafe impl Sync for Win32Child {}

impl Drop for Win32Child {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            #[cfg(target_os = "windows")]
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

impl Win32Child {
    pub(crate) fn id(&self) -> u32 {
        self.pid
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        #[cfg(target_os = "windows")]
        {
            let mut exit_code: u32 = 0;
            let ret = unsafe { GetExitCodeProcess(self.handle, &mut exit_code) };
            if ret == 0 {
                return Err(io::Error::last_os_error());
            }
            const STILL_ACTIVE: u32 = 259;
            if exit_code == STILL_ACTIVE {
                Ok(None)
            } else {
                use std::os::windows::process::ExitStatusExt;
                Ok(Some(ExitStatus::from_raw(exit_code)))
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub(crate) enum GameChild {
    Standard(Child),
    Win32(Win32Child),
}

impl GameChild {
    pub(crate) fn id(&self) -> u32 {
        match self {
            GameChild::Standard(child) => child.id(),
            GameChild::Win32(child) => child.id(),
        }
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self {
            GameChild::Standard(child) => child.try_wait(),
            GameChild::Win32(child) => child.try_wait(),
        }
    }
}

#[cfg(target_os = "windows")]
fn spawn_via_shellexecute(
    game_exe: &Path,
    game_args: &[String],
    game_root_dir: &Path,
) -> io::Result<GameChild> {
    if !game_exe.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Executable file not found",
        ));
    }
    let verb_wide: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
    let file_wide: Vec<u16> = game_exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dir_wide: Vec<u16> = game_root_dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let params_wide: Option<Vec<u16>> = if game_args.is_empty() {
        None
    } else {
        let params_str = game_args
            .iter()
            .map(|arg| {
                if arg.contains(' ') || arg.contains('\t') || arg.contains('"') {
                    format!("\"{}\"", arg.replace('"', "\\\""))
                } else {
                    arg.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        Some(
            params_str
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect(),
        )
    };

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: 0x00000040,
        hwnd: std::ptr::null_mut(),
        lpVerb: verb_wide.as_ptr(),
        lpFile: file_wide.as_ptr(),
        lpParameters: params_wide
            .as_ref()
            .map_or(std::ptr::null(), |p| p.as_ptr()),
        lpDirectory: dir_wide.as_ptr(),
        nShow: 1,
        hInstApp: std::ptr::null_mut(),
        lpIDList: std::ptr::null_mut(),
        lpClass: std::ptr::null(),
        hkeyClass: std::ptr::null_mut(),
        dwHotKey: 0,
        hIconOrMonitor: std::ptr::null_mut(),
        hProcess: std::ptr::null_mut(),
    };

    let res = unsafe { ShellExecuteExW(&mut info) };
    if res == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.hProcess.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "ShellExecuteExW succeeded without a process handle",
        ));
    }

    let pid = unsafe { GetProcessId(info.hProcess) };
    Ok(GameChild::Win32(Win32Child {
        handle: info.hProcess,
        pid,
    }))
}

#[cfg(not(target_os = "windows"))]
fn spawn_via_shellexecute(
    _game_exe: &Path,
    _game_args: &[String],
    _game_root_dir: &Path,
) -> io::Result<GameChild> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "ShellExecuteExW is only supported on Windows",
    ))
}

pub(crate) fn spawn_game_process(game_exe: &str, game_args: &[String]) -> io::Result<GameChild> {
    let game_exe_path = Path::new(game_exe);
    let game_root_dir = get_game_root_dir(game_exe_path);
    match Command::new(game_exe)
        .args(game_args)
        .current_dir(&game_root_dir)
        .spawn()
    {
        Ok(child) => Ok(GameChild::Standard(child)),
        Err(e) => {
            log_stdout(&format!(
                "[WARN] Direct spawn failed for {}: {}. Attempting ShellExecute runas fallback...",
                game_exe, e
            ));
            spawn_via_shellexecute(game_exe_path, game_args, &game_root_dir)
        }
    }
}

#[cfg(target_os = "windows")]
fn show_error_message_box(game_exe: &str, e: &io::Error) {
    let title: Vec<u16> = "fk_kuro_launcher Error\0".encode_utf16().collect();
    let message_str = format!(
        "Failed to spawn game process:\n{}\n\nError: {}",
        game_exe, e
    );
    let text: Vec<u16> = message_str
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            0x00000010,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_error_message_box(_game_exe: &str, _e: &io::Error) {}

pub(crate) fn handle_spawn_error(game_exe: &str, e: &io::Error) {
    let err_msg = format!("[ERROR] Failed to spawn game process {}: {}", game_exe, e);
    log_stderr(&err_msg);
    let appdata_dir = get_appdata_dir();
    let _ = fs::create_dir_all(&appdata_dir);
    let error_log_path = appdata_dir.join("fk_kuro_launcher_error.log");
    let _ = fs::write(&error_log_path, &err_msg);
    show_error_message_box(game_exe, e);
    thread::sleep(Duration::from_secs(10));
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_game_process_nonexistent() {
        let res = spawn_game_process("non_existent_game_exe_12345.exe", &[]);
        assert!(res.is_err(), "Spawning a non-existent binary should return Err");
    }
}
