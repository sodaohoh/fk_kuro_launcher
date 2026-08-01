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

/// Generate a 16×16 solid-color RGBA icon (blue square).
fn make_default_icon_rgba() -> (Vec<u8>, u32, u32) {
    const W: u32 = 16;
    const H: u32 = 16;
    let pixel: [u8; 4] = [0x40, 0x80, 0xFF, 0xFF]; // RGBA blue
    let data: Vec<u8> = pixel.iter().copied().cycle().take((W * H * 4) as usize).collect();
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
