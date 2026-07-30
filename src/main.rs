use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

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

fn main() {
    let args: Vec<String> = env::args().collect();

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
            .share_access(7)
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
