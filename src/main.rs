use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
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

fn spawn_game_process(game_exe: &str, game_args: &[String]) -> io::Result<Child> {
    let game_dir = Path::new(game_exe).parent().unwrap_or(Path::new("."));
    Command::new(game_exe)
        .args(game_args)
        .current_dir(game_dir)
        .spawn()
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

    let default_log_path = r"C:\Program Files (x86)\Steam\steamapps\common\Wuthering Waves\Client\Saved\Logs\Client.log";
    let default_game_exe = r"C:\Program Files (x86)\Steam\steamapps\common\Wuthering Waves\Client\Binaries\Win64\Client-Win64-Shipping.exe";

    let (game_exe, game_args) = if args.len() > 1 {
        (args[1].clone(), args[2..].to_vec())
    } else {
        (default_game_exe.to_string(), vec![])
    };

    let log_path = default_log_path;

    println!("[INFO] Steam Wrapper Monitor Started.");
    println!("[INFO] Executable Target: {}", game_exe);
    println!("[INFO] Log Target: {}", log_path);

    let lut = build_lut();

    // Initialize offset to current end of file
    let mut offset: u64 = if let Ok(metadata) = fs::metadata(log_path) {
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
            println!("[ERROR] Failed to spawn game process: {}", e);
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
                            if let Ok(meta) = fs::metadata(log_path) {
                                offset = meta.len();
                            }
                            thread::sleep(Duration::from_secs(5));
                            continue;
                        }
                        Err(e) => {
                            println!("[ERROR] Failed to respawn game process: {}", e);
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
            .open(log_path)
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
}
