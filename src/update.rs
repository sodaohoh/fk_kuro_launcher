#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::logging::{get_appdata_dir, log_stderr, log_stdout};

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

/// Build the content of the PowerShell update handoff script.
pub(crate) fn build_update_handoff_script() -> String {
    r#"$targetPid = $args[0]
$exePath = $args[1]
$newPath = $args[2]
$oldPath = "$exePath.old"
# Wait for the launcher to exit
try { (Get-Process -Id $targetPid -ErrorAction Stop).WaitForExit(30000) } catch {}
Start-Sleep -Seconds 1
# Perform the swap
if (Test-Path $oldPath) { Remove-Item $oldPath -Force -ErrorAction SilentlyContinue }
Rename-Item $exePath $oldPath -Force -ErrorAction Stop
Rename-Item $newPath $exePath -Force -ErrorAction Stop
Remove-Item $oldPath -Force -ErrorAction SilentlyContinue
"#
    .to_string()
}

pub(crate) fn check_latest_release(current_version: &str) {
    // Guard: only auto-update when running from the canonical installed path.
    let appdata_dir = get_appdata_dir();
    let canonical_install = appdata_dir.join("fk_kuro_launcher.exe");
    let current_exe = match env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            log_stderr("[ERROR] Auto-update failed: could not determine executable path.");
            return;
        }
    };

    // Case-insensitive comparison (Windows paths)
    if current_exe
        .to_string_lossy()
        .to_lowercase()
        != canonical_install.to_string_lossy().to_lowercase()
    {
        log_stdout("[INFO] Running from non-installed path; skipping auto-update.");
        return;
    }

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
                    "[INFO] New version detected ({}). Downloading update...",
                    latest_tag
                ));

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
                        // Validate downloaded file has a reasonable size
                        let file_ok = fs::metadata(&new_exe)
                            .map(|m| m.len() > 1000)
                            .unwrap_or(false);
                        if !file_ok {
                            log_stderr("[ERROR] Downloaded update file is too small or unreadable.");
                            let _ = fs::remove_file(&new_exe);
                            return;
                        }

                        // Write handoff script
                        let handoff_script = appdata_dir.join("update.ps1");
                        if let Err(e) = fs::write(&handoff_script, build_update_handoff_script()) {
                            log_stderr(&format!(
                                "[ERROR] Failed to write update handoff script: {}",
                                e
                            ));
                            let _ = fs::remove_file(&new_exe);
                            return;
                        }

                        // Launch the handoff script (fire-and-forget)
                        let spawn_result = Command::new("powershell")
                            .creation_flags(CREATE_NO_WINDOW)
                            .args([
                                "-NoProfile",
                                "-ExecutionPolicy",
                                "Bypass",
                                "-File",
                                handoff_script.to_str().unwrap_or_default(),
                                &std::process::id().to_string(),
                                current_exe.to_str().unwrap_or_default(),
                                new_exe.to_str().unwrap_or_default(),
                            ])
                            .spawn();

                        match spawn_result {
                            Ok(_) => {
                                log_stdout(&format!(
                                    "[INFO] Update {} downloaded. Will be applied when the launcher exits.",
                                    latest_tag
                                ));
                            }
                            Err(e) => {
                                log_stderr(&format!(
                                    "[ERROR] Failed to launch update handoff script: {}",
                                    e
                                ));
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
            }
        }
    }
}
