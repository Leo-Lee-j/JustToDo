<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTaskStore } from "@/stores/taskStore";
import { useTabStore } from "@/stores/tabStore";
import TaskItem from "./TaskItem.vue";
import type { Task } from "@/types";

const taskStore = useTaskStore();
const tabStore = useTabStore();
const keyword = ref("");
const showTrash = ref(false);
const trashed = ref<Task[]>([]);

const filtered = computed(() => {
  const k = keyword.value.trim().toLowerCase();
  let list = taskStore.tasks.filter((t) => !t.deletedAt);
  if (k) list = list.filter((t) => t.title.toLowerCase().includes(k));
  return list.sort((a, b) => a.order - b.order);
});

onMounted(async () => {
  await tabStore.load();
  await taskStore.load();
});

async function toggleTrash() {
  showTrash.value = !showTrash.value;
  if (showTrash.value) trashed.value = await invoke<Task[]>("get_trashed_tasks");
}

async function restore(id: string) {
  await invoke("restore_task", { id });
  trashed.value = trashed.value.filter((task) => task.id !== id);
}

async function close() {
  await invoke("hide_window", { label: "popup" });
}
</script>

<template>
  <div class="popup">
    <header class="popup-head">
      <input v-model="keyword" placeholder="搜索任务..." class="search" />
      <button @click="toggleTrash" :class="{ active: showTrash }" title="回收站">🗑</button>
      <button @click="close" class="close">✕</button>
    </header>
    <div class="popup-list">
      <template v-if="showTrash">
        <div v-for="t in trashed" :key="t.id" class="trash-row">
          <span>{{ t.title }}</span><button @click="restore(t.id)">恢复</button>
        </div>
        <div v-if="!trashed.length" class="empty">回收站为空</div>
      </template>
      <template v-else>
        <TaskItem v-for="t in filtered" :key="t.id" :task="t" />
        <div v-if="!filtered.length" class="empty">无匹配任务</div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.popup {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg);
  border-radius: var(--radius);
  overflow: hidden;
  border: 1px solid var(--border);
}
.popup-head {
  display: flex;
  gap: 6px;
  padding: 8px;
  border-bottom: 1px solid var(--border);
}
.search {
  flex: 1;
  background: var(--bg-soft);
  border-radius: 6px;
  padding: 6px 10px;
}
.close {
  padding: 0 10px;
  color: var(--text-soft);
}
.popup-head button { padding: 0 6px; color: var(--text-soft); }
.popup-head button.active { color: var(--primary); }
.trash-row { display: flex; gap: 8px; align-items: center; padding: 8px 10px; border-bottom: 1px solid var(--border); font-size: 12px; }
.trash-row span { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.trash-row button { color: var(--primary); }
.popup-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}
.empty {
  text-align: center;
  color: var(--text-soft);
  padding: 30px;
  font-size: 12px;
}
</style>
