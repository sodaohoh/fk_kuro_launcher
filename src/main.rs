#![cfg_attr(windows, windows_subsystem = "windows")]

mod log_decoder;
mod logging;
mod paths;
mod process;
mod single_instance;
mod tray;
mod update;

use log_decoder::{build_lut, decode_bytes, update_restart_marker_tail};
use logging::{log_stderr, log_stdout};
use paths::{resolve_paths, split_steam_command_args};
use process::{handle_spawn_error, spawn_game_process};
use single_instance::acquire_single_instance;
use tray::{TrayCommand, TrayStatus};
use update::check_latest_release;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    // Clean up leftover .old file from a previous update
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

    // Single-instance guard — must be held until main() returns.
    let _instance_guard = match acquire_single_instance() {
        Some(guard) => guard,
        None => return,
    };

    // Create channels between tray thread and worker
    let (status_tx, status_rx) = mpsc::channel::<TrayStatus>();
    let (cmd_tx, cmd_rx) = mpsc::channel::<TrayCommand>();

    // Spawn the tray icon thread (owns the Win32 message loop)
    let tray_handle = thread::spawn(move || {
        tray::run_tray(status_rx, cmd_tx);
    });

    // Run the wrapper logic on the main thread
    run_wrapper(status_tx, cmd_rx);

    // Wait for the tray thread to clean up
    let _ = tray_handle.join();
}

const HOTFIX_RESTART_INTENT_TIMEOUT: Duration = Duration::from_secs(15);

fn drain_log(
    log_path: &str,
    offset: &mut u64,
    log_tail: &mut String,
    lut: &[Option<char>; 256],
    pending_hotfix_restart: &mut Option<Instant>,
) {
    if let Ok(mut file) = OpenOptions::new()
        .read(true)
        .share_mode(7)
        .open(log_path)
    {
        if let Ok(metadata) = file.metadata() {
            let file_len = metadata.len();
            if file_len < *offset {
                *offset = 0;
                log_tail.clear();
                *pending_hotfix_restart = None;
            }

            if file_len > *offset && file.seek(SeekFrom::Start(*offset)).is_ok() {
                let read_size = (file_len - *offset) as usize;
                let mut buffer = vec![0u8; read_size];
                if let Ok(n) = file.read(&mut buffer) {
                    if n > 0 {
                        *offset += n as u64;
                        let decoded_text = decode_bytes(&buffer[..n], lut);
                        if update_restart_marker_tail(log_tail, &decoded_text) {
                            let intent_is_active = pending_hotfix_restart.is_some_and(|observed| {
                                observed.elapsed() <= HOTFIX_RESTART_INTENT_TIMEOUT
                            });
                            if !intent_is_active {
                                log_stdout("[WARN] Hotfix restart requested by engine!");
                                *pending_hotfix_restart = Some(Instant::now());
                            }
                        }
                    }
                }
            }
        }
    }
}

fn run_wrapper(status_tx: mpsc::Sender<TrayStatus>, cmd_rx: mpsc::Receiver<TrayCommand>) {
    let _ = status_tx.send(TrayStatus::Starting);

    log_stdout(&format!(
        "[INFO] Version: v{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH")
    ));

    thread::spawn(|| {
        check_latest_release(env!("CARGO_PKG_VERSION"));
    });

    let args: Vec<String> = env::args().collect();
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
            let pid = child.id();
            log_stdout(&format!("[INFO] Game spawned with PID: {}", pid));
            let _ = status_tx.send(TrayStatus::Running { pid });
            child
        }
        Err(e) => {
            let msg = format!("{}", e);
            let _ = status_tx.send(TrayStatus::Failed { message: msg });
            handle_spawn_error(&game_exe, &e);
            let _ = status_tx.send(TrayStatus::Exiting);
            return;
        }
    };

    let mut pending_hotfix_restart: Option<Instant> = None;
    let mut log_tail = String::new();

    loop {
        // Check for tray exit command
        if let Ok(TrayCommand::Exit) = cmd_rx.try_recv() {
            log_stdout("[INFO] User requested exit from tray.");
            break;
        }

        // Drain the log before checking whether the child has exited.
        drain_log(
            &log_path,
            &mut offset,
            &mut log_tail,
            &lut,
            &mut pending_hotfix_restart,
        );

        if pending_hotfix_restart.is_some_and(|observed| {
            observed.elapsed() > HOTFIX_RESTART_INTENT_TIMEOUT
        }) {
            log_stdout("[INFO] Hotfix restart intent expired while game remained running.");
            pending_hotfix_restart = None;
        }

        let child_status = match game_child.try_wait() {
            Ok(status) => status,
            Err(e) => {
                log_stderr(&format!("[ERROR] Error waiting on child process: {}", e));
                let _ = status_tx.send(TrayStatus::Failed {
                    message: format!("Wait error: {}", e),
                });
                break;
            }
        };

        match child_status {
            Some(status) => {
                log_stdout(&format!("[INFO] Child game process exited with status: {}", status));
                // Perform a final log drain AFTER child termination to catch last-millisecond restart markers
                drain_log(
                    &log_path,
                    &mut offset,
                    &mut log_tail,
                    &lut,
                    &mut pending_hotfix_restart,
                );

                let hotfix_restart_requested = pending_hotfix_restart.is_some_and(|observed| {
                    observed.elapsed() <= HOTFIX_RESTART_INTENT_TIMEOUT
                });
                if hotfix_restart_requested {
                    log_stdout("[WARN] Hotfix restart detected! Respawning child process in 3s...");
                    let _ = status_tx.send(TrayStatus::HotfixRestart);
                    thread::sleep(Duration::from_secs(3));
                    match spawn_game_process(&game_exe, &game_args) {
                        Ok(child) => {
                            let pid = child.id();
                            log_stdout(&format!("[SUCCESS] Game respawned with PID: {}", pid));
                            let _ = status_tx.send(TrayStatus::Running { pid });
                            game_child = child;
                            pending_hotfix_restart = None;
                            log_tail.clear();
                            if let Ok(meta) = fs::metadata(&log_path) {
                                offset = meta.len();
                            }
                            thread::sleep(Duration::from_secs(5));
                            continue;
                        }
                        Err(e) => {
                            let msg = format!("{}", e);
                            let _ = status_tx.send(TrayStatus::Failed { message: msg });
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

    let _ = status_tx.send(TrayStatus::Exiting);
}

