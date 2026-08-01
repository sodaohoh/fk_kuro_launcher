use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;

pub(crate) fn get_appdata_dir() -> PathBuf {
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

pub(crate) fn get_launcher_log_path() -> PathBuf {
    let dir = get_appdata_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join("launcher.log")
}

static LOG_MUTEX: Mutex<()> = Mutex::new(());

fn write_to_launcher_log(msg: &str) {
    let _guard = LOG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let log_path = get_launcher_log_path();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{}", msg);
    }
}

fn write_to_attached_console(msg: &str, is_error: bool) {
    if is_error {
        let _ = writeln!(io::stderr(), "{}", msg);
    } else {
        let _ = writeln!(io::stdout(), "{}", msg);
    }
}

pub(crate) fn log_stdout(msg: &str) {
    write_to_launcher_log(msg);
    write_to_attached_console(msg, false);
}

pub(crate) fn log_stderr(msg: &str) {
    write_to_launcher_log(msg);
    write_to_attached_console(msg, true);
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_persistent_logging() {
        let test_msg = "test_persistent_logging_entry_999";
        log_stdout(test_msg);

        let log_file = get_launcher_log_path();
        assert!(log_file.exists(), "launcher.log should exist");

        let contents = fs::read_to_string(&log_file).unwrap();
        assert!(contents.contains(test_msg), "launcher.log should contain log entry");
    }
}
