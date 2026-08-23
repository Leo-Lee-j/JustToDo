<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Task } from "@/types";

const tasks = ref<Task[]>([]);
const loading = ref(true);

async function load() {
  loading.value = true;
  try {
    tasks.value = await invoke<Task[]>("get_trashed_tasks");
  } finally {
    loading.value = false;
  }
}

async function restore(id: string) {
  await invoke("restore_task", { id });
  tasks.value = tasks.value.filter((task) => task.id !== id);
}

async function purge(id: string) {
  await invoke("purge_task", { id });
  tasks.value = tasks.value.filter((task) => task.id !== id);
}

onMounted(load);
</script>

<template>
  <main class="trash">
    <header class="trash-head">
      <h1>回收站</h1>
      <button class="close" title="关闭" @click="invoke('hide_window', { label: 'sticky' })">✕</button>
    </header>
    <div v-if="loading" class="empty">正在加载...</div>
    <div v-else-if="!tasks.length" class="empty">回收站为空</div>
    <ul v-else class="trash-list">
      <li v-for="task in tasks" :key="task.id" class="trash-item">
        <span class="task-title">{{ task.title }}</span>
        <span class="task-date">{{ task.deletedAt?.slice(0, 10) }}</span>
        <button title="恢复" @click="restore(task.id)">恢复</button>
        <button class="danger" title="永久删除" @click="purge(task.id)">删除</button>
      </li>
    </ul>
  </main>
</template>

<style scoped>
.trash { height: 100vh; display: flex; flex-direction: column; background: var(--bg); color: var(--text); }
.trash-head { display: flex; align-items: center; justify-content: space-between; padding: 10px 12px; border-bottom: 1px solid var(--border); }
h1 { font-size: 14px; }
.close { color: var(--text-soft); padding: 2px 6px; }
.empty { padding: 36px 12px; text-align: center; color: var(--text-soft); font-size: 12px; }
.trash-list { list-style: none; overflow: auto; padding: 6px; }
.trash-item { display: flex; align-items: center; gap: 8px; padding: 8px; border-bottom: 1px solid var(--border); min-height: 38px; }
.task-title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
.task-date { color: var(--text-soft); font-size: 11px; }
.trash-item button { color: var(--primary); font-size: 12px; }
.trash-item .danger { color: var(--danger); }
</style>
