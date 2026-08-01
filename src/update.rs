#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::logging::{log_stderr, log_stdout};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let mut parts = s.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let patch_str = parts.next()?;
    let patch_num_str: String = patch_str
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let patch: u64 = patch_num_str.parse().ok()?;
    Some((major, minor, patch))
}

pub(crate) fn is_newer_version(current: &str, latest: &str) -> bool {
    if let (Some(cur), Some(lat)) = (parse_version(current), parse_version(latest)) {
        lat > cur
    } else {
        false
    }
}

pub(crate) fn apply_update(current_exe: &Path, new_exe: &Path) -> io::Result<()> {
    let old_exe = PathBuf::from(format!("{}.old", current_exe.display()));
    if old_exe.exists() {
        let _ = fs::remove_file(&old_exe);
    }
    fs::rename(current_exe, &old_exe)?;
    if let Err(e) = fs::rename(new_exe, current_exe) {
        let _ = fs::rename(&old_exe, current_exe);
        return Err(e);
    }
    let _ = fs::remove_file(&old_exe);
    Ok(())
}

pub(crate) fn check_latest_release(current_version: &str) {
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
                                Ok(()) => log_stdout(&format!(
                                    "[SUCCESS] Auto-update applied successfully! Version {} will run on next launch.",
                                    latest_tag
                                )),
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
