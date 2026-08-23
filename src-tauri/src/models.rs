// JustToDo Rust 数据模型（与 src/types/index.ts 对齐）

use serde::{Deserialize, Serialize};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubTask {
    pub id: String,
    pub title: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    #[serde(rename = "type")]
    pub kind: String, // "link"
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub tab_id: String,
    pub category_id: String,
    pub priority: i32, // 0-4, 0 is highest
    pub due_date: Option<String>,
    pub reminder_time: Option<String>,
    pub reminder_enabled: bool,
    pub notified_at: Option<String>,
    pub order: i32,
    pub notes: String,
    pub subtasks: Vec<SubTask>,
    pub attachments: Vec<Attachment>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskHistoryEntry {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub operation: String,
    pub timestamp: String,
    pub deleted: bool,
    #[serde(default = "default_history_status")]
    pub status: TaskStatus,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub due_date: Option<String>,
}

fn default_history_status() -> TaskStatus { TaskStatus::Todo }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: String,
    pub name: String,
    pub order: i32,
    pub pinned: bool,
    pub color: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub order: i32,
    pub is_built_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowConfig {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub always_on_top: bool,
    pub opacity: i32, // 20-100
    pub auto_hide: bool,
    pub auto_hide_delay: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarConfig {
    pub enabled: bool,
    pub position: String, // left | center | right
    pub visible_count: i32,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationConfig {
    #[serde(default = "default_notifications_enabled")]
    pub enabled: bool,
    #[serde(default = "default_reminder_hours")]
    pub reminder_hours: i32,
    #[serde(default)]
    pub reminder_type: String,
    #[serde(default = "default_sound_enabled")]
    pub sound_enabled: bool,
}

fn default_reminder_hours() -> i32 { 1 }
fn default_notifications_enabled() -> bool { true }
fn default_sound_enabled() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConfig {
    pub launch_on_startup: bool,
    pub single_instance: bool,
    pub font_family: String,
    #[serde(default)]
    pub shortcuts: ShortcutConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutConfig {
    #[serde(default = "default_new_task_shortcut")]
    pub new_task: String,
    #[serde(default = "default_new_tab_shortcut")]
    pub new_tab: String,
    #[serde(default = "default_search_shortcut")]
    pub search: String,
    #[serde(default = "default_show_window_shortcut")]
    pub show_window: String,
}

fn default_new_task_shortcut() -> String { "Ctrl+N".into() }
fn default_new_tab_shortcut() -> String { "Ctrl+Shift+T".into() }
fn default_search_shortcut() -> String { "Ctrl+F".into() }
fn default_show_window_shortcut() -> String { "Ctrl+Shift+Space".into() }

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self { new_task: default_new_task_shortcut(), new_tab: default_new_tab_shortcut(), search: default_search_shortcut(), show_window: default_show_window_shortcut() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub window: WindowConfig,
    pub taskbar: TaskbarConfig,
    pub notification: NotificationConfig,
    pub general: GeneralConfig,
    pub theme: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub tasks: Vec<Task>,
    pub tabs: Vec<Tab>,
    pub categories: Vec<Category>,
    #[serde(default)]
    pub history: Vec<TaskHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsAndCategories {
    pub tabs: Vec<Tab>,
    pub categories: Vec<Category>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            window: WindowConfig {
                x: 0,
                y: 20,
                width: 360,
                height: 480,
                always_on_top: false,
                opacity: 100,
                auto_hide: false,
                auto_hide_delay: 30,
            },
            taskbar: TaskbarConfig {
                enabled: true,
                position: "right".into(),
                visible_count: 3,
                title: "📌 今日要务".into(),
            },
            notification: NotificationConfig {
                enabled: true,
                reminder_hours: 1,
                reminder_type: "1hour".into(),
                sound_enabled: true,
            },
            general: GeneralConfig {
                launch_on_startup: false,
                single_instance: true,
                font_family: "Microsoft YaHei".into(),
                shortcuts: ShortcutConfig::default(),
            },
            theme: "light".into(),
            version: APP_VERSION.into(),
        }
    }
}

impl Default for AppData {
    fn default() -> Self {
        AppData {
            tasks: vec![],
            tabs: vec![
                Tab {
                    id: "tab-today".into(),
                    name: "📌 今日要务".into(),
                    order: 0,
                    pinned: true,
                    color: "#D0021B".into(),
                    created_at: now_utc(),
                },
                Tab {
                    id: "tab-work".into(),
                    name: "💼 工作".into(),
                    order: 1,
                    pinned: false,
                    color: "#4A90D9".into(),
                    created_at: now_utc(),
                },
                Tab {
                    id: "tab-life".into(),
                    name: "🏠 生活".into(),
                    order: 2,
                    pinned: false,
                    color: "#7ED321".into(),
                    created_at: now_utc(),
                },
            ],
            categories: vec![
                Category { id: "cat-work".into(), name: "工作".into(), color: "#4A90D9".into(), icon: "💼".into(), order: 0, is_built_in: true },
                Category { id: "cat-personal".into(), name: "个人".into(), color: "#7ED321".into(), icon: "🏠".into(), order: 1, is_built_in: true },
                Category { id: "cat-study".into(), name: "学习".into(), color: "#9013FE".into(), icon: "📚".into(), order: 2, is_built_in: true },
                Category { id: "cat-shopping".into(), name: "购物".into(), color: "#F5A623".into(), icon: "🛒".into(), order: 3, is_built_in: true },
                Category { id: "cat-health".into(), name: "健康".into(), color: "#D0021B".into(), icon: "🏃".into(), order: 4, is_built_in: true },
                Category { id: "cat-other".into(), name: "其他".into(), color: "#9B9B9B".into(), icon: "📝".into(), order: 5, is_built_in: true },
            ],
            history: vec![],
        }
    }
}

pub fn now_utc() -> String {
    use chrono::Utc;
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
