#![cfg_attr(windows, windows_subsystem = "windows")]

mod log_decoder;
mod logging;
mod paths;
mod process;
mod update;

use log_decoder::{build_lut, decode_bytes, update_restart_marker_tail};
use logging::{log_stderr, log_stdout};
use paths::{resolve_paths, split_steam_command_args};
use process::{handle_spawn_error, spawn_game_process};
use update::check_latest_release;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[cfg(test)]
use logging::get_launcher_log_path;
#[cfg(test)]
use paths::{get_game_root_dir, split_steam_command_args_for_test};
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use update::{apply_update, is_newer_version, parse_version};

fn main() {
    if let Ok(current_exe) = env::current_exe() {
        let old_exe = PathBuf::from(format!("{}.old", current_exe.display()));
        if old_exe.exists() {
            let _ = fs::remove_file(&old_exe);
        }
    }

    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-v") {
        log_stdout(&format!(
            "fk_kuro_launcher v{} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("BUILD_GIT_HASH")
        ));
        return;
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
    let mut log_tail = String::new();

    loop {
        // Drain the log before checking whether the child has exited. The game
        // can write its restart marker and exit between two polling iterations.

        // Monitor Client.log for hotfix restart triggers.
        if let Ok(mut file) = OpenOptions::new()
            .read(true)
            .share_mode(7)
            .open(&log_path)
        {
            if let Ok(metadata) = file.metadata() {
                let file_len = metadata.len();
                if file_len < offset {
                    offset = 0;
                    log_tail.clear();
                }

                if file_len > offset && file.seek(SeekFrom::Start(offset)).is_ok() {
                    let read_size = (file_len - offset) as usize;
                    let mut buffer = vec![0u8; read_size];
                    if let Ok(n) = file.read(&mut buffer) {
                        if n > 0 {
                            offset += n as u64;
                            let decoded_text = decode_bytes(&buffer[..n], &lut);
                            if update_restart_marker_tail(&mut log_tail, &decoded_text) {
                                log_stdout("[WARN] Hotfix restart requested by engine!");
                                is_hotfix_restart = true;
                            }
                        }
                    }
                }
            }
        }

        let child_status = match game_child.try_wait() {
            Ok(status) => status,
            Err(e) => {
                log_stderr(&format!("[ERROR] Error waiting on child process: {}", e));
                break;
            }
        };

        match child_status {
            Some(status) => {
                log_stdout(&format!("[INFO] Child game process exited with status: {}", status));
                if is_hotfix_restart {
                    log_stdout("[WARN] Hotfix restart detected! Respawning child process in 3s...");
                    thread::sleep(Duration::from_secs(3));
                    match spawn_game_process(&game_exe, &game_args) {
                        Ok(child) => {
                            log_stdout(&format!("[SUCCESS] Game respawned with PID: {}", child.id()));
                            game_child = child;
                            is_hotfix_restart = false;
                            log_tail.clear();
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
            None => {
                // Child is still running.
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
    fn test_restart_marker_detected_across_log_reads() {
        let mut tail = String::new();

        assert!(!update_restart_marker_tail(&mut tail, "Engine exit req"));
        assert_eq!(tail, "Engine exit req");
        assert!(update_restart_marker_tail(&mut tail, "uested"));
        assert!(tail.is_empty());

        assert!(!update_restart_marker_tail(&mut tail, "NeedRes"));
        assert!(update_restart_marker_tail(&mut tail, "tart"));
    }

    #[test]
    fn test_restart_marker_tail_is_bounded_without_marker() {
        let mut tail = String::new();
        let long_text = "x".repeat(256);

        assert!(!update_restart_marker_tail(&mut tail, &long_text));
        assert_eq!(
            tail.chars().count(),
            "Engine exit requested".chars().count() - 1
        );
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
