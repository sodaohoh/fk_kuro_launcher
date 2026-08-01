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
#[link(name = "kernel32")]
extern "system" {
    fn GetProcessId(hProcess: *mut std::ffi::c_void) -> u32;
    fn GetExitCodeProcess(hProcess: *mut std::ffi::c_void, lpExitCode: *mut u32) -> i32;
    fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
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
    if res == 0 || info.hProcess.is_null() {
        return Err(io::Error::last_os_error());
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
            if e.raw_os_error() == Some(740) {
                spawn_via_shellexecute(game_exe_path, game_args, &game_root_dir)
            } else {
                Err(e)
            }
        }
    }
}

fn handle_spawn_error(game_exe: &str, e: &io::Error) {
    let err_msg = format!("[ERROR] Failed to spawn game process {}: {}", game_exe, e);
    eprintln!("{}", err_msg);
    let _ = fs::write("fk_kuro_launcher_error.log", &err_msg);
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
                println!(
                    "[INFO] New version detected ({}). Performing automatic update...",
                    latest_tag
                );

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
                                    println!(
                                        "[SUCCESS] Auto-update applied successfully! Version {} will run on next launch.",
                                        latest_tag
                                    );
                                }
                                Err(e) => {
                                    println!("[ERROR] Auto-update failed to replace executable: {}", e);
                                    let _ = fs::remove_file(&new_exe);
                                }
                            }
                        }
                        Ok(_) | Err(_) => {
                            println!("[ERROR] Auto-update failed during download.");
                            if new_exe.exists() {
                                let _ = fs::remove_file(&new_exe);
                            }
                        }
                    }
                } else {
                    println!("[ERROR] Auto-update failed: could not determine executable path.");
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

fn get_client_dir(path: &Path) -> PathBuf {
    if let Some(p1) = path.parent() {
        if let Some(p2) = p1.parent() {
            if let Some(p3) = p2.parent() {
                return p3.to_path_buf();
            }
        }
        return p1.to_path_buf();
    }
    PathBuf::from(".")
}

fn resolve_log_path(primary_log: PathBuf, parent_dir: &Path) -> String {
    let candidate1 = primary_log;
    let candidate2 = parent_dir
        .join("Client")
        .join("Saved")
        .join("Logs")
        .join("Client.log");
    let candidate3 = parent_dir.join("Client.log");

    if candidate1.exists() {
        candidate1.to_string_lossy().to_string()
    } else if candidate2.exists() {
        candidate2.to_string_lossy().to_string()
    } else if candidate3.exists() {
        candidate3.to_string_lossy().to_string()
    } else {
        candidate1.to_string_lossy().to_string()
    }
}

pub fn resolve_paths(input_exe: Option<&str>) -> (String, String, Vec<String>) {
    if let Some(input_str) = input_exe {
        let path = PathBuf::from(input_str);
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let is_shipping_exe = file_name.eq_ignore_ascii_case("Client-Win64-Shipping.exe")
            || path
                .to_string_lossy()
                .to_lowercase()
                .contains("client-win64-shipping.exe");

        if is_shipping_exe {
            let game_exe = input_str.to_string();
            let client_dir = get_client_dir(&path);
            let primary_log = client_dir.join("Saved").join("Logs").join("Client.log");
            let root_dir = client_dir.parent().unwrap_or(&client_dir);
            let log_path = resolve_log_path(primary_log, root_dir);
            (game_exe, log_path, vec![])
        } else {
            let parent = path.parent().unwrap_or(Path::new("."));
            let shipping_exe = parent
                .join("Client")
                .join("Binaries")
                .join("Win64")
                .join("Client-Win64-Shipping.exe");
            let primary_log = parent
                .join("Client")
                .join("Saved")
                .join("Logs")
                .join("Client.log");

            let game_exe = if shipping_exe.exists() {
                shipping_exe.to_string_lossy().to_string()
            } else if file_name.eq_ignore_ascii_case("Wuthering Waves.exe")
                || file_name.to_lowercase().contains("wuthering waves")
            {
                shipping_exe.to_string_lossy().to_string()
            } else {
                input_str.to_string()
            };

            let log_path = resolve_log_path(primary_log, parent);
            (game_exe, log_path, vec![])
        }
    } else {
        let base_dir = if let Some(steam_path) = get_steam_path_from_registry() {
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

        let shipping_exe = base_dir
            .join("Client")
            .join("Binaries")
            .join("Win64")
            .join("Client-Win64-Shipping.exe");
        let primary_log = base_dir
            .join("Client")
            .join("Saved")
            .join("Logs")
            .join("Client.log");

        let log_path = resolve_log_path(primary_log, &base_dir);
        (
            shipping_exe.to_string_lossy().to_string(),
            log_path,
            vec![],
        )
    }
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

    println!(
        "[INFO] Version: v{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH")
    );

    thread::spawn(|| {
        check_latest_release(env!("CARGO_PKG_VERSION"));
    });

    let input_exe = args.get(1).map(|s| s.as_str());
    let (game_exe, log_path, mut game_args) = resolve_paths(input_exe);
    if args.len() > 2 {
        game_args.extend(args[2..].to_vec());
    }

    println!("[INFO] Steam Wrapper Monitor Started.");
    println!("[INFO] Executable Target: {}", game_exe);
    println!("[INFO] Log Target: {}", log_path);

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
            println!("[INFO] Game spawned with PID: {}", child.id());
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
                println!("[INFO] Child game process exited with status: {}", status);
                if is_hotfix_restart {
                    println!("[WARN] Hotfix restart detected! Respawning child process in 3s...");
                    thread::sleep(Duration::from_secs(3));
                    match spawn_game_process(&game_exe, &game_args) {
                        Ok(child) => {
                            println!("[SUCCESS] Game respawned with PID: {}", child.id());
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
                    println!("[INFO] Normal game exit detected. Steam Wrapper shutting down.");
                    break;
                }
            }
            Ok(None) => {
                // Child is still running
            }
            Err(e) => {
                println!("[ERROR] Error waiting on child process: {}", e);
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
                                    println!("[WARN] Hotfix restart requested by engine!");
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
    fn test_resolve_paths_wuthering_waves_shim() {
        let input = r"D:\SteamLibrary\steamapps\common\Wuthering Waves\Wuthering Waves.exe";
        let (game_exe, log_path, game_args) = resolve_paths(Some(input));
        let expected_game_exe = PathBuf::from(r"D:\SteamLibrary\steamapps\common\Wuthering Waves\Client\Binaries\Win64\Client-Win64-Shipping.exe")
            .to_string_lossy()
            .to_string();
        let expected_log_path = PathBuf::from(r"D:\SteamLibrary\steamapps\common\Wuthering Waves\Client\Saved\Logs\Client.log")
            .to_string_lossy()
            .to_string();
        assert_eq!(game_exe, expected_game_exe);
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
}
