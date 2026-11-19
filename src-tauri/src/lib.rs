mod commands;
#[cfg(target_os = "windows")]
mod desktop;
mod models;
mod storage;
mod taskbar;

use std::sync::{Mutex, OnceLock};
use storage::{SingleInstance, Store};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

static NOTIFICATION_CHECK_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// The Windows toast backend accepts named system sounds (rather than file
/// paths); other desktop backends accept a sound-theme name.
pub(crate) fn notification_sound() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "IM"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "message-new-instant"
    }
}

fn due_within_window(
    end: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
    hours: i32,
) -> bool {
    end - now <= chrono::Duration::hours(hours.clamp(0, 168) as i64)
}

fn check_due_notifications(app: &AppHandle, store: &Store) -> Result<usize, String> {
    let _check_guard = NOTIFICATION_CHECK_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "notification lock poisoned".to_string())?;
    use chrono::{DateTime, Utc};
    use tauri_plugin_notification::NotificationExt;

    let config = store.config();
    if !config.notification.enabled {
        return Ok(0);
    }
    let now = Utc::now();
    let mut due = Vec::new();
    for task in &store.data().tasks {
        if task.deleted_at.is_some() || task.notified_at.is_some() {
            continue;
        }
        if !matches!(
            task.status,
            models::TaskStatus::Todo | models::TaskStatus::Doing
        ) {
            continue;
        }
        let Some(raw) = task.due_date.as_deref() else {
            continue;
        };
        let Ok(end) = DateTime::parse_from_rfc3339(raw) else {
            continue;
        };
        let end = end.with_timezone(&Utc);
        if due_within_window(end, now, config.notification.reminder_hours) {
            due.push((task.id.clone(), task.title.clone(), raw.to_string(), end));
        }
    }
    if due.is_empty() {
        return Ok(0);
    }

    let mut lines: Vec<String> = due
        .iter()
        .take(5)
        .map(|(_, title, _, end)| {
            let seconds = (*end - now).num_seconds();
            let remaining = if seconds <= 0 {
                "已到期".to_string()
            } else if seconds < 3600 {
                format!("剩余 {} 分钟", (seconds + 59) / 60)
            } else {
                format!("剩余 {} 小时", (seconds + 3599) / 3600)
            };
            format!("{} · {}", title, remaining)
        })
        .collect();
    if due.len() > 5 {
        lines.push(format!("还有 {} 个任务", due.len() - 5));
    }
    let mut builder = app
        .notification()
        .builder()
        .title("任务提醒")
        .body(lines.join("\n"));
    if config.notification.sound_enabled {
        builder = builder.sound(notification_sound());
    }
    builder.show().map_err(|e| e.to_string())?;

    let notified_at = models::now_utc();
    let count = due.len();
    store.mutate(|data| {
        for (id, _, date, _) in &due {
            if let Some(task) = data.tasks.iter_mut().find(|task| {
                task.id == *id && task.due_date.as_ref() == Some(date) && task.notified_at.is_none()
            }) {
                task.notified_at = Some(notified_at.clone());
            }
        }
        Ok(())
    })?;
    commands::emit_data_changed(app);
    Ok(count)
}

#[cfg(test)]
mod notification_tests {
    use super::due_within_window;
    use chrono::{Duration, Utc};

    #[test]
    fn zero_hours_only_matches_due_or_overdue() {
        let now = Utc::now();
        assert!(due_within_window(now, now, 0));
        assert!(due_within_window(now - Duration::minutes(1), now, 0));
        assert!(!due_within_window(now + Duration::minutes(1), now, 0));
    }

    #[test]
    fn reminder_window_matches_hours_and_clamps_bounds() {
        let now = Utc::now();
        assert!(due_within_window(now + Duration::hours(2), now, 2));
        assert!(!due_within_window(
            now + Duration::hours(2) + Duration::seconds(1),
            now,
            2
        ));
        assert!(due_within_window(now + Duration::hours(168), now, 999));
    }
}

#[tauri::command]
fn check_notifications(app: AppHandle, store: tauri::State<'_, Store>) -> Result<usize, String> {
    check_due_notifications(&app, &store)
}

// ---------- 窗口控制命令 ----------

#[tauri::command]
fn set_opacity(app: AppHandle, label: String, opacity: i32) {
    let o = f64::from(opacity.clamp(20, 100)) / 100.0;
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
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_LAYERED,
    };
    use windows::Win32::UI::WindowsAndMessaging::{SetLayeredWindowAttributes, LWA_ALPHA};
    let hwnd = w.hwnd().map_err(|e| e.to_string())?;
    let hwnd = HWND(hwnd.0 as _);
    unsafe {
        let mut style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if style & (WS_EX_LAYERED.0 as i32) == 0 {
            style |= WS_EX_LAYERED.0 as i32;
            SetWindowLongW(hwnd, GWL_EXSTYLE, style);
        }
        let alpha = (255.0 * alpha) as u8;
        let _ = SetLayeredWindowAttributes(
            hwnd,
            windows::Win32::Foundation::COLORREF(0),
            alpha,
            LWA_ALPHA,
        );
    }
    Ok(())
}

fn sync_taskbar_window(app: &AppHandle, store: &Store) -> Result<bool, String> {
    let config = store.config();
    let Some(win) = app.get_webview_window("taskbar") else {
        return Ok(false);
    };
    if !config.taskbar.enabled {
        let _ = win.hide();
        return Ok(false);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = win.hide();
        return Ok(false);
    }
    #[cfg(target_os = "windows")]
    {
        let size = win.outer_size().map_err(|err| err.to_string())?;
        let (x, y) = taskbar::taskbar_anchor(
            &config.taskbar.position,
            size.width as i32,
            size.height as i32,
        );
        win.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }))
            .map_err(|e| e.to_string())?;
        win.show().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[tauri::command]
fn sync_taskbar(app: AppHandle, store: tauri::State<'_, Store>) -> Result<bool, String> {
    sync_taskbar_window(&app, &store)
}

#[tauri::command]
fn show_popup(app: AppHandle) {
    if let Some(p) = app.get_webview_window("popup") {
        if let Some(anchor) = app
            .get_webview_window("taskbar")
            .filter(|window| window.is_visible().unwrap_or(false))
            .or_else(|| app.get_webview_window("sticky"))
        {
            if let (Ok(position), Ok(size), Ok(Some(monitor))) = (
                anchor.outer_position(),
                p.outer_size(),
                anchor.current_monitor(),
            ) {
                let area = monitor.work_area();
                let min_x = area.position.x;
                let min_y = area.position.y;
                let max_x = min_x + (area.size.width as i32 - size.width as i32).max(0);
                let max_y = min_y + (area.size.height as i32 - size.height as i32).max(0);
                let y = if anchor.label() == "taskbar" {
                    position.y - size.height as i32
                } else {
                    position.y
                };
                let _ = p.set_position(tauri::PhysicalPosition::new(
                    position.x.clamp(min_x, max_x),
                    y.clamp(min_y, max_y),
                ));
            }
        }
        let _ = p.show();
        let _ = p.set_focus();
    }
}

#[tauri::command]
fn show_sticky(app: AppHandle) {
    if let Some(window) = app.get_webview_window("sticky") {
        let _ = app.emit_to("sticky", "sticky:expand", ());
        #[cfg(target_os = "windows")]
        match desktop::show(&window) {
            Ok(()) => {
                if let Err(error) = desktop::reveal_desktop() {
                    eprintln!("无法显示 Windows 桌面：{error}");
                }
            }
            Err(error) => {
                eprintln!("无法显示桌面便签：{error}");
                show_popup(app);
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = window.show();
    } else {
        show_popup(app);
    }
}

#[tauri::command]
fn set_sticky_collapsed(
    app: AppHandle,
    store: tauri::State<'_, Store>,
    collapsed: bool,
) -> Result<(), String> {
    let window = app.get_webview_window("sticky").ok_or("便签窗口不存在")?;
    let config = store.config();
    let position = window.outer_position().map_err(|err| err.to_string())?;
    let scale = window.scale_factor().map_err(|err| err.to_string())?;
    let offset = |full: i32, compact: i32| ((full - compact) as f64 * scale).round() as i32;
    let dx = offset(360, 200);
    let dy = offset(480, 36);
    let (width, height, x, y, opacity) = if collapsed {
        (200.0, 36.0, position.x + dx, position.y + dy, 0.30)
    } else {
        (
            360.0,
            480.0,
            position.x - dx,
            position.y - dy,
            f64::from(config.window.opacity.clamp(20, 100)) / 100.0,
        )
    };
    #[cfg(target_os = "windows")]
    {
        desktop::set_bounds(
            &window,
            x,
            y,
            (width * scale).round() as i32,
            (height * scale).round() as i32,
        )?;
        set_win_opacity(&window, opacity)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        window.set_size(tauri::LogicalSize::new(width, height)).map_err(|err| err.to_string())?;
        window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|err| err.to_string())?;
        let _ = window.set_opacity(opacity);
    }
    Ok(())
}

#[tauri::command]
fn move_sticky(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    let window = app.get_webview_window("sticky").ok_or("便签窗口不存在")?;
    #[cfg(target_os = "windows")]
    return desktop::move_to(&window, x, y);
    #[cfg(not(target_os = "windows"))]
    window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|err| err.to_string())
}

#[tauri::command]
fn hide_window(app: AppHandle, label: String) {
    if let Some(w) = app.get_webview_window(&label) {
        #[cfg(target_os = "windows")]
        if label == "sticky" {
            let _ = desktop::hide(&w);
            return;
        }
        let _ = w.hide();
    }
}

#[tauri::command]
fn list_system_fonts() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let script = "$OutputEncoding = [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false); Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts','HKCU:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts' | ForEach-Object { $_.PSObject.Properties | Where-Object { $_.Name -notmatch '^PS' } | ForEach-Object { ($_.Name -replace ' \\(TrueType\\)| \\(OpenType\\)| \\(All res\\)$','').Trim() } } | Sort-Object -Unique";
        let mut command = std::process::Command::new("powershell.exe");
        command.creation_flags(CREATE_NO_WINDOW).args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]);

        if let Ok(output) = command.output() {
            let mut fonts: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(String::from)
                .collect();
            fonts.sort_unstable();
            fonts.dedup();
            if !fonts.is_empty() {
                return fonts;
            }
        }
    }
    vec![
        "Arial".into(),
        "Segoe UI".into(),
        "sans-serif".into(),
        "serif".into(),
        "monospace".into(),
    ]
}

// ---------- 应用启动 ----------

fn show_startup_error(error: &str) {
    eprintln!("JustToDo 无法启动：{error}");
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::core::{w, HSTRING};
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let message = HSTRING::from(format!(
            "无法读取或保存应用数据。现有文件已保留。\n\n{error}"
        ));
        MessageBoxW(
            None,
            &message,
            w!("JustToDo 无法启动"),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub fn run() {
    let dir = storage::data_dir();
    let _instance = match SingleInstance::acquire(&dir) {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => return show_startup_error(&error),
    };
    let store = match Store::open(dir) {
        Ok(store) => store,
        Err(error) => return show_startup_error(&error),
    };
    if let Err(error) = commands::purge_expired_trash(&store) {
        eprintln!("回收站清理失败：{error}");
    }
    let cfg = store.config();

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
            // 应用窗口初始透明度
            if let Some(w) = app.get_webview_window("sticky") {
                if let (Ok(Some(monitor)), Ok(size)) = (w.primary_monitor(), w.outer_size()) {
                    let work = monitor.work_area();
                    let margin = (20.0 * monitor.scale_factor()).round() as i32;
                    let x = work.position.x
                        + (work.size.width as i32 - size.width as i32 - margin).max(0);
                    let y = work.position.y + margin;
                    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = set_win_opacity(&w, cfg.window.opacity as f64 / 100.0);
                    if let Err(error) = desktop::show(&w) {
                        eprintln!("无法挂载桌面便签：{error}");
                    }
                }
                #[cfg(not(target_os = "windows"))]
                let _ = w.show();
            }
            let _ = sync_taskbar_window(app.handle(), &app.state::<Store>());
            // 钉住条定位
            // 托盘
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "显示桌面便签", true, None::<&str>)?;
            let today = MenuItem::with_id(app, "today", "今日要务", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &today, &quit])?;
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("JustToDo")
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_sticky(tray.app_handle().clone());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        show_sticky(app.clone());
                    }
                    "today" => show_popup(app.clone()),
                    _ => {}
                })
                .build(app)?;
            let scheduler_app = app.handle().clone();
            std::thread::spawn(move || loop {
                let _ = check_due_notifications(&scheduler_app, &scheduler_app.state::<Store>());
                std::thread::sleep(std::time::Duration::from_secs(60));
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(window.label(), "sticky" | "popup") {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    hide_window(window.app_handle().clone(), window.label().to_owned());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_data,
            commands::get_config,
            commands::set_config,
            commands::send_test_notification,
            check_notifications,
            commands::create_task,
            commands::update_task,
            commands::reorder_tasks,
            commands::delete_task,
            commands::restore_task,
            commands::create_tab,
            commands::rename_tab,
            commands::delete_tab,
            commands::purge_task,
            set_opacity,
            sync_taskbar,
            show_popup,
            show_sticky,
            set_sticky_collapsed,
            move_sticky,
            hide_window,
            list_system_fonts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running JustToDo");
}
