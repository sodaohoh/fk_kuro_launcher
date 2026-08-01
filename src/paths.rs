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
