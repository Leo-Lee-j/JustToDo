import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { AppData, Task, TaskStatus } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { useConfigStore } from "./configStore";
import { syncSnapshot } from "@/utils/snapshot";

function dueTimestamp(task: Task) {
  const date = Date.parse(task.dueDate ?? "");
  return Number.isNaN(date) ? Infinity : date;
}

export const useTaskStore = defineStore("task", () => {
  const configStore = useConfigStore();
  const data = ref<AppData>({ tasks: [], tabs: [], categories: [], history: [] });
  const loadError = ref("");
  const recentDelete = ref<{ id: string; title: string }>();
  let undoTimer: ReturnType<typeof setTimeout> | undefined;
  const { load, refresh } = syncSnapshot("data:changed", async () => {
    data.value = await invoke<AppData>("get_app_data");
    loadError.value = "";
  }, (error) => { loadError.value = "无法读取任务：" + String(error); });

  const tasks = computed(() => data.value.tasks);
  const history = computed(() => [...data.value.history].reverse());
  const trashed = computed(() => tasks.value.filter((task) => task.deletedAt)
    .sort((a, b) => Date.parse(b.deletedAt!) - Date.parse(a.deletedAt!)));
  const topPriorityTasks = computed(() => tasks.value
    .filter((task) => !task.deletedAt && task.status !== "done" && task.status !== "cancelled")
    .sort((a, b) => a.priority - b.priority || dueTimestamp(a) - dueTimestamp(b)
      || Date.parse(b.createdAt) - Date.parse(a.createdAt))
    .slice(0, configStore.config.taskbar.visibleCount));

  async function mutate<T>(command: string, args: Record<string, unknown>): Promise<T> {
    try {
      return await invoke<T>(command, args);
    } finally {
      await refresh();
    }
  }

  async function create(title: string, tabId: string, dueDate: string | null, notes: string, priority: number) {
    const task = await mutate<Task>("create_task", { title, tabId, dueDate, notes, priority });
    if (dueDate) void invoke("check_notifications").catch(console.error);
    return task;
  }

  async function update(id: string, patch: Partial<Task>, clientUpdatedAt?: string) {
    const current = tasks.value.find((task) => task.id === id);
    if (!current) throw new Error("任务已不存在");
    await mutate<Task>("update_task", { id, patch, clientUpdatedAt: clientUpdatedAt ?? current.updatedAt });
    if ("dueDate" in patch || "status" in patch) void invoke("check_notifications").catch(console.error);
  }

  async function softDelete(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    await mutate<void>("delete_task", { id });
    if (!task) return;
    clearTimeout(undoTimer);
    recentDelete.value = { id, title: task.title };
    undoTimer = setTimeout(() => { recentDelete.value = undefined; }, 5000);
  }

  async function restore(id: string) {
    const task = await mutate<Task>("restore_task", { id });
    if (recentDelete.value?.id === id) recentDelete.value = undefined;
    return task;
  }

  return {
    data, tasks, history, trashed, topPriorityTasks, loadError, recentDelete, load, mutate, create, update,
    setStatus: (id: string, status: TaskStatus) => update(id, { status }),
    setPriority: (id: string, priority: number) => update(id, { priority }),
    reorder: (tabId: string, orderedIds: string[]) => mutate<void>("reorder_tasks", { tabId, orderedIds }),
    softDelete, restore,
    purge: (id: string) => mutate<void>("purge_task", { id }),
  };
});
