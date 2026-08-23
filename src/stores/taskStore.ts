import { defineStore } from "pinia";
import type { Task, TaskHistoryEntry, TaskStatus } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useConfigStore } from "./configStore";

export const useTaskStore = defineStore("task", {
  state: () => ({
    tasks: [] as Task[],
    activeTabId: "" as string,
    loaded: false,
    history: [] as TaskHistoryEntry[],
  }),
  getters: {
    activeTasks(state): Task[] {
      return state.tasks
        .filter((t) => t.tabId === state.activeTabId && !t.deletedAt)
        .sort((a, b) => a.order - b.order);
    },
    topPriorityTasks(state): Task[] {
      const cfg = useConfigStore();
      const n = cfg.config.taskbar.visibleCount;
      return [...state.tasks]
        .filter((t) => !t.deletedAt && t.status !== "done" && t.status !== "cancelled")
        .sort((a, b) => {
          if (a.priority !== b.priority) return a.priority - b.priority;
          const ad = a.dueDate ? new Date(a.dueDate).getTime() : Infinity;
          const bd = b.dueDate ? new Date(b.dueDate).getTime() : Infinity;
          if (ad !== bd) return ad - bd;
          return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
        })
        .slice(0, n);
    },
  },
  actions: {
    async refreshHistory() {
      this.history = await invoke<TaskHistoryEntry[]>("get_task_history");
    },
    async load() {
      this.tasks = await invoke<Task[]>("get_tasks");
      await this.refreshHistory();
      this.loaded = true;
      this.subscribe();
    },
    subscribe() {
      listen<{ task: Task }>("task:created", (e) => {
        if (!this.tasks.find((t) => t.id === e.payload.task.id))
          this.tasks.push(e.payload.task);
      });
      listen<{ task: Task }>("task:updated", (e) => {
        const i = this.tasks.findIndex((t) => t.id === e.payload.task.id);
        if (i >= 0) this.tasks[i] = e.payload.task;
      });
      listen<{ id: string; deletedAt: string | null }>("task:deleted", (e) => {
        const t = this.tasks.find((t) => t.id === e.payload.id);
        if (t) t.deletedAt = e.payload.deletedAt;
      });
      listen<{ id: string }>("task:restored", (e) => {
        const t = this.tasks.find((t) => t.id === e.payload.id);
        if (t) t.deletedAt = null;
      });
    },
    async create(title: string, tabId: string) {
      const task = await invoke<Task>("create_task", { title, tabId });
      await this.refreshHistory();
      return task;
    },
    async update(id: string, patch: Partial<Task>) {
      const idx = this.tasks.findIndex((t) => t.id === id);
      if (idx < 0) return;
      const current = this.tasks[idx];
      const updated = await invoke<Task>("update_task", {
        id,
        patch,
        clientUpdatedAt: current.updatedAt,
      });
      this.tasks[idx] = updated;
      await this.refreshHistory();
    },
    async setStatus(id: string, status: TaskStatus) {
      await this.update(id, { status });
    },
    async setPriority(id: string, priority: number) {
      await this.update(id, { priority });
    },
    async reorder(orderedIds: string[]) {
      orderedIds.forEach((id, order) => {
        const t = this.tasks.find((x) => x.id === id);
        if (t) t.order = order;
      });
      await invoke("reorder_tasks", { tabId: this.activeTabId, orderedIds });
    },
    async softDelete(id: string) {
      await invoke("delete_task", { id });
      await this.refreshHistory();
    },
    async restore(id: string) {
      await invoke("restore_task", { id });
      await this.refreshHistory();
    },
  },
});
