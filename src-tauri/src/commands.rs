// JustToDo Tauri 命令：任务 / Tab / 分类 / 配置 CRUD + 事件广播

use crate::models::*;
use crate::storage::Store;
use tauri::{AppHandle, Emitter, State};

fn record_history(store: &Store, task: &Task, operation: &str, deleted: bool) {
    let entry = TaskHistoryEntry {
        id: format!("history-{}", uuid::Uuid::new_v4()),
        task_id: task.id.clone(),
        title: task.title.clone(),
        operation: operation.into(),
        timestamp: now_utc(),
        deleted,
        status: task.status.clone(),
    };
    store.with_data_mut(|d| {
        d.history.push(entry);
        if d.history.len() > 500 { let drop_count = d.history.len() - 500; d.history.drain(0..drop_count); }
    });
}

fn emit_task(app: &AppHandle, event: &str, task: &Task) {
    let _ = app.emit(event, serde_json::json!({ "task": task }));
}

#[tauri::command]
pub fn get_tasks(store: State<'_, Store>) -> Vec<Task> {
    store.data().tasks
}

#[tauri::command]
pub fn get_tabs_and_categories(store: State<'_, Store>) -> TabsAndCategories {
    let d = store.data();
    TabsAndCategories {
        tabs: d.tabs,
        categories: d.categories,
    }
}

#[tauri::command]
pub fn get_config(store: State<'_, Store>) -> Config {
    store.config()
}

#[tauri::command]
pub fn set_config(store: State<'_, Store>, config: Config) {
    store.set_config(config);
}

#[tauri::command]
pub fn create_task(app: AppHandle, store: State<'_, Store>, title: String, tab_id: String) -> Task {
    let now = now_utc();
    let order = store.data().tasks.iter().filter(|t| t.tab_id == tab_id).count() as i32;
    let task = Task {
        id: format!("task-{}", uuid::Uuid::new_v4()),
        title,
        status: TaskStatus::Todo,
        tab_id: tab_id.clone(),
        category_id: "cat-other".into(),
        priority: 2,
        due_date: None,
        reminder_time: None,
        reminder_enabled: false,
        notified_at: None,
        order,
        notes: String::new(),
        subtasks: vec![],
        attachments: vec![],
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        deleted_at: None,
    };
    store.with_data_mut(|d| d.tasks.push(task.clone()));
    record_history(&store, &task, "created", false);
    emit_task(&app, "task:created", &task);
    task
}

#[tauri::command]
pub fn update_task(
    app: AppHandle,
    store: State<'_, Store>,
    id: String,
    patch: serde_json::Value,
    client_updated_at: String,
) -> Result<Task, String> {
    let mut result: Result<Task, String> = Err("not found".into());
    store.with_data_mut(|d| {
        if let Some(t) = d.tasks.iter_mut().find(|t| t.id == id) {
            // 乐观锁：服务端 updatedAt 更新则拒绝
            if t.updated_at != client_updated_at {
                result = Err("conflict".into());
                return;
            }
            // 应用 patch
            if let Some(v) = patch.get("title").and_then(|v| v.as_str()) {
                t.title = v.to_string();
            }
            if let Some(v) = patch.get("status").and_then(|v| v.as_str()) {
                t.status = match v {
                    "todo" => TaskStatus::Todo,
                    "doing" => TaskStatus::Doing,
                    "done" => TaskStatus::Done,
                    "cancelled" => TaskStatus::Cancelled,
                    _ => t.status.clone(),
                };
                if v == "done" {
                    t.completed_at = Some(now_utc());
                }
            }
            if let Some(v) = patch.get("priority").and_then(|v| v.as_i64()) {
                t.priority = (v as i32).clamp(0, 4);
            }
            if let Some(v) = patch.get("categoryId").and_then(|v| v.as_str()) {
                t.category_id = v.to_string();
            }
            if let Some(v) = patch.get("dueDate") {
                t.due_date = v.as_str().map(|s| s.to_string());
            }
            if let Some(v) = patch.get("reminderTime") {
                t.reminder_time = v.as_str().map(|s| s.to_string());
            }
            if let Some(v) = patch.get("reminderEnabled").and_then(|v| v.as_bool()) {
                t.reminder_enabled = v;
            }
            if let Some(v) = patch.get("notes").and_then(|v| v.as_str()) {
                t.notes = v.to_string();
            }
            t.updated_at = now_utc();
            result = Ok(t.clone());
        }
    });
    if let Ok(ref task) = result {
        // Record history after releasing the data lock; record_history acquires it again.
        record_history(&store, task, "updated", task.deleted_at.is_some());
        emit_task(&app, "task:updated", task);
    }
    result
}

#[tauri::command]
pub fn reorder_tasks(store: State<'_, Store>, tab_id: String, ordered_ids: Vec<String>) {
    store.with_data_mut(|d| {
        for (order, id) in ordered_ids.iter().enumerate() {
            if let Some(t) = d.tasks.iter_mut().find(|t| &t.id == id && t.tab_id == tab_id) {
                t.order = order as i32;
            }
        }
    });
}

#[tauri::command]
pub fn delete_task(app: AppHandle, store: State<'_, Store>, id: String) {
    let ts = now_utc();
    let deleted_task = store.data().tasks.iter().find(|t| t.id == id).cloned();
    store.with_data_mut(|d| {
        if let Some(t) = d.tasks.iter_mut().find(|t| t.id == id) {
            t.deleted_at = Some(ts.clone());
        }
    });
    if let Some(task) = deleted_task { record_history(&store, &task, "deleted", true); }
    let _ = app.emit("task:deleted", serde_json::json!({ "id": id, "deletedAt": ts }));
}

#[tauri::command]
pub fn restore_task(app: AppHandle, store: State<'_, Store>, id: String) {
    let mut restored = None;
    store.with_data_mut(|d| {
        if let Some(t) = d.tasks.iter_mut().find(|t| t.id == id) {
            if !d.tabs.iter().any(|tab| tab.id == t.tab_id) {
                let tab_id = format!("tab-restore-{}", uuid::Uuid::new_v4());
                d.tabs.push(Tab { id: tab_id.clone(), name: "恢复任务".into(), order: d.tabs.len() as i32, pinned: false, color: "#4A90D9".into(), created_at: now_utc() });
                t.tab_id = tab_id;
            }
            t.deleted_at = None;
            t.updated_at = now_utc();
            restored = Some(t.clone());
        }
    });
    if let Some(task) = restored { record_history(&store, &task, "restored", false); }
    let _ = app.emit("task:restored", serde_json::json!({ "id": id }));
}

#[tauri::command]
pub fn get_task_history(store: State<'_, Store>) -> Vec<TaskHistoryEntry> {
    let mut history = store.data().history;
    history.reverse();
    history
}

// ---------- Tab ----------

#[tauri::command]
pub fn create_tab(app: AppHandle, store: State<'_, Store>, name: String) -> Tab {
    let order = store.data().tabs.len() as i32;
    let tab = Tab {
        id: format!("tab-{}", uuid::Uuid::new_v4()),
        name,
        order,
        pinned: false,
        color: "#4A90D9".into(),
        created_at: now_utc(),
    };
    store.with_data_mut(|d| d.tabs.push(tab.clone()));
    let _ = app.emit("tab:changed", serde_json::json!({ "tabs": store.data().tabs }));
    tab
}

#[tauri::command]
pub fn rename_tab(app: AppHandle, store: State<'_, Store>, id: String, name: String) {
    store.with_data_mut(|d| {
        if let Some(t) = d.tabs.iter_mut().find(|t| t.id == id) {
            t.name = name;
        }
    });
    let _ = app.emit("tab:changed", serde_json::json!({ "tabs": store.data().tabs }));
}

#[tauri::command]
pub fn delete_tab(app: AppHandle, store: State<'_, Store>, id: String) {
    store.with_data_mut(|d| {
        d.tabs.retain(|t| t.id != id);
        d.tasks.retain(|t| t.tab_id != id);
    });
    let _ = app.emit("tab:changed", serde_json::json!({ "tabs": store.data().tabs }));
}

// ---------- 回收站 ----------

#[tauri::command]
pub fn get_trashed_tasks(store: State<'_, Store>) -> Vec<Task> {
    store
        .data()
        .tasks
        .into_iter()
        .filter(|t| t.deleted_at.is_some())
        .collect()
}

#[tauri::command]
pub fn purge_task(store: State<'_, Store>, id: String) {
    store.backup_before_purge();
    store.with_data_mut(|d| d.tasks.retain(|t| t.id != id));
}

// ---------- 回收站 30 天自动清理（启动时调用） ----------

pub fn purge_expired_trash(store: &Store) {
    use chrono::{DateTime, Utc};
    let now = Utc::now();
    let threshold = now - chrono::Duration::days(30);
    store.backup_before_purge();
    store.with_data_mut(|d| {
        d.tasks.retain(|t| {
            if let Some(da) = &t.deleted_at {
                if let Ok(dt) = DateTime::parse_from_rfc3339(da) {
                    return dt.with_timezone(&Utc) > threshold;
                }
            }
            true
        });
    });
}
