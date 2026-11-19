use crate::models::*;
use crate::storage::Store;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, State};

pub(crate) fn emit_data_changed(app: &AppHandle) {
    let _ = app.emit("data:changed", ());
}

fn save<R>(
    app: &AppHandle,
    store: &Store,
    change: impl FnOnce(&mut AppData) -> Result<R, String>,
) -> Result<R, String> {
    let result = store.mutate(change)?;
    emit_data_changed(app);
    Ok(result)
}

fn record_history(data: &mut AppData, task: &Task, operation: &str) {
    data.history.push(TaskHistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        task_id: task.id.clone(),
        title: task.title.clone(),
        operation: operation.into(),
        timestamp: task.updated_at.clone(),
        deleted: task.deleted_at.is_some(),
        status: task.status,
        updated_at: task.updated_at.clone(),
        due_date: task.due_date.clone(),
    });
    let overflow = data.history.len().saturating_sub(500);
    data.history.drain(..overflow);
}

fn required_text(value: &str, max: usize, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max {
        return Err(format!("{label}须为 1–{max} 个字符"));
    }
    Ok(value.into())
}

fn valid_date(due_date: Option<String>) -> Result<Option<String>, String> {
    if due_date
        .as_deref()
        .is_some_and(|date| DateTime::parse_from_rfc3339(date).is_err())
    {
        return Err("到期时间无效".into());
    }
    Ok(due_date)
}

fn next_task_order(data: &AppData, tab_id: &str) -> i32 {
    data.tasks
        .iter()
        .filter(|task| task.tab_id == tab_id && task.deleted_at.is_none())
        .map(|task| task.order)
        .max()
        .map_or(0, |order| order.saturating_add(1))
}

fn require_tab(data: &AppData, id: &str) -> Result<(), String> {
    data.tabs
        .iter()
        .any(|tab| tab.id == id)
        .then_some(())
        .ok_or_else(|| "Tab 已不存在".into())
}

#[tauri::command]
pub fn get_app_data(store: State<'_, Store>) -> AppData {
    store.data()
}

#[tauri::command]
pub fn get_config(store: State<'_, Store>) -> Config {
    store.config()
}

#[tauri::command]
pub fn set_config(app: AppHandle, store: State<'_, Store>, config: Config) -> Result<(), String> {
    store.set_config(config)?;
    let _ = app.emit("config:changed", ());
    Ok(())
}

#[tauri::command]
pub fn send_test_notification(app: AppHandle, store: State<'_, Store>) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    let mut builder = app
        .notification()
        .builder()
        .title("JustToDo")
        .body("任务提醒测试");
    if store.config().notification.sound_enabled {
        builder = builder.sound(crate::notification_sound());
    }
    builder.show().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn create_task(
    app: AppHandle,
    store: State<'_, Store>,
    title: String,
    tab_id: String,
    due_date: Option<String>,
    notes: String,
    priority: i32,
) -> Result<Task, String> {
    let title = required_text(&title, 255, "任务标题")?;
    let due_date = valid_date(due_date)?;
    save(&app, &store, |data| {
        require_tab(data, &tab_id)?;
        let now = now_utc();
        let task = Task {
            id: format!("task-{}", uuid::Uuid::new_v4()),
            title,
            status: TaskStatus::Todo,
            order: next_task_order(data, &tab_id),
            tab_id,
            category_id: "cat-other".into(),
            priority: priority.clamp(0, 4),
            due_date,
            reminder_time: None,
            reminder_enabled: false,
            notified_at: None,
            reminder_notified_at: None,
            notes: notes.chars().take(255).collect(),
            subtasks: vec![],
            attachments: vec![],
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
            deleted_at: None,
        };
        data.tasks.push(task.clone());
        record_history(data, &task, "created");
        Ok(task)
    })
}

fn update_active_task(
    data: &mut AppData,
    id: &str,
    patch: serde_json::Value,
    client_updated_at: &str,
) -> Result<Task, String> {
    let index = data
        .tasks
        .iter()
        .position(|task| task.id == id && task.deleted_at.is_none())
        .ok_or("任务已删除或不存在")?;
    let mut task = data.tasks[index].clone();
    if task.updated_at != client_updated_at {
        return Err("任务已在其他窗口更新，请核对后重试".into());
    }
    if let Some(title) = patch.get("title").and_then(|value| value.as_str()) {
        task.title = required_text(title, 255, "任务标题")?;
    }
    let now = now_utc();
    if let Some(status) = patch.get("status") {
        let status = serde_json::from_value(status.clone()).map_err(|_| "任务状态无效")?;
        if status != task.status {
            task.completed_at = (status == TaskStatus::Done).then(|| now.clone());
            task.status = status;
        }
    }
    if let Some(priority) = patch.get("priority").and_then(|value| value.as_i64()) {
        task.priority = priority.clamp(0, 4) as i32;
    }
    if let Some(category) = patch.get("categoryId").and_then(|value| value.as_str()) {
        if !data.categories.iter().any(|item| item.id == category) {
            return Err("任务分类已不存在".into());
        }
        task.category_id = category.into();
    }
    if let Some(tab) = patch
        .get("tabId")
        .and_then(|value| value.as_str())
        .filter(|tab| *tab != task.tab_id)
    {
        require_tab(data, tab)?;
        task.order = next_task_order(data, tab);
        task.tab_id = tab.into();
    }
    if let Some(value) = patch.get("dueDate") {
        let due_date = serde_json::from_value(value.clone()).map_err(|_| "到期时间无效")?;
        let due_date = valid_date(due_date)?;
        if task.due_date != due_date {
            task.due_date = due_date;
            task.notified_at = None;
        }
    }
    let mut reminder_changed = false;
    if let Some(value) = patch.get("reminderTime") {
        let reminder_time = serde_json::from_value(value.clone()).map_err(|_| "提醒时间无效")?;
        let reminder_time = valid_date(reminder_time).map_err(|_| "提醒时间须包含有效时区")?;
        if task.reminder_time != reminder_time {
            task.reminder_time = reminder_time;
            reminder_changed = true;
        }
    }
    if let Some(value) = patch.get("reminderEnabled") {
        let reminder_enabled = value.as_bool().ok_or("提醒开关无效")?;
        if task.reminder_enabled != reminder_enabled {
            task.reminder_enabled = reminder_enabled;
            reminder_changed = true;
        }
    }
    if task.reminder_enabled && task.reminder_time.is_none() {
        return Err("请先设置提醒时间".into());
    }
    if reminder_changed {
        task.reminder_notified_at = None;
    }
    if let Some(notes) = patch.get("notes").and_then(|value| value.as_str()) {
        task.notes = notes.chars().take(255).collect();
    }
    task.updated_at = now;
    data.tasks[index] = task.clone();
    record_history(data, &task, "updated");
    Ok(task)
}

#[tauri::command]
pub fn update_task(
    app: AppHandle,
    store: State<'_, Store>,
    id: String,
    patch: serde_json::Value,
    client_updated_at: String,
) -> Result<Task, String> {
    save(&app, &store, |data| {
        update_active_task(data, &id, patch, &client_updated_at)
    })
}

fn apply_task_order(
    data: &mut AppData,
    tab_id: &str,
    ordered_ids: &[String],
) -> Result<(), String> {
    require_tab(data, tab_id)?;
    let ranks: std::collections::HashMap<_, _> = ordered_ids
        .iter()
        .enumerate()
        .map(|(rank, id)| (id.as_str(), rank as i32))
        .collect();
    let active: Vec<_> = data
        .tasks
        .iter_mut()
        .filter(|task| task.tab_id == tab_id && task.deleted_at.is_none())
        .collect();
    if ranks.len() != ordered_ids.len()
        || active.len() != ranks.len()
        || active
            .iter()
            .any(|task| !ranks.contains_key(task.id.as_str()))
    {
        return Err("任务列表已变化，请重新排序".into());
    }
    for task in active {
        task.order = ranks[task.id.as_str()];
    }
    Ok(())
}

#[tauri::command]
pub fn reorder_tasks(
    app: AppHandle,
    store: State<'_, Store>,
    tab_id: String,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    save(&app, &store, |data| {
        apply_task_order(data, &tab_id, &ordered_ids)
    })
}

fn trash_tasks(data: &mut AppData, ids: &HashSet<&str>) {
    let now = now_utc();
    let mut deleted = Vec::new();
    for task in &mut data.tasks {
        if task.deleted_at.is_none() && ids.contains(task.id.as_str()) {
            task.deleted_at = Some(now.clone());
            task.updated_at = now.clone();
            deleted.push(task.clone());
        }
    }
    for task in deleted {
        record_history(data, &task, "deleted");
    }
}

#[tauri::command]
pub fn delete_task(app: AppHandle, store: State<'_, Store>, id: String) -> Result<(), String> {
    save(&app, &store, |data| {
        if !data
            .tasks
            .iter()
            .any(|task| task.id == id && task.deleted_at.is_none())
        {
            return Err("任务已删除或不存在".into());
        }
        trash_tasks(data, &HashSet::from([id.as_str()]));
        Ok(())
    })
}

fn restore_trashed_task(data: &mut AppData, id: &str) -> Result<Task, String> {
    let index = data
        .tasks
        .iter()
        .position(|task| task.id == id && task.deleted_at.is_some())
        .ok_or("任务已恢复或不存在")?;
    let mut task = data.tasks[index].clone();
    if !data.tabs.iter().any(|tab| tab.id == task.tab_id) {
        task.tab_id = data
            .tabs
            .iter()
            .min_by_key(|tab| (!tab.pinned, tab.order))
            .ok_or("请先创建一个 Tab")?
            .id
            .clone();
    }
    task.order = next_task_order(data, &task.tab_id);
    task.deleted_at = None;
    task.updated_at = now_utc();
    data.tasks[index] = task.clone();
    record_history(data, &task, "restored");
    Ok(task)
}

#[tauri::command]
pub fn restore_task(app: AppHandle, store: State<'_, Store>, id: String) -> Result<Task, String> {
    save(&app, &store, |data| restore_trashed_task(data, &id))
}

#[tauri::command]
pub fn create_tab(app: AppHandle, store: State<'_, Store>, name: String) -> Result<Tab, String> {
    let name = required_text(&name, 30, "Tab 名称")?;
    save(&app, &store, |data| {
        let tab = Tab {
            id: format!("tab-{}", uuid::Uuid::new_v4()),
            name,
            order: data
                .tabs
                .iter()
                .map(|tab| tab.order)
                .max()
                .map_or(0, |order| order.saturating_add(1)),
            pinned: false,
            color: "#4A90D9".into(),
            created_at: now_utc(),
        };
        data.tabs.push(tab.clone());
        Ok(tab)
    })
}

#[tauri::command]
pub fn rename_tab(
    app: AppHandle,
    store: State<'_, Store>,
    id: String,
    name: String,
) -> Result<(), String> {
    let name = required_text(&name, 30, "Tab 名称")?;
    save(&app, &store, |data| {
        let tab = data
            .tabs
            .iter_mut()
            .find(|tab| tab.id == id)
            .ok_or("Tab 已不存在")?;
        tab.name = name;
        Ok(())
    })
}

fn delete_tab_and_trash(data: &mut AppData, id: &str) -> Result<(), String> {
    require_tab(data, id)?;
    if data.tabs.len() == 1 {
        return Err("至少保留一个 Tab".into());
    }
    let ids: HashSet<_> = data
        .tasks
        .iter()
        .filter(|task| task.tab_id == id)
        .map(|task| task.id.clone())
        .collect();
    let ids = ids.iter().map(String::as_str).collect();
    trash_tasks(data, &ids);
    data.tabs.retain(|tab| tab.id != id);
    Ok(())
}

#[tauri::command]
pub fn delete_tab(app: AppHandle, store: State<'_, Store>, id: String) -> Result<(), String> {
    save(&app, &store, |data| delete_tab_and_trash(data, &id))
}

fn purge_trashed_task(data: &mut AppData, id: &str) -> Result<(), String> {
    let index = data
        .tasks
        .iter()
        .position(|task| task.id == id && task.deleted_at.is_some())
        .ok_or("只能彻底删除回收站中的任务")?;
    data.tasks.remove(index);
    Ok(())
}

#[tauri::command]
pub fn purge_task(app: AppHandle, store: State<'_, Store>, id: String) -> Result<(), String> {
    store.mutate_with_backup(|data| purge_trashed_task(data, &id))?;
    emit_data_changed(&app);
    Ok(())
}

pub fn purge_expired_trash(store: &Store) -> Result<(), String> {
    let threshold = Utc::now() - Duration::days(30);
    let expired = |task: &Task| {
        task.deleted_at
            .as_deref()
            .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
            .is_some_and(|date| date <= threshold)
    };
    if store.data().tasks.iter().any(expired) {
        store.mutate_with_backup(|data| {
            data.tasks.retain(|task| !expired(task));
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::tests::TestDir;
    use serde_json::json;

    fn task(id: &str, tab_id: &str, order: i32) -> Task {
        let now = now_utc();
        serde_json::from_value(json!({
            "id": id, "title": id, "status": "todo", "tabId": tab_id,
            "categoryId": "cat-other", "priority": 2, "dueDate": null,
            "reminderTime": null, "reminderEnabled": false, "notifiedAt": null, "reminderNotifiedAt": null,
            "order": order, "notes": "", "subtasks": [], "attachments": [],
            "createdAt": now, "updatedAt": now, "completedAt": null, "deletedAt": null
        }))
        .unwrap()
    }

    #[test]
    fn deleting_tab_preserves_tasks_and_restores_to_existing_tab() {
        let mut data = AppData {
            tasks: vec![task("work", "tab-work", 0), task("today", "tab-today", 4)],
            ..AppData::default()
        };
        delete_tab_and_trash(&mut data, "tab-work").unwrap();
        assert_eq!(data.tasks.len(), 2);
        assert!(data.tasks[0].deleted_at.is_some());
        assert!(data.tasks[1].deleted_at.is_none());
        assert!(data.history[0].deleted);
        let restored = restore_trashed_task(&mut data, "work").unwrap();
        assert_eq!(restored.tab_id, "tab-today");
        assert_eq!(restored.order, 5);
        assert!(restored.deleted_at.is_none());
        assert_eq!(data.history.last().unwrap().operation, "restored");
    }

    #[test]
    fn last_tab_and_active_tasks_cannot_be_permanently_deleted() {
        let mut data = AppData::default();
        data.tabs.truncate(1);
        assert!(delete_tab_and_trash(&mut data, "tab-today").is_err());
        data.tasks.push(task("active", "tab-today", 0));
        assert!(purge_trashed_task(&mut data, "active").is_err());
        assert!(restore_trashed_task(&mut data, "active").is_err());
        assert_eq!(data.tasks.len(), 1);
    }

    #[test]
    fn ordering_rejects_stale_duplicate_and_foreign_ids() {
        let mut data = AppData {
            tasks: vec![
                task("a", "tab-today", 0),
                task("b", "tab-today", 1),
                task("other", "tab-work", 0),
            ],
            ..AppData::default()
        };
        for ids in [vec!["a"], vec!["a", "a"], vec!["a", "other"]] {
            let ids: Vec<_> = ids.into_iter().map(String::from).collect();
            assert!(apply_task_order(&mut data, "tab-today", &ids).is_err());
            assert_eq!(data.tasks[0].order, 0);
        }
        apply_task_order(&mut data, "tab-today", &["b".into(), "a".into()]).unwrap();
        assert_eq!(data.tasks[0].order, 1);
        assert_eq!(data.tasks[1].order, 0);
        assert_eq!(data.tasks[2].order, 0);
    }

    #[test]
    fn edit_conflicts_are_rejected_and_reopening_clears_completion() {
        let mut data = AppData::default();
        data.tasks.push(task("edit", "tab-today", 0));
        let version = data.tasks[0].updated_at.clone();
        let done =
            update_active_task(&mut data, "edit", json!({"status": "done"}), &version).unwrap();
        assert_eq!(done.status, TaskStatus::Done);
        assert!(done.completed_at.is_some());
        assert_ne!(done.updated_at, version);
        assert!(update_active_task(&mut data, "edit", json!({"title":"stale"}), &version).is_err());
        let reopened = update_active_task(
            &mut data,
            "edit",
            json!({"status":"doing"}),
            &done.updated_at,
        )
        .unwrap();
        assert!(reopened.completed_at.is_none());
        assert_eq!(data.tasks[0].title, "edit");
    }

    #[test]
    fn due_date_edits_reset_notification_and_move_appends_to_target() {
        let mut data = AppData::default();
        let mut edited = task("edit", "tab-today", 0);
        edited.notified_at = Some(now_utc());
        data.tasks = vec![edited, task("target", "tab-work", 7)];
        let version = data.tasks[0].updated_at.clone();
        let updated = update_active_task(
            &mut data,
            "edit",
            json!({
                "dueDate":"2030-01-01T00:00:00Z", "tabId":"tab-work", "categoryId":"cat-study"
            }),
            &version,
        )
        .unwrap();
        assert!(updated.notified_at.is_none());
        assert_eq!(updated.order, 8);
        assert_eq!(updated.tab_id, "tab-work");
        assert_eq!(updated.category_id, "cat-study");
        assert!(update_active_task(
            &mut data,
            "edit",
            json!({"title":"  "}),
            &updated.updated_at
        )
        .is_err());
        assert!(update_active_task(
            &mut data,
            "edit",
            json!({"dueDate":"invalid"}),
            &updated.updated_at
        )
        .is_err());
        assert_eq!(data.tasks[0].due_date, updated.due_date);
    }

    #[test]
    fn task_reminder_requires_a_valid_time_and_resets_only_its_own_deduplication() {
        let mut data = AppData::default();
        let mut task = task("reminder", "tab-today", 0);
        task.notified_at = Some("due-reminder-sent".into());
        task.reminder_notified_at = Some("custom-reminder-sent".into());
        data.tasks.push(task);
        let version = data.tasks[0].updated_at.clone();

        let updated = update_active_task(
            &mut data,
            "reminder",
            json!({ "reminderTime": "2030-03-04T10:30:00+08:00", "reminderEnabled": true }),
            &version,
        )
        .unwrap();
        assert_eq!(
            updated.reminder_time.as_deref(),
            Some("2030-03-04T10:30:00+08:00")
        );
        assert!(updated.reminder_enabled);
        assert!(updated.reminder_notified_at.is_none());
        assert_eq!(updated.notified_at.as_deref(), Some("due-reminder-sent"));

        assert!(update_active_task(
            &mut data,
            "reminder",
            json!({ "reminderTime": "not-a-date" }),
            &updated.updated_at,
        )
        .is_err());
        assert!(update_active_task(
            &mut data,
            "reminder",
            json!({ "reminderEnabled": true, "reminderTime": null }),
            &updated.updated_at,
        )
        .is_err());
        assert_eq!(data.tasks[0].reminder_time, updated.reminder_time);
    }

    #[test]
    fn existing_task_data_without_reminder_deduplication_migrates_on_read() {
        let mut value = json!({
            "id": "legacy", "title": "legacy", "status": "todo", "tabId": "tab-today",
            "categoryId": "cat-other", "priority": 2, "dueDate": null,
            "reminderTime": null, "reminderEnabled": false, "notifiedAt": null,
            "order": 0, "notes": "", "subtasks": [], "attachments": [],
            "createdAt": "now", "updatedAt": "now", "completedAt": null, "deletedAt": null
        });
        value.as_object_mut().unwrap().remove("reminderNotifiedAt");
        let task: Task = serde_json::from_value(value).unwrap();
        assert!(task.reminder_notified_at.is_none());
    }

    #[test]
    fn expired_trash_cleanup_preserves_recent_tasks_and_creates_backup() {
        let dir = TestDir::new();
        let store = Store::open(dir.0.clone()).unwrap();
        store
            .mutate(|data| {
                let mut expired = task("expired", "tab-today", 0);
                expired.deleted_at = Some((Utc::now() - Duration::days(31)).to_rfc3339());
                let mut recent = task("recent", "tab-today", 1);
                recent.deleted_at = Some(now_utc());
                data.tasks = vec![expired, recent, task("live", "tab-today", 2)];
                Ok(())
            })
            .unwrap();
        purge_expired_trash(&store).unwrap();
        assert_eq!(store.data().tasks.len(), 2);
        assert!(!store.data().tasks.iter().any(|task| task.id == "expired"));
        let backups: Vec<_> = std::fs::read_dir(dir.0.join("backup")).unwrap().collect();
        assert_eq!(backups.len(), 1);
        purge_expired_trash(&store).unwrap();
        assert_eq!(std::fs::read_dir(dir.0.join("backup")).unwrap().count(), 1);
    }
}
