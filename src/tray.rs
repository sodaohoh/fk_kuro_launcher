use std::sync::mpsc;

use crate::logging::{get_appdata_dir, get_launcher_log_path, log_stderr, log_stdout};

/// Status transitions the worker thread sends to the tray thread.
pub(crate) enum TrayStatus {
    Starting,
    Running { pid: u32 },
    HotfixRestart,
    Failed { message: String },
    Exiting,
}

/// Commands the tray thread sends back to the worker thread.
pub(crate) enum TrayCommand {
    /// User clicked "Exit" — worker should stop monitoring (game keeps running).
    Exit,
}

// --- Win32 message pump FFI (Windows only) ---

#[cfg(target_os = "windows")]
#[repr(C)]
#[allow(non_snake_case)]
struct MSG {
    hwnd: *mut std::ffi::c_void,
    message: u32,
    wParam: usize,
    lParam: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn PeekMessageW(
        msg: *mut MSG,
        hwnd: *mut std::ffi::c_void,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
        wRemoveMsg: u32,
    ) -> i32;
    fn TranslateMessage(msg: *const MSG) -> i32;
    fn DispatchMessageW(msg: *const MSG) -> isize;
}

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut std::ffi::c_void,
        lpOperation: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShowCmd: i32,
    ) -> *mut std::ffi::c_void;
}

#[cfg(target_os = "windows")]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Generate a 16×16 high-quality RGBA icon representing the Kuro / Wuthering Waves launcher.
///
/// Features a dark slate navy rounded container (`#0F172A`),
/// a stylized gold "K" emblem (`#F59E0B`), and a glowing cyan Wuthering Wave crest (`#06B6D4`).
fn make_default_icon_rgba() -> (Vec<u8>, u32, u32) {
    const W: u32 = 16;
    const H: u32 = 16;

    // Color Palette [R, G, B, A]
    const TRANSPARENT: [u8; 4]  = [0, 0, 0, 0];
    const BORDER_OUTER: [u8; 4] = [51, 65, 85, 255];   // #334155 Slate border accent
    const BORDER_INNER: [u8; 4] = [30, 41, 59, 255];   // #1E293B Dark slate inner border
    const BG_NAVY: [u8; 4]      = [15, 23, 42, 255];   // #0F172A Navy background
    const BG_DARK: [u8; 4]      = [11, 17, 32, 255];   // #0B1120 Deep navy shadow

    // Gold "K" Emblem Palette
    const GOLD_WHITE: [u8; 4]   = [255, 253, 231, 255]; // #FFFDE7 White-gold highlight
    const GOLD_YELLOW: [u8; 4]  = [254, 240, 138, 255]; // #FEF08A Bright yellow-gold
    const GOLD_MAIN: [u8; 4]    = [245, 158, 11, 255];  // #F59E0B Main gold
    const GOLD_AMBER: [u8; 4]   = [217, 119, 6, 255];   // #D97706 Amber shadow

    // Cyan Wuthering Wave Motif Palette
    const CYAN_GLOW: [u8; 4]    = [207, 250, 254, 255]; // #CFFAFE Cyan sparkle highlight
    const CYAN_LIGHT: [u8; 4]   = [103, 232, 249, 255]; // #67E8F9 Glowing cyan
    const CYAN_MAIN: [u8; 4]    = [6, 182, 212, 255];   // #06B6D4 Main wave cyan
    const TEAL_MAIN: [u8; 4]    = [14, 116, 144, 255];  // #0E7490 Deep wave teal
    const TEAL_DARK: [u8; 4]    = [21, 94, 117, 255];   // #155E75 Dark teal shadow

    const ICON_MAP: [&str; 16] = [
        "..BBBBBBBBBBBB..",
        ".bkkkkkkkkkkkkb.",
        "BkkkkkkkkkkkkkkB",
        "BkkWYGkkkkkkGYkB",
        "BkkWYGkkkkkGYkkB",
        "BkkWYGkkkkGYkkkB",
        "BkkWYGkkkGYkkkkB",
        "BkkWYGGGGYkkkkkB",
        "BkkWYGGGGgkkkkkB",
        "BkkWYGkkGYgkkkkB",
        "BkkWYGkkkGcCckkB",
        "BkkWYGkkkScCcCkB",
        "BkkWYGkttcCcCtkB",
        "BkkkkdtttDDDDdkB",
        ".bkkkkkkkkkkkkb.",
        "..BBBBBBBBBBBB..",
    ];

    let mut data = Vec::with_capacity((W * H * 4) as usize);

    for row in ICON_MAP {
        for ch in row.bytes() {
            let px = match ch {
                b'B' => BORDER_OUTER,
                b'b' => BORDER_INNER,
                b'k' => BG_NAVY,
                b'd' => BG_DARK,
                b'W' => GOLD_WHITE,
                b'Y' => GOLD_YELLOW,
                b'G' => GOLD_MAIN,
                b'g' => GOLD_AMBER,
                b'S' => CYAN_GLOW,
                b'C' => CYAN_LIGHT,
                b'c' => CYAN_MAIN,
                b't' => TEAL_MAIN,
                b'D' => TEAL_DARK,
                _ => TRANSPARENT,
            };
            data.extend_from_slice(&px);
        }
    }

    (data, W, H)
}

/// Open a file or folder using the OS default handler.
#[cfg(target_os = "windows")]
fn shell_open(path: &std::path::Path) {
    let open = wide("open");
    let file = wide(&path.to_string_lossy());
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            open.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        );
    }
}

/// Run the tray icon event loop.
///
/// This function blocks until the worker sends `TrayStatus::Exiting` or the user
/// clicks "Exit" in the context menu.  It must be called on a thread that owns
/// (or will create) a Win32 message loop.
pub(crate) fn run_tray(
    status_rx: mpsc::Receiver<TrayStatus>,
    cmd_tx: mpsc::Sender<TrayCommand>,
) {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    // Build icon
    let (rgba, w, h) = make_default_icon_rgba();
    let icon = match Icon::from_rgba(rgba, w, h) {
        Ok(i) => i,
        Err(e) => {
            log_stderr(&format!("[ERROR] Failed to create tray icon image: {}", e));
            return;
        }
    };

    // Build menu
    let menu = Menu::new();
    let status_item = MenuItem::new("fk_kuro_launcher \u{2014} Starting\u{2026}", false, None);
    let open_log_item = MenuItem::new("Open launcher log", true, None);
    let open_folder_item = MenuItem::new("Open install folder", true, None);
    let exit_item = MenuItem::new("Exit (leave game running)", true, None);

    let _ = menu.append(&status_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&open_log_item);
    let _ = menu.append(&open_folder_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&exit_item);

    // Build tray icon
    let tray = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("fk_kuro_launcher")
        .with_icon(icon)
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            log_stderr(&format!("[ERROR] Failed to create tray icon: {}", e));
            return;
        }
    };

    log_stdout("[INFO] System tray icon created.");

    // Cache menu item IDs for event matching
    let open_log_id = open_log_item.id().clone();
    let open_folder_id = open_folder_item.id().clone();
    let exit_id = exit_item.id().clone();

    // Event loop
    loop {
        // 1. Pump Win32 messages so the tray icon responds to clicks
        #[cfg(target_os = "windows")]
        {
            let mut msg: MSG = unsafe { std::mem::zeroed() };
            while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 1) } != 0 {
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        // 2. Handle menu events
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == open_log_id {
                #[cfg(target_os = "windows")]
                shell_open(&get_launcher_log_path());
            } else if event.id == open_folder_id {
                #[cfg(target_os = "windows")]
                shell_open(&get_appdata_dir());
            } else if event.id == exit_id {
                log_stdout("[INFO] User requested exit from tray.");
                let _ = cmd_tx.send(TrayCommand::Exit);
                break;
            }
        }

        // 3. Handle status updates from the worker thread
        let mut should_exit = false;
        while let Ok(status) = status_rx.try_recv() {
            match status {
                TrayStatus::Starting => {
                    let _ = status_item.set_text("fk_kuro_launcher \u{2014} Starting\u{2026}");
                    let _ = tray.set_tooltip(Some("fk_kuro_launcher \u{2014} Starting\u{2026}"));
                }
                TrayStatus::Running { pid } => {
                    let text = format!("fk_kuro_launcher \u{2014} Running (PID {})", pid);
                    let _ = status_item.set_text(&text);
                    let _ = tray.set_tooltip(Some(&text));
                }
                TrayStatus::HotfixRestart => {
                    let _ = status_item.set_text("fk_kuro_launcher \u{2014} Restarting (hotfix)\u{2026}");
                    let _ = tray.set_tooltip(Some("fk_kuro_launcher \u{2014} Restarting (hotfix)\u{2026}"));
                }
                TrayStatus::Failed { ref message } => {
                    let text = format!("fk_kuro_launcher \u{2014} Error: {}", message);
                    let _ = status_item.set_text(&text);
                    let _ = tray.set_tooltip(Some(&text));
                }
                TrayStatus::Exiting => {
                    should_exit = true;
                }
            }
        }
        if should_exit {
            break;
        }

        // 4. Detect worker thread crash/unexpected exit (sender dropped without Exiting)
        use std::sync::mpsc::TryRecvError;
        match status_rx.try_recv() {
            Err(TryRecvError::Disconnected) => break,
            _ => {}
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Dropping `tray` removes the tray icon from the notification area.
    drop(tray);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_default_icon_rgba_dimensions_and_buffer() {
        let (rgba, w, h) = make_default_icon_rgba();
        assert_eq!(w, 16);
        assert_eq!(h, 16);
        assert_eq!(rgba.len(), (16 * 16 * 4) as usize);

        // Ensure alpha values are non-zero for non-transparent background pixels
        // Background center pixel (e.g. x=8, y=2) should be dark navy background (#0F172A, 255)
        let center_idx = (2 * 16 + 8) * 4;
        assert_eq!(&rgba[center_idx..center_idx + 4], &[15, 23, 42, 255]);

        // "K" Stem pixel (e.g. x=3, y=3) should be white-gold highlight (#FFFDE7, 255)
        let stem_idx = (3 * 16 + 3) * 4;
        assert_eq!(&rgba[stem_idx..stem_idx + 4], &[255, 253, 231, 255]);

        // Cyan wave pixel (e.g. x=11, y=11) should be glowing cyan (#67E8F9, 255)
        let wave_idx = (11 * 16 + 11) * 4;
        assert_eq!(&rgba[wave_idx..wave_idx + 4], &[103, 232, 249, 255]);
    }
}
