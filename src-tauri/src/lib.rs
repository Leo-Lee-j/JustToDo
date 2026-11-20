mod commands;
#[cfg(target_os = "windows")]
mod desktop;
mod models;
mod storage;

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

fn scheduled_reminder(
    task: &models::Task,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    if !task.reminder_enabled || task.notified_at.is_some() || task.reminder_notified_at.is_some() {
        return None;
    }
    use chrono::DateTime;
    let reminder = DateTime::parse_from_rfc3339(task.reminder_time.as_deref()?).ok()?;
    let reminder = reminder.with_timezone(&chrono::Utc);
    (reminder <= now).then_some(reminder)
}

fn due_notification(
    task: &models::Task,
    now: chrono::DateTime<chrono::Utc>,
    hours: i32,
) -> Option<(String, chrono::DateTime<chrono::Utc>)> {
    if task.notified_at.is_some()
        || task.deleted_at.is_some()
        || !matches!(
            task.status,
            models::TaskStatus::Todo | models::TaskStatus::Doing
        )
    {
        return None;
    }
    use chrono::DateTime;
    let raw = task.due_date.as_ref()?;
    let end = DateTime::parse_from_rfc3339(raw)
        .ok()?
        .with_timezone(&chrono::Utc);
    due_within_window(end, now, hours).then(|| (raw.clone(), end))
}

fn check_due_notifications(app: &AppHandle, store: &Store) -> Result<usize, String> {
    let _check_guard = NOTIFICATION_CHECK_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "notification lock poisoned".to_string())?;
    use chrono::Utc;
    use tauri_plugin_notification::NotificationExt;

    let config = store.config();
    if !config.notification.enabled {
        return Ok(0);
    }
    let now = Utc::now();
    let mut due = Vec::new();
    let mut reminders = Vec::new();
    for task in &store.data().tasks {
        if task.deleted_at.is_some()
            || !matches!(
                task.status,
                models::TaskStatus::Todo | models::TaskStatus::Doing
            )
        {
            continue;
        }
        let scheduled = scheduled_reminder(task, now);
        if let Some(reminder) = scheduled {
            reminders.push((
                task.id.clone(),
                task.title.clone(),
                task.reminder_time
                    .clone()
                    .expect("scheduled reminder has a time"),
                reminder,
            ));
        }
        if scheduled.is_none() {
            if let Some((date, end)) =
                due_notification(task, now, config.notification.reminder_hours)
            {
                due.push((task.id.clone(), task.title.clone(), date, end));
            }
        }
    }
    if due.is_empty() && reminders.is_empty() {
        return Ok(0);
    }

    let reminder_ids: std::collections::HashSet<&str> =
        reminders.iter().map(|(id, ..)| id.as_str()).collect();
    let mut lines: Vec<String> = due
        .iter()
        .filter(|(id, ..)| !reminder_ids.contains(id.as_str()))
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
    lines.extend(
        reminders
            .iter()
            .take(5usize.saturating_sub(lines.len()))
            .map(|(id, title, _, reminder)| {
                let scheduled = reminder.with_timezone(&chrono::Local).format("%m/%d %H:%M");
                if let Some((_, _, _, end)) = due.iter().find(|(due_id, ..)| due_id == id) {
                    let seconds = (*end - now).num_seconds();
                    let remaining = if seconds <= 0 {
                        "已到期".to_string()
                    } else if seconds < 3600 {
                        format!("剩余 {} 分钟", (seconds + 59) / 60)
                    } else {
                        format!("剩余 {} 小时", (seconds + 3599) / 3600)
                    };
                    format!("{} · 提醒时间已到 ({}) · {}", title, scheduled, remaining)
                } else {
                    format!("{} · 提醒时间已到 ({})", title, scheduled)
                }
            }),
    );
    let due_ids: std::collections::HashSet<&str> = due.iter().map(|(id, ..)| id.as_str()).collect();
    let total = due_ids.len() + reminder_ids.difference(&due_ids).count();
    if total > lines.len() {
        lines.push(format!("还有 {} 条提醒", total - lines.len()));
    }
    let mut builder = app
        .notification()
        .builder()
        .title("JustToDo 任务提醒")
        .body(lines.join("\n"));
    if config.notification.sound_enabled {
        builder = builder.sound(notification_sound());
    }
    #[cfg(target_os = "windows")]
    {
        builder = builder.id(1);
    }
    builder.show().map_err(|e| e.to_string())?;

    let notified_at = models::now_utc();
    let count = total;
    store.mutate(|data| {
        for (id, _, date, _) in &due {
            if let Some(task) = data.tasks.iter_mut().find(|task| {
                task.id == *id && task.due_date.as_ref() == Some(date) && task.notified_at.is_none()
            }) {
                task.notified_at = Some(notified_at.clone());
                task.reminder_notified_at = Some(notified_at.clone());
            }
        }
        for (id, _, reminder_time, _) in &reminders {
            if let Some(task) = data.tasks.iter_mut().find(|task| {
                task.id == *id
                    && task.reminder_enabled
                    && task.reminder_time.as_ref() == Some(reminder_time)
                    && task.reminder_notified_at.is_none()
            }) {
                task.notified_at = Some(notified_at.clone());
                task.reminder_notified_at = Some(notified_at.clone());
            }
        }
        Ok(())
    })?;
    commands::emit_data_changed(app);
    Ok(count)
}

#[cfg(test)]
mod notification_tests {
    use super::{due_notification, due_within_window, scheduled_reminder};
    use crate::models::{Task, TaskStatus};
    use chrono::{DateTime, Duration, Utc};

    fn reminder_task(
        reminder_time: Option<String>,
        reminder_enabled: bool,
        reminder_notified_at: Option<String>,
    ) -> Task {
        Task {
            id: "task-reminder".into(),
            title: "提醒测试".into(),
            status: TaskStatus::Todo,
            tab_id: "tab-today".into(),
            category_id: "cat-other".into(),
            priority: 2,
            due_date: None,
            reminder_time,
            reminder_enabled,
            notified_at: None,
            reminder_notified_at,
            order: 0,
            notes: String::new(),
            subtasks: vec![],
            attachments: vec![],
            created_at: String::new(),
            updated_at: String::new(),
            completed_at: None,
            deleted_at: None,
        }
    }

    #[test]
    fn scheduled_reminders_wait_then_fire_once() {
        let now = Utc::now();
        let future = (now + Duration::minutes(1)).to_rfc3339();
        let task = reminder_task(Some(future), true, None);
        assert!(scheduled_reminder(&task, now).is_none());

        let due = (now - Duration::seconds(1)).to_rfc3339();
        let task = reminder_task(Some(due.clone()), true, None);
        assert_eq!(
            scheduled_reminder(&task, now),
            DateTime::parse_from_rfc3339(&due)
                .ok()
                .map(|v| v.with_timezone(&Utc))
        );

        let task = reminder_task(Some(due.clone()), true, Some(now.to_rfc3339()));
        assert!(scheduled_reminder(&task, now).is_none());
        let mut task = reminder_task(Some(due.clone()), true, None);
        task.due_date = Some(due.clone());
        task.notified_at = Some(now.to_rfc3339());
        assert!(scheduled_reminder(&task, now).is_none());
        assert!(due_notification(&task, now, 1).is_none());
        task.notified_at = None;
        assert_eq!(
            due_notification(&task, now, 1).map(|entry| entry.0),
            Some(due.clone())
        );
        let task = reminder_task(Some(due), false, None);
        assert!(scheduled_reminder(&task, now).is_none());
    }

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

fn show_popup(app: AppHandle) {
    if let Some(p) = app.get_webview_window("popup") {
        if let Some(anchor) = app.get_webview_window("sticky") {
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
                let _ = p.set_position(tauri::PhysicalPosition::new(
                    position.x.clamp(min_x, max_x),
                    position.y.clamp(min_y, max_y),
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
        window
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(|err| err.to_string())?;
        window
            .set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|err| err.to_string())?;
        let _ = window.set_opacity(opacity);
    }
    Ok(())
}

#[tauri::command]
fn move_sticky(
    app: AppHandle,
    store: tauri::State<'_, Store>,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let window = app.get_webview_window("sticky").ok_or("便签窗口不存在")?;
    #[cfg(target_os = "windows")]
    desktop::move_to(&window, x, y)?;
    #[cfg(not(target_os = "windows"))]
    window
        .set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|err| err.to_string())?;
    store.save_sticky_position(x, y)
}

fn clamp_saved_position(
    window: &tauri::webview::WebviewWindow,
    x: i32,
    y: i32,
) -> Option<tauri::PhysicalPosition<i32>> {
    let size = window.outer_size().ok()?;
    let monitors = window.available_monitors().ok()?;
    monitors
        .into_iter()
        .map(|monitor| {
            let area = monitor.work_area();
            let left = area.position.x;
            let top = area.position.y;
            let right = left + (area.size.width as i32 - size.width as i32).max(0);
            let bottom = top + (area.size.height as i32 - size.height as i32).max(0);
            let px = x.clamp(left, right);
            let py = y.clamp(top, bottom);
            let dx = i64::from(px) - i64::from(x);
            let dy = i64::from(py) - i64::from(y);
            (dx * dx + dy * dy, tauri::PhysicalPosition::new(px, py))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, position)| position)
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

#[cfg(target_os = "windows")]
fn confirm_recovery(recovery: &storage::StartupRecovery) -> bool {
    use windows::core::{w, HSTRING};
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO,
    };
    let message = match recovery {
        storage::StartupRecovery::Tasks(backup) => {
            let time = chrono::DateTime::parse_from_rfc3339(&backup.saved_at)
                .map(|date| date.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|_| backup.saved_at.clone());
            format!("任务文件无法读取。是否恢复 {time} 的备份（{} 项任务，{} 项在回收站）？\n\n受损文件会保留在 backup 文件夹中。", backup.active_tasks, backup.trashed_tasks)
        }
        storage::StartupRecovery::Settings { has_backup: true } =>
            "设置文件无法读取。是否恢复上一次保存的设置？\n\n受损文件会保留在 backup 文件夹中。".into(),
        storage::StartupRecovery::Settings { has_backup: false } =>
            "设置文件无法读取，也没有可用的设置备份。是否使用默认设置继续？\n\n受损文件会保留在 backup 文件夹中。".into(),
    };
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            w!("JustToDo 数据恢复"),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    }
}

#[tauri::command]
fn list_backups(store: tauri::State<'_, Store>) -> Result<Vec<storage::Backup>, String> {
    store.backups()
}

#[tauri::command]
fn restore_backup(
    app: AppHandle,
    store: tauri::State<'_, Store>,
    id: String,
) -> Result<(), String> {
    let _guard = NOTIFICATION_CHECK_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "提醒服务暂不可用")?;
    store.restore_backup(&id)?;
    commands::emit_data_changed(&app);
    Ok(())
}

pub fn run() {
    let dir = storage::data_dir();
    let _instance = match SingleInstance::acquire(&dir) {
        Ok(Some(instance)) => instance,
        Ok(None) => {
            let _ = storage::request_activation(&dir);
            return;
        }
        Err(error) => return show_startup_error(&error),
    };
    let store = loop {
        match Store::open(dir.clone()) {
            Ok(store) => break store,
            Err(error) => {
                #[cfg(target_os = "windows")]
                if let Some(recovery) = storage::startup_recovery(&dir) {
                    if !confirm_recovery(&recovery) {
                        return;
                    }
                    if let Err(error) = storage::recover_startup(&dir, recovery) {
                        return show_startup_error(&error);
                    }
                    continue;
                }
                return show_startup_error(&error);
            }
        }
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
        .manage(store)
        .setup(move |app| {
            // 应用窗口初始透明度
            if let Some(w) = app.get_webview_window("sticky") {
                let restored_position = app.state::<Store>().sticky_position();
                if let (Ok(Some(monitor)), Ok(size)) = (w.primary_monitor(), w.outer_size()) {
                    let work = monitor.work_area();
                    let margin = (20.0 * monitor.scale_factor()).round() as i32;
                    let x = work.position.x
                        + (work.size.width as i32 - size.width as i32 - margin).max(0);
                    let y = work.position.y + margin;
                    let position = restored_position
                        .and_then(|saved| clamp_saved_position(&w, saved.x, saved.y))
                        .unwrap_or_else(|| tauri::PhysicalPosition::new(x, y));
                    let _ = w.set_position(position);
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
            // 托盘
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "显示桌面便签", true, None::<&str>)?;
            let list = MenuItem::with_id(app, "list", "任务列表", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &list, &quit])?;
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
                    "list" => show_popup(app.clone()),
                    _ => {}
                })
                .build(app)?;
            let activation_app = app.handle().clone();
            let activation_dir = storage::data_dir();
            std::thread::spawn(move || loop {
                if storage::take_activation(&activation_dir) {
                    show_sticky(activation_app.clone());
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            });
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
            commands::update_tab,
            commands::reorder_tabs,
            commands::delete_tab,
            commands::create_category,
            commands::update_category,
            commands::delete_category,
            commands::purge_task,
            list_backups,
            restore_backup,
            set_opacity,
            show_sticky,
            set_sticky_collapsed,
            move_sticky,
            hide_window,
            list_system_fonts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running JustToDo");
}
