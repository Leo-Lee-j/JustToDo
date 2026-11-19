import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { AppData, Task, TaskStatus } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { syncSnapshot } from "@/utils/snapshot";
import { useToast } from "@/composables/useToast";

function dueTimestamp(task: Task) {
  const date = Date.parse(task.dueDate ?? "");
  return Number.isNaN(date) ? Infinity : date;
}

export const useTaskStore = defineStore("task", () => {
  const data = ref<AppData>({ tasks: [], tabs: [], categories: [], history: [] });
  const loadError = ref("");
  const { showToast, removeToast } = useToast();
  const undoStack: { token: string; tasks: Task[]; expiresAt: number; toastId?: string }[] = [];
  const { load, refresh } = syncSnapshot("data:changed", async () => {
    data.value = await invoke<AppData>("get_app_data");
    loadError.value = "";
  }, (error) => { loadError.value = "无法读取任务：" + String(error); });

  const tasks = computed(() => data.value.tasks);
  const history = computed(() => [...data.value.history].reverse());
  const trashed = computed(() => tasks.value.filter((task) => task.deletedAt)
    .sort((a, b) => Date.parse(b.deletedAt!) - Date.parse(a.deletedAt!)));
  const topPriorityTask = computed(() => tasks.value
    .filter((task) => !task.deletedAt && task.status !== "done" && task.status !== "cancelled")
    .sort((a, b) => a.priority - b.priority || dueTimestamp(a) - dueTimestamp(b)
      || Date.parse(b.createdAt) - Date.parse(a.createdAt))[0]);

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
    if ("dueDate" in patch || "status" in patch || "reminderTime" in patch || "reminderEnabled" in patch) {
      void invoke("check_notifications").catch(console.error);
    }
  }

  async function softDeleteMany(ids: string[]) {
    const deleted: Task[] = [];
    try {
      for (const id of ids) {
        const task = tasks.value.find((item) => item.id === id && !item.deletedAt);
        if (!task) continue;
        await mutate<void>("delete_task", { id });
        deleted.push(task);
      }
    } finally {
      if (deleted.length) {
        const token = crypto.randomUUID();
        const entry: (typeof undoStack)[number] = { token, tasks: deleted, expiresAt: Date.now() + 5000 };
        undoStack.push(entry);
        entry.toastId = showToast({
          message: deleted.length === 1 ? "已删除：" + deleted[0].title : "已删除 " + deleted.length + " 项任务",
          type: "success",
          duration: 5000,
          action: { label: "撤销", handler: () => { undoDelete(token); } },
        });
        setTimeout(() => {
          const index = undoStack.findIndex((entry) => entry.token === token);
          if (index >= 0) undoStack.splice(index, 1);
        }, 5000);
      }
    }
  }

  function undoDelete(token?: string): boolean {
    const now = Date.now();
    while (undoStack.length && undoStack[undoStack.length - 1].expiresAt <= now) undoStack.pop();
    const index = token ? undoStack.findIndex((entry) => entry.token === token) : undoStack.length - 1;
    if (index < 0) return false;
    const [entry] = undoStack.splice(index, 1);
    if (entry.toastId) removeToast(entry.toastId);
    void (async () => {
      for (const task of entry.tasks) {
        try { await restore(task.id); }
        catch (error) { showToast({ type: "error", message: String(error) }); }
      }
    })();
    return true;
  }

  async function restore(id: string) {
    for (let index = undoStack.length - 1; index >= 0; index--) {
      const entry = undoStack[index];
      entry.tasks = entry.tasks.filter((task) => task.id !== id);
      if (!entry.tasks.length) {
        undoStack.splice(index, 1);
        if (entry.toastId) removeToast(entry.toastId);
      }
    }
    return mutate<Task>("restore_task", { id });
  }

  return {
    data, tasks, history, trashed, topPriorityTask, loadError, load, mutate, create, update,
    setStatus: (id: string, status: TaskStatus) => update(id, { status }),
    setPriority: (id: string, priority: number) => update(id, { priority }),
    reorder: (tabId: string, orderedIds: string[]) => mutate<void>("reorder_tasks", { tabId, orderedIds }),
    softDelete: (id: string) => softDeleteMany([id]), softDeleteMany, undoDelete, restore,
    purge: (id: string) => mutate<void>("purge_task", { id }),
  };
});
