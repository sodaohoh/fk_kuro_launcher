use std::env;
use std::fs::{self, OpenOptions};
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
        use std::io::Write;
        let _ = writeln!(file, "{}", msg);
    }
}

pub(crate) fn log_stdout(msg: &str) {
    println!("{}", msg);
    write_to_launcher_log(msg);
}

pub(crate) fn log_stderr(msg: &str) {
    eprintln!("{}", msg);
    write_to_launcher_log(msg);
}
