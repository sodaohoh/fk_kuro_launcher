use std::process::Command;

fn get_git_hash() -> String {
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !hash.is_empty() {
                return hash;
            }
        }
    }
    "release".to_string()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    let git_hash = get_git_hash();
    println!("cargo:rustc-env=BUILD_GIT_HASH={}", git_hash);

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set("FileDescription", "Wuthering Waves (Steam Wrapper) Hotfix Monitor");
        res.set("ProductName", "fk_kuro_launcher");
        res.set("LegalCopyright", "Copyright (c) 2026 sodaohoh");
        res.set("Copyright", "Copyright (c) 2026 sodaohoh");
        if let Err(e) = res.compile() {
            eprintln!("winres error: {}", e);
        }
    }
}
