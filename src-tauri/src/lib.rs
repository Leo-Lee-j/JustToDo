mod commands;
mod models;
mod storage;
mod taskbar;

use storage::{SingleInstance, Store};
use tauri::{AppHandle, Manager};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

// ---------- 窗口控制命令 ----------

#[tauri::command]
fn set_always_on_top(app: AppHandle, label: String, on: bool) {
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.set_always_on_top(on);
    }
}

#[tauri::command]
fn set_opacity(app: AppHandle, label: String, opacity: i32) {
    let o = (opacity.max(20).min(100)) as f64 / 100.0;
    if let Some(w) = app.get_webview_window(&label) {
        #[cfg(target_os = "windows")]
        {
            let _ = set_win_opacity(&w, o);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = w.set_opacity(o);
        }
    }
}

#[cfg(target_os = "windows")]
fn set_win_opacity(w: &tauri::webview::WebviewWindow, alpha: f64) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetLayeredWindowAttributes, LWA_ALPHA,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_LAYERED,
    };
    let hwnd = w.hwnd().map_err(|e| e.to_string())?;
    let hwnd = HWND(hwnd.0 as _);
    unsafe {
        let mut style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if style & (WS_EX_LAYERED.0 as i32) == 0 {
            style |= WS_EX_LAYERED.0 as i32;
            SetWindowLongW(hwnd, GWL_EXSTYLE, style);
        }
        let alpha = (255.0 * alpha) as u8;
        let _ = SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), alpha, LWA_ALPHA);
    }
    Ok(())
}

#[tauri::command]
fn position_taskbar(app: AppHandle, position: String) -> Result<bool, String> {
    let (w, h) = (320i32, 96i32);
    let (x, y) = taskbar::taskbar_anchor(&position, w, h);
    if let Some(win) = app.get_webview_window("taskbar") {
        let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        let _ = win.show();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn show_popup(app: AppHandle) {
    if let Some(p) = app.get_webview_window("popup") {
        if let Some(sticky) = app.get_webview_window("sticky") {
            if let Ok(pos) = sticky.outer_position() {
                let _ = p.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: pos.x,
                    y: pos.y,
                }));
            }
        }
        let _ = p.show();
        let _ = p.set_focus();
    }
}

#[tauri::command]
fn hide_window(app: AppHandle, label: String) {
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.hide();
    }
}

#[tauri::command]
fn list_system_fonts() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let script = "$OutputEncoding = [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false); Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts','HKCU:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts' | ForEach-Object { $_.PSObject.Properties | Where-Object { $_.Name -notmatch '^PS' } | ForEach-Object { ($_.Name -replace ' \\(TrueType\\)| \\(OpenType\\)| \\(All res\\)$','').Trim() } } | Sort-Object -Unique";
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
        {
            let mut fonts: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(String::from)
                .collect();
            fonts.sort_unstable();
            fonts.dedup();
            if !fonts.is_empty() { return fonts; }
        }
    }
    vec!["Arial".into(), "Segoe UI".into(), "sans-serif".into(), "serif".into(), "monospace".into()]
}

// ---------- 应用启动 ----------

fn diag(msg: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    let line = format!("[JTD] {}\n", msg);
    let _ = eprint!("{}", line);
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("G:\\office\\product\\JustToDo\\.omc\\jtd-run.log")
    {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

pub fn run() {
    diag("run() start");
    // 单实例检查
    if let None = SingleInstance::acquire() {
        // 已有实例：直接退出（理想情况下聚焦已有窗口，此处简化）
        eprintln!("JustToDo 已在运行，退出重复实例。");
        std::process::exit(0);
    }
    diag("single-instance acquired");

    let store = Store::new();
    diag("store created");
    commands::purge_expired_trash(&store);
    diag("trash purged");
    let cfg = store.config();
    diag("config loaded");

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(store)
        .setup(move |app| {
            diag("setup() start");
            // 应用窗口初始透明度
            if let Some(w) = app.get_webview_window("sticky") {
                diag("sticky window found");
                #[cfg(target_os = "windows")]
                {
                    let _ = set_win_opacity(&w, cfg.window.opacity as f64 / 100.0);
                }
            } else {
                diag("!! sticky window NOT found after launch");
            }
            // 钉住条定位
            // 托盘
            diag("building tray");
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("JustToDo")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("sticky") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;
            diag("setup() done, returning Ok");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_tasks,
            commands::get_tabs_and_categories,
            commands::get_config,
            commands::set_config,
            commands::create_task,
            commands::update_task,
            commands::reorder_tasks,
            commands::delete_task,
            commands::restore_task,
            commands::create_tab,
            commands::rename_tab,
            commands::delete_tab,
            commands::get_trashed_tasks,
            commands::get_task_history,
            commands::purge_task,
            set_always_on_top,
            set_opacity,
            position_taskbar,
            show_popup,
            hide_window,
            list_system_fonts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running JustToDo");
    diag("run() returning (event loop exited)");
}
