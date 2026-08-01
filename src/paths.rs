use std::path::{Path, PathBuf};

pub(crate) fn get_game_root_dir(game_exe: &Path) -> PathBuf {
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
        parent
            .join("Client")
            .join("Binaries")
            .join("Win64")
            .join("Client-Win64-Shipping.exe"),
        parent
            .join("Wuthering Waves Game")
            .join("Client")
            .join("Binaries")
            .join("Win64")
            .join("Client-Win64-Shipping.exe"),
        parent
            .join("Wuthering Waves")
            .join("Client")
            .join("Binaries")
            .join("Win64")
            .join("Client-Win64-Shipping.exe"),
        parent
            .join("Binaries")
            .join("Win64")
            .join("Client-Win64-Shipping.exe"),
    ]
}

fn get_log_candidates(parent: &Path) -> Vec<PathBuf> {
    vec![
        parent
            .join("Client")
            .join("Saved")
            .join("Logs")
            .join("Client.log"),
        parent
            .join("Wuthering Waves Game")
            .join("Client")
            .join("Saved")
            .join("Logs")
            .join("Client.log"),
        parent
            .join("Wuthering Waves")
            .join("Client")
            .join("Saved")
            .join("Logs")
            .join("Client.log"),
        parent.join("Saved").join("Logs").join("Client.log"),
        parent.join("Client.log"),
    ]
}

pub(crate) fn resolve_paths(input_exe: Option<&str>) -> (String, String, Vec<String>) {
    let parent = if let Some(input_str) = input_exe {
        get_game_root_dir(Path::new(input_str))
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
        if Path::new(input_str).exists() {
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

pub(crate) fn split_steam_command_args(args: &[String]) -> (Option<String>, Vec<String>) {
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
        if Path::new(&command).is_file() || arg.to_ascii_lowercase().ends_with(".exe") {
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
pub(crate) fn split_steam_command_args_for_test(args: &[&str]) -> (Option<String>, Vec<String>) {
    split_steam_command_args(
        &args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

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
