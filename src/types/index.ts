// JustToDo 全局 TypeScript 类型定义（与 Rust models.rs 对齐）

export type TaskStatus = "todo" | "doing" | "done" | "cancelled";

export interface SubTask {
  id: string;
  title: string;
  done: boolean;
}

export interface Attachment {
  type: "link";
  url: string;
  title: string;
}

export interface Task {
  id: string;
  title: string;
  status: TaskStatus;
  tabId: string;
  categoryId: string;
  priority: number; // 0-4, 0 is highest
  dueDate: string | null; // UTC ISO 8601
  reminderTime: string | null; // UTC ISO 8601
  reminderEnabled: boolean;
  notifiedAt: string | null; // 去重用
  order: number;
  notes: string;
  subtasks: SubTask[];
  attachments: Attachment[];
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
  deletedAt: string | null; // 软删除
}

export interface TaskHistoryEntry {
  id: string;
  taskId: string;
  title: string;
  operation: string;
  timestamp: string;
  deleted: boolean;
  status: TaskStatus;
  updatedAt: string;
  dueDate: string | null;
}

export interface Tab {
  id: string;
  name: string;
  order: number;
  pinned: boolean;
  color: string;
  createdAt: string;
}

export interface Category {
  id: string;
  name: string;
  color: string;
  icon: string;
  order: number;
  isBuiltIn: boolean;
}

export interface WindowConfig {
  x: number;
  y: number;
  width: number;
  height: number;
  alwaysOnTop: boolean;
  opacity: number; // 20-100
  autoHide: boolean;
  autoHideDelay: number; // 秒
}

export interface TaskbarConfig {
  enabled: boolean;
  position: "left" | "center" | "right";
  visibleCount: number;
  title: string;
}

export interface NotificationConfig {
  enabled: boolean;
  reminderHours: number;
  reminderType: "1hour" | "today" | "tomorrow" | "custom";
  soundEnabled: boolean;
}

export interface GeneralConfig {
  launchOnStartup: boolean;
  singleInstance: boolean;
  fontFamily: string;
  shortcuts: ShortcutConfig;
  taskCompletionMode: "checkbox" | "gesture" | "both";
}

export interface ShortcutConfig {
  newTask: string;
  newTab: string;
  search: string;
  showWindow: string;
}

export interface Config {
  window: WindowConfig;
  taskbar: TaskbarConfig;
  notification: NotificationConfig;
  general: GeneralConfig;
  theme: "light" | "dark";
  version: string;
}

export interface AppData {
  tasks: Task[];
  tabs: Tab[];
  categories: Category[];
}

// 跨窗口事件 payload
export interface TaskEventPayload {
  task: Task;
}

export interface TaskDeletedPayload {
  id: string;
  deletedAt: string | null;
}

export interface TabChangedPayload {
  tabs: Tab[];
}

export interface ConfigChangedPayload {
  config: Config;
}
