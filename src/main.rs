use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use std::thread;
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Build byte substitution table (LUT) for Kuro Games Client.log
fn build_lut() -> [Option<char>; 256] {
    let mut lut = [None; 256];

    // Punctuation & Symbols
    lut[0xb4] = Some('[');
    lut[0xb2] = Some(']');
    lut[0x8b] = Some('.');
    lut[0x9f] = Some(':');
    lut[0xc2] = Some('-');
    lut[0xaf] = Some('\n');
    lut[0xe2] = Some('\r');
    lut[0x8d] = Some('(');
    lut[0xc6] = Some(')');
    lut[0xd4] = Some(',');
    lut[0x9e] = Some('q');
    lut[0xc0] = Some('\\');
    lut[0x2f] = Some('/');
    lut[0x3a] = Some(':');
    lut[0xfd] = Some(' ');
    lut[0x85] = Some(' ');
    lut[0xa0] = Some('O');

    // Digits 0 - 9
    lut[0x95] = Some('0');
    lut[0xde] = Some('1');
    lut[0x97] = Some('2');
    lut[0xdc] = Some('3');
    lut[0x91] = Some('4');
    lut[0xda] = Some('5');
    lut[0x93] = Some('6');
    lut[0xd8] = Some('7');
    lut[0x9d] = Some('8');
    lut[0xd6] = Some('9');

    // Uppercase Letters
    lut[0xae] = Some('A');
    lut[0xac] = Some('C');
    lut[0xb8] = Some('W');
    lut[0xe9] = Some('L');
    lut[0xa4] = Some('K');
    lut[0x9c] = Some('S');
    lut[0xf7] = Some('R');
    lut[0xf1] = Some('T');
    lut[0xa2] = Some('M');
    lut[0xaa] = Some('E');
    lut[0xe1] = Some('V');
    lut[0xf5] = Some('P');
    lut[0xed] = Some('H');
    lut[0xeb] = Some('N');
    lut[0xa8] = Some('G');
    lut[0xbe] = Some('Q');
    lut[0xe7] = Some('B');
    lut[0xd2] = Some('|');

    // Lowercase Letters
    lut[0x8e] = Some('a');
    lut[0x8c] = Some('c');
    lut[0xc1] = Some('d');
    lut[0x8a] = Some('e');
    lut[0xc3] = Some('f');
    lut[0x88] = Some('g');
    lut[0xcd] = Some('h');
    lut[0x86] = Some('i');
    lut[0xc9] = Some('l');
    lut[0x82] = Some('m');
    lut[0xcb] = Some('n');
    lut[0x80] = Some('o');
    lut[0xd5] = Some('p');
    lut[0xd7] = Some('r');
    lut[0xbc] = Some('s');
    lut[0xd1] = Some('t');
    lut[0x9a] = Some('u');
    lut[0xd3] = Some('v');
    lut[0x98] = Some('w');
    lut[0xdd] = Some('x');
    lut[0x96] = Some('y');
    lut[0xdf] = Some('z');

    lut
}

/// Decode raw bytes using substitution LUT
fn decode_bytes(bytes: &[u8], lut: &[Option<char>; 256]) -> String {
    let mut decoded = String::with_capacity(bytes.len());
    for &b in bytes {
        if let Some(c) = lut[b as usize] {
            decoded.push(c);
        } else {
            decoded.push(b as char);
        }
    }
    decoded
}

pub fn get_game_root_dir(game_exe: &Path) -> PathBuf {
    let path_str = game_exe.to_string_lossy();
    let lower = path_str.to_lowercase();
    if lower.contains("client/binaries/win64") || lower.contains(r"client\binaries\win64") {
        if let Some(p1) = game_exe.parent() {
            if let Some(p2) = p1.parent() {
                if let Some(p3) = p2.parent() {
                    if let Some(p4) = p3.parent() {
                        if p4.as_os_str().is_empty() {
                            return PathBuf::from(".");
                        }
                        return p4.to_path_buf();
                    }
                }
            }
        }
    }
    let parent = game_exe.parent().unwrap_or(Path::new("."));
    if parent.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        parent.to_path_buf()
    }
}

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

fn get_appdata_dir() -> PathBuf {
    if let Ok(local) = env::var("LOCALAPPDATA") {
        if !local.trim().is_empty() {
            return PathBuf::from(local).join("fk_kuro_launcher");
        }
    }
    if let Ok(profile) = env::var("USERPROFILE") {
        if !profile.trim().is_empty() {
            return PathBuf::from(profile)
                .join("AppData")
                .join("Local")
                .join("fk_kuro_launcher");
        }
    }
    env::temp_dir().join("fk_kuro_launcher")
}

fn get_launcher_log_path() -> PathBuf {
    let dir = get_appdata_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join("launcher.log")
}

static LOG_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn write_to_launcher_log(msg: &str) {
    let _guard = LOG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let log_path = get_launcher_log_path();
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{}", msg);
    }
}

fn log_stdout(msg: &str) {
    println!("{}", msg);
    write_to_launcher_log(msg);
}

fn log_stderr(msg: &str) {
    eprintln!("{}", msg);
    write_to_launcher_log(msg);
}

#[derive(Debug)]
pub struct Win32Child {
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
    pub fn id(&self) -> u32 {
        self.pid
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
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
pub enum GameChild {
    Standard(Child),
    Win32(Win32Child),
}

impl GameChild {
    pub fn id(&self) -> u32 {
        match self {
            GameChild::Standard(child) => child.id(),
            GameChild::Win32(child) => child.id(),
        }
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
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
        Some(params_str.encode_utf16().chain(std::iter::once(0)).collect())
    };

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: 0x00000040, // SEE_MASK_NOCLOSEPROCESS
        hwnd: std::ptr::null_mut(),
        lpVerb: verb_wide.as_ptr(),
        lpFile: file_wide.as_ptr(),
        lpParameters: params_wide.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
        lpDirectory: dir_wide.as_ptr(),
        nShow: 1, // SW_SHOWNORMAL
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

fn spawn_game_process(game_exe: &str, game_args: &[String]) -> io::Result<GameChild> {
    let game_exe_path = Path::new(game_exe);
    let game_root_dir = get_game_root_dir(game_exe_path);

    let spawn_res = Command::new(game_exe)
        .args(game_args)
        .current_dir(&game_root_dir)
        .spawn();

    match spawn_res {
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
    let message_str = format!("Failed to spawn game process:\n{}\n\nError: {}", game_exe, e);
    let text: Vec<u16> = message_str.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            0x00000010, // MB_OK | MB_ICONERROR
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_error_message_box(_game_exe: &str, _e: &io::Error) {}

fn handle_spawn_error(game_exe: &str, e: &io::Error) {
    let err_msg = format!("[ERROR] Failed to spawn game process {}: {}", game_exe, e);
    log_stderr(&err_msg);

    let appdata_dir = get_appdata_dir();
    let _ = fs::create_dir_all(&appdata_dir);
    let error_log_path = appdata_dir.join("fk_kuro_launcher_error.log");
    let _ = fs::write(&error_log_path, &err_msg);

    show_error_message_box(game_exe, e);

    thread::sleep(Duration::from_secs(10));
}

fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let mut parts = s.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let patch_str = parts.next()?;
    let patch_num_str: String = patch_str.chars().take_while(|c| c.is_ascii_digit()).collect();
    let patch: u64 = patch_num_str.parse().ok()?;
    Some((major, minor, patch))
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    if let (Some(cur), Some(lat)) = (parse_version(current), parse_version(latest)) {
        lat > cur
    } else {
        false
    }
}

fn apply_update(current_exe: &Path, new_exe: &Path) -> io::Result<()> {
    let old_exe = PathBuf::from(format!("{}.old", current_exe.display()));

    // Clean up any existing .old file from previous updates
    if old_exe.exists() {
        let _ = fs::remove_file(&old_exe);
    }

    // Rename currently running executable <exe_path> -> <exe_path>.old
    fs::rename(current_exe, &old_exe)?;

    // Move downloaded <exe_path>.new -> <exe_path>
    if let Err(e) = fs::rename(new_exe, current_exe) {
        // Rollback rename if moving new_exe failed
        let _ = fs::rename(&old_exe, current_exe);
        return Err(e);
    }

    // Clean up .old file if possible (may be locked on Windows until process exits)
    let _ = fs::remove_file(&old_exe);

    Ok(())
}

fn check_latest_release(current_version: &str) {
    let output = Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-NoProfile",
            "-Command",
            "$ProgressPreference = 'SilentlyContinue'; try { (Invoke-RestMethod -Uri 'https://api.github.com/repos/sodaohoh/fk_kuro_launcher/releases/latest' -UserAgent 'fk_kuro_launcher' -TimeoutSec 5).tag_name } catch {}",
        ])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let latest_tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !latest_tag.is_empty() && is_newer_version(current_version, &latest_tag) {
                log_stdout(&format!(
                    "[INFO] New version detected ({}). Performing automatic update...",
                    latest_tag
                ));

                if let Ok(current_exe) = env::current_exe() {
                    let new_exe = PathBuf::from(format!("{}.new", current_exe.display()));
                    let download_url = "https://github.com/sodaohoh/fk_kuro_launcher/releases/latest/download/fk_kuro_launcher.exe";

                    let download_output = Command::new("powershell")
                        .creation_flags(CREATE_NO_WINDOW)
                        .args([
                            "-NoProfile",
                            "-Command",
                            "$ProgressPreference = 'SilentlyContinue'; try { Invoke-WebRequest -Uri $args[0] -OutFile $args[1] -UseBasicParsing -ErrorAction Stop } catch { exit 1 }",
                            download_url,
                            new_exe.to_str().unwrap_or_default(),
                        ])
                        .output();

                    match download_output {
                        Ok(out) if out.status.success() && new_exe.exists() => {
                            match apply_update(&current_exe, &new_exe) {
                                Ok(()) => {
                                    log_stdout(&format!(
                                        "[SUCCESS] Auto-update applied successfully! Version {} will run on next launch.",
                                        latest_tag
                                    ));
                                }
                                Err(e) => {
                                    log_stderr(&format!("[ERROR] Auto-update failed to replace executable: {}", e));
                                    let _ = fs::remove_file(&new_exe);
                                }
                            }
                        }
                        Ok(_) | Err(_) => {
                            log_stderr("[ERROR] Auto-update failed during download.");
                            if new_exe.exists() {
                                let _ = fs::remove_file(&new_exe);
                            }
                        }
                    }
                } else {
                    log_stderr("[ERROR] Auto-update failed: could not determine executable path.");
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn get_steam_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(steam_key) = hkcu.open_subkey(r"Software\Valve\Steam") {
        if let Ok(steam_path) = steam_key.get_value::<String, _>("SteamPath") {
            if !steam_path.trim().is_empty() {
                return Some(PathBuf::from(steam_path));
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn get_steam_path_from_registry() -> Option<PathBuf> {
    None
}

fn get_shipping_exe_candidates(parent: &Path) -> Vec<PathBuf> {
    vec![
        parent.join("Client").join("Binaries").join("Win64").join("Client-Win64-Shipping.exe"),
        parent.join("Wuthering Waves Game").join("Client").join("Binaries").join("Win64").join("Client-Win64-Shipping.exe"),
        parent.join("Wuthering Waves").join("Client").join("Binaries").join("Win64").join("Client-Win64-Shipping.exe"),
        parent.join("Binaries").join("Win64").join("Client-Win64-Shipping.exe"),
    ]
}

fn get_log_candidates(parent: &Path) -> Vec<PathBuf> {
    vec![
        parent.join("Client").join("Saved").join("Logs").join("Client.log"),
        parent.join("Wuthering Waves Game").join("Client").join("Saved").join("Logs").join("Client.log"),
        parent.join("Wuthering Waves").join("Client").join("Saved").join("Logs").join("Client.log"),
        parent.join("Saved").join("Logs").join("Client.log"),
        parent.join("Client.log"),
    ]
}

pub fn resolve_paths(input_exe: Option<&str>) -> (String, String, Vec<String>) {
    let parent = if let Some(input_str) = input_exe {
        let path = Path::new(input_str);
        get_game_root_dir(path)
    } else if let Some(steam_path) = get_steam_path_from_registry() {
        let wuwa_dir = steam_path
            .join("steamapps")
            .join("common")
            .join("Wuthering Waves");
        if wuwa_dir.exists() {
            wuwa_dir
        } else {
            steam_path
                .join("steamapps")
                .join("common")
                .join("Wuthering Waves")
        }
    } else {
        PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Wuthering Waves")
    };

    let exe_candidates = get_shipping_exe_candidates(&parent);
    let game_exe = if let Some(found_exe) = exe_candidates.iter().find(|cand| cand.exists()) {
        found_exe.to_string_lossy().to_string()
    } else if let Some(input_str) = input_exe {
        let path = Path::new(input_str);
        if path.exists() {
            input_str.to_string()
        } else {
            exe_candidates[0].to_string_lossy().to_string()
        }
    } else {
        exe_candidates[0].to_string_lossy().to_string()
    };

    let log_candidates = get_log_candidates(&parent);
    let log_path = log_candidates
        .iter()
        .find(|cand| cand.exists())
        .unwrap_or(&log_candidates[0])
        .to_string_lossy()
        .to_string();

    (game_exe, log_path, vec![])
}

fn split_steam_command_args(args: &[String]) -> (Option<String>, Vec<String>) {
    if args.is_empty() {
        return (None, Vec::new());
    }

    let mut command = String::new();
    let mut command_end = None;

    for (index, arg) in args.iter().enumerate() {
        if !command.is_empty() {
            command.push(' ');
        }
        command.push_str(arg);

        let command_path = Path::new(&command);
        if command_path.is_file() || arg.to_ascii_lowercase().ends_with(".exe") {
            command_end = Some(index + 1);
            break;
        }
    }

    let command_end = command_end.unwrap_or(1);
    (
        Some(command),
        args.get(command_end..).unwrap_or_default().to_vec(),
    )
}

#[cfg(test)]
fn split_steam_command_args_for_test(args: &[&str]) -> (Option<String>, Vec<String>) {
    split_steam_command_args(
        &args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>(),
    )
}

fn main() {
    if let Ok(current_exe) = env::current_exe() {
        let old_exe = PathBuf::from(format!("{}.old", current_exe.display()));
        if old_exe.exists() {
            let _ = fs::remove_file(&old_exe);
        }
    }

    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-v") {
        println!(
            "fk_kuro_launcher v{} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("BUILD_GIT_HASH")
        );
        std::process::exit(0);
    }

    log_stdout(&format!(
        "[INFO] Version: v{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH")
    ));

    thread::spawn(|| {
        check_latest_release(env!("CARGO_PKG_VERSION"));
    });

    let (input_exe, mut game_args) = split_steam_command_args(&args[1..]);
    let (game_exe, log_path, resolved_args) = resolve_paths(input_exe.as_deref());
    game_args.extend(resolved_args);

    log_stdout("[INFO] Steam Wrapper Monitor Started.");
    log_stdout(&format!("[INFO] Executable Target: {}", game_exe));
    log_stdout(&format!("[INFO] Log Target: {}", log_path));

    let lut = build_lut();

    // Initialize offset to current end of file
    let mut offset: u64 = if let Ok(metadata) = fs::metadata(&log_path) {
        metadata.len()
    } else {
        0
    };

    // Spawn child process
    let mut game_child = match spawn_game_process(&game_exe, &game_args) {
        Ok(child) => {
            log_stdout(&format!("[INFO] Game spawned with PID: {}", child.id()));
            child
        }
        Err(e) => {
            handle_spawn_error(&game_exe, &e);
            return;
        }
    };

    let mut is_hotfix_restart = false;

    loop {
        // Check if child game process has exited
        match game_child.try_wait() {
            Ok(Some(status)) => {
                log_stdout(&format!("[INFO] Child game process exited with status: {}", status));
                if is_hotfix_restart {
                    log_stdout("[WARN] Hotfix restart detected! Respawning child process in 3s...");
                    thread::sleep(Duration::from_secs(3));
                    match spawn_game_process(&game_exe, &game_args) {
                        Ok(child) => {
                            log_stdout(&format!("[SUCCESS] Game respawned with PID: {}", child.id()));
                            game_child = child;
                            is_hotfix_restart = false;
                            if let Ok(meta) = fs::metadata(&log_path) {
                                offset = meta.len();
                            }
                            thread::sleep(Duration::from_secs(5));
                            continue;
                        }
                        Err(e) => {
                            handle_spawn_error(&game_exe, &e);
                            break;
                        }
                    }
                } else {
                    log_stdout("[INFO] Normal game exit detected. Steam Wrapper shutting down.");
                    break;
                }
            }
            Ok(None) => {
                // Child is still running
            }
            Err(e) => {
                log_stderr(&format!("[ERROR] Error waiting on child process: {}", e));
                break;
            }
        }

        // Monitor Client.log for hotfix restart triggers
        if let Ok(mut file) = OpenOptions::new()
            .read(true)
            .share_mode(7)
            .open(&log_path)
        {
            if let Ok(metadata) = file.metadata() {
                let file_len = metadata.len();
                if file_len < offset {
                    offset = 0;
                }

                if file_len > offset {
                    if file.seek(SeekFrom::Start(offset)).is_ok() {
                        let read_size = (file_len - offset) as usize;
                        let mut buffer = vec![0u8; read_size];
                        if let Ok(n) = file.read(&mut buffer) {
                            if n > 0 {
                                offset += n as u64;
                                let decoded_text = decode_bytes(&buffer[..n], &lut);

                                if decoded_text.contains("Engine exit requested")
                                    || decoded_text.contains("NeedRestart")
                                {
                                    log_stdout("[WARN] Hotfix restart requested by engine!");
                                    is_hotfix_restart = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(1000));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert_eq!(parse_version("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("v0.2.0"), Some((0, 2, 0)));
        assert_eq!(parse_version("V1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v0.2.0-rc1"), Some((0, 2, 0)));

        assert!(is_newer_version("0.1.0", "v0.2.0"));
        assert!(is_newer_version("v0.1.0", "v0.1.1"));
        assert!(is_newer_version("0.1.0", "v1.0.0"));
        assert!(is_newer_version("0.9.0", "v0.10.0"));

        assert!(!is_newer_version("0.1.0", "v0.1.0"));
        assert!(!is_newer_version("0.1.0", "v0.0.9"));
        assert!(!is_newer_version("v0.2.0", "v0.1.0"));
        assert!(!is_newer_version("invalid", "v0.1.0"));
        assert!(!is_newer_version("0.1.0", "invalid"));
    }

    #[test]
    fn test_apply_update_file_replacement() {
        let temp_dir = env::temp_dir().join("fk_kuro_launcher_test_update");
        let _ = fs::create_dir_all(&temp_dir);

        let current_exe = temp_dir.join("test_launcher.exe");
        let new_exe = PathBuf::from(format!("{}.new", current_exe.display()));
        let old_exe = PathBuf::from(format!("{}.old", current_exe.display()));

        // Create initial current_exe and new_exe
        fs::write(&current_exe, b"old_version_content").unwrap();
        fs::write(&new_exe, b"new_version_content").unwrap();

        // Also simulate leftover .old file from a previous update
        fs::write(&old_exe, b"ancient_version_content").unwrap();

        // Perform file replacement
        let res = apply_update(&current_exe, &new_exe);
        assert!(res.is_ok(), "apply_update should succeed");

        // Verify replaced file content
        let updated_content = fs::read_to_string(&current_exe).unwrap();
        assert_eq!(updated_content, "new_version_content");

        // Verify .new file is gone
        assert!(!new_exe.exists(), ".new file should be removed after rename");

        // Cleanup temp dir
        let _ = fs::remove_dir_all(&temp_dir);
    }
    #[test]
    fn test_resolve_paths_wuthering_waves_nonexistent_shipping_exe() {
        let input = r"D:\SteamLibrary\steamapps\common\Wuthering Waves\Wuthering Waves.exe";
        let (game_exe, log_path, game_args) = resolve_paths(Some(input));
        // Since shipping_exe candidates do not exist and input_str does not exist on disk, fallback to candidate 1
        let expected_candidate_1 = PathBuf::from(r"D:\SteamLibrary\steamapps\common\Wuthering Waves\Client\Binaries\Win64\Client-Win64-Shipping.exe")
            .to_string_lossy()
            .to_string();
        assert_eq!(game_exe, expected_candidate_1);
        let expected_log_path = PathBuf::from(r"D:\SteamLibrary\steamapps\common\Wuthering Waves\Client\Saved\Logs\Client.log")
            .to_string_lossy()
            .to_string();
        assert_eq!(log_path, expected_log_path);
        assert!(game_args.is_empty());
    }

    #[test]
    fn test_resolve_paths_shipping_exe_direct() {
        let input = r"E:\SteamLibrary\steamapps\common\Wuthering Waves\Client\Binaries\Win64\Client-Win64-Shipping.exe";
        let (game_exe, log_path, game_args) = resolve_paths(Some(input));
        let expected_game_exe = input.to_string();
        let expected_log_path = PathBuf::from(r"E:\SteamLibrary\steamapps\common\Wuthering Waves\Client\Saved\Logs\Client.log")
            .to_string_lossy()
            .to_string();
        assert_eq!(game_exe, expected_game_exe);
        assert_eq!(log_path, expected_log_path);
        assert!(game_args.is_empty());
    }

    #[test]
    fn test_resolve_paths_standalone_none() {
        let (game_exe, log_path, game_args) = resolve_paths(None);
        assert!(game_exe.to_lowercase().contains("client-win64-shipping.exe"));
        assert!(log_path.to_lowercase().contains("client.log"));
        assert!(game_args.is_empty());
    }

    #[test]
    fn test_resolve_paths_with_existing_files() {
        let temp_dir = env::temp_dir().join("fk_kuro_launcher_test_paths_existing");
        let client_dir = temp_dir.join("Client");
        let win64_dir = client_dir.join("Binaries").join("Win64");
        let logs_dir = client_dir.join("Saved").join("Logs");
        let _ = fs::create_dir_all(&win64_dir);
        let _ = fs::create_dir_all(&logs_dir);

        let shipping_exe = win64_dir.join("Client-Win64-Shipping.exe");
        let log_file = logs_dir.join("Client.log");
        let _ = fs::write(&shipping_exe, b"fake_exe");
        let _ = fs::write(&log_file, b"fake_log");

        let shim_exe = temp_dir.join("Wuthering Waves.exe");
        let shim_str = shim_exe.to_str().unwrap();

        let (game_exe, log_path, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(game_exe), shipping_exe);
        assert_eq!(PathBuf::from(log_path), log_file);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_split_steam_command_args_reassembles_unquoted_path() {
        let args = [
            r"C:\Program",
            r"Files",
            r"(x86)\Steam\steamapps\common\Wuthering",
            r"Waves\Wuthering",
            r"Waves.exe",
            "-dx11",
        ];

        let (game_exe, forwarded_args) = split_steam_command_args_for_test(&args);

        assert_eq!(
            game_exe,
            Some(
                r"C:\Program Files (x86)\Steam\steamapps\common\Wuthering Waves\Wuthering Waves.exe"
                    .to_string()
            )
        );
        assert_eq!(forwarded_args, vec!["-dx11"]);
    }

    #[test]
    fn test_split_steam_command_args_preserves_quoted_path_and_args() {
        let temp_dir = env::temp_dir().join("fk_kuro_launcher_test_steam_args");
        let game_exe = temp_dir.join("Wuthering Waves.exe");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::write(&game_exe, b"fake_exe").unwrap();

        let game_exe_string = game_exe.to_string_lossy().to_string();
        let args = [game_exe_string.as_str(), "-some-game-flag"];
        let (resolved_exe, forwarded_args) = split_steam_command_args_for_test(&args);

        assert_eq!(resolved_exe, Some(game_exe_string));
        assert_eq!(forwarded_args, vec!["-some-game-flag"]);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_persistent_logging() {
        let test_msg = "test_persistent_logging_entry_999";
        log_stdout(test_msg);

        let log_file = get_launcher_log_path();
        assert!(log_file.exists(), "launcher.log should exist");

        let contents = fs::read_to_string(&log_file).unwrap();
        assert!(contents.contains(test_msg), "launcher.log should contain log entry");
    }

    #[test]
    fn test_spawn_game_process_nonexistent() {
        let res = spawn_game_process("non_existent_game_exe_12345.exe", &[]);
        assert!(res.is_err(), "Spawning a non-existent binary should return Err");
    }

    #[test]
    fn test_resolve_paths_fallback_client_log() {
        let temp_dir = env::temp_dir().join("fk_kuro_launcher_test_paths_fallback");
        let _ = fs::create_dir_all(&temp_dir);

        // Create parent level Client.log fallback instead of Client/Saved/Logs/Client.log
        let fallback_log = temp_dir.join("Client.log");
        let _ = fs::write(&fallback_log, b"fake_fallback_log");

        let shim_exe = temp_dir.join("Wuthering Waves.exe");
        let shim_str = shim_exe.to_str().unwrap();

        let (_, log_path, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(log_path), fallback_log);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_get_game_root_dir() {
        let p1 = Path::new(r"C:\Games\Wuthering Waves\Client\Binaries\Win64\Client-Win64-Shipping.exe");
        assert_eq!(get_game_root_dir(p1), PathBuf::from(r"C:\Games\Wuthering Waves"));

        let p2 = Path::new(r"C:/Games/Wuthering Waves/Client/Binaries/Win64/Client-Win64-Shipping.exe");
        assert_eq!(get_game_root_dir(p2), PathBuf::from(r"C:/Games/Wuthering Waves"));

        let p3 = Path::new(r"D:\Games\Wuthering Waves\Wuthering Waves.exe");
        assert_eq!(get_game_root_dir(p3), PathBuf::from(r"D:\Games\Wuthering Waves"));

        let p4 = Path::new(r"Client\Binaries\Win64\Client-Win64-Shipping.exe");
        assert_eq!(get_game_root_dir(p4), PathBuf::from("."));

        let p5 = Path::new("Wuthering Waves.exe");
        assert_eq!(get_game_root_dir(p5), PathBuf::from("."));
    }

    #[test]
    fn test_multi_candidate_shipping_exe_resolution() {
        let temp_dir = env::temp_dir().join("fk_kuro_launcher_test_multi_cand_exe");
        let shim_exe = temp_dir.join("Wuthering Waves.exe");
        let shim_str = shim_exe.to_str().unwrap();

        // Candidate 1: Client/Binaries/Win64/Client-Win64-Shipping.exe
        let cand1 = temp_dir.join("Client").join("Binaries").join("Win64").join("Client-Win64-Shipping.exe");
        // Candidate 2: Wuthering Waves Game/Client/Binaries/Win64/Client-Win64-Shipping.exe
        let cand2 = temp_dir.join("Wuthering Waves Game").join("Client").join("Binaries").join("Win64").join("Client-Win64-Shipping.exe");
        // Candidate 3: Wuthering Waves/Client/Binaries/Win64/Client-Win64-Shipping.exe
        let cand3 = temp_dir.join("Wuthering Waves").join("Client").join("Binaries").join("Win64").join("Client-Win64-Shipping.exe");
        // Candidate 4: Binaries/Win64/Client-Win64-Shipping.exe
        let cand4 = temp_dir.join("Binaries").join("Win64").join("Client-Win64-Shipping.exe");

        // Test Candidate 4
        fs::create_dir_all(cand4.parent().unwrap()).unwrap();
        fs::write(&cand4, b"cand4").unwrap();
        let (resolved_exe, _, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_exe), cand4);

        // Test Candidate 3 (overrides Candidate 4)
        fs::create_dir_all(cand3.parent().unwrap()).unwrap();
        fs::write(&cand3, b"cand3").unwrap();
        let (resolved_exe, _, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_exe), cand3);

        // Test Candidate 2 (overrides Candidate 3)
        fs::create_dir_all(cand2.parent().unwrap()).unwrap();
        fs::write(&cand2, b"cand2").unwrap();
        let (resolved_exe, _, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_exe), cand2);

        // Test Candidate 1 (overrides Candidate 2)
        fs::create_dir_all(cand1.parent().unwrap()).unwrap();
        fs::write(&cand1, b"cand1").unwrap();
        let (resolved_exe, _, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_exe), cand1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_multi_candidate_shipping_exe_input_str_fallback() {
        let temp_dir = env::temp_dir().join("fk_kuro_launcher_test_input_fallback");
        let _ = fs::create_dir_all(&temp_dir);

        let custom_exe = temp_dir.join("CustomGame.exe");
        fs::write(&custom_exe, b"custom_exe").unwrap();
        let custom_str = custom_exe.to_str().unwrap();

        // No candidate 1..4 exists, but input_str (custom_exe) exists on disk
        let (resolved_exe, _, _) = resolve_paths(Some(custom_str));
        assert_eq!(resolved_exe, custom_str);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_multi_candidate_log_resolution_order() {
        let temp_dir = env::temp_dir().join("fk_kuro_launcher_test_multi_cand_log");
        let shim_exe = temp_dir.join("Wuthering Waves.exe");
        let shim_str = shim_exe.to_str().unwrap();

        let log1 = temp_dir.join("Client").join("Saved").join("Logs").join("Client.log");
        let log2 = temp_dir.join("Wuthering Waves Game").join("Client").join("Saved").join("Logs").join("Client.log");
        let log3 = temp_dir.join("Wuthering Waves").join("Client").join("Saved").join("Logs").join("Client.log");
        let log4 = temp_dir.join("Saved").join("Logs").join("Client.log");
        let log5 = temp_dir.join("Client.log");

        // Test candidate 5
        fs::create_dir_all(log5.parent().unwrap()).unwrap();
        fs::write(&log5, b"log5").unwrap();
        let (_, resolved_log, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_log), log5);

        // Test candidate 4
        fs::create_dir_all(log4.parent().unwrap()).unwrap();
        fs::write(&log4, b"log4").unwrap();
        let (_, resolved_log, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_log), log4);

        // Test candidate 3
        fs::create_dir_all(log3.parent().unwrap()).unwrap();
        fs::write(&log3, b"log3").unwrap();
        let (_, resolved_log, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_log), log3);

        // Test candidate 2
        fs::create_dir_all(log2.parent().unwrap()).unwrap();
        fs::write(&log2, b"log2").unwrap();
        let (_, resolved_log, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_log), log2);

        // Test candidate 1
        fs::create_dir_all(log1.parent().unwrap()).unwrap();
        fs::write(&log1, b"log1").unwrap();
        let (_, resolved_log, _) = resolve_paths(Some(shim_str));
        assert_eq!(PathBuf::from(resolved_log), log1);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
