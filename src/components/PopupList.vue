<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTaskStore } from "@/stores/taskStore";
import { useConfigStore } from "@/stores/configStore";
import { installWindowShortcuts } from "@/utils/shortcuts";
import TaskItem from "./TaskItem.vue";
import TrashView from "./TrashView.vue";

const taskStore = useTaskStore();
const configStore = useConfigStore();
const keyword = ref("");
const showTrash = ref(false);
const searchInput = ref<HTMLInputElement>();
const filtered = computed(() => {
  const query = keyword.value.trim().toLowerCase();
  return taskStore.tasks.filter((task) => !task.deletedAt &&
    (task.title.toLowerCase().includes(query) || task.notes.toLowerCase().includes(query)))
    .sort((a, b) => a.order - b.order);
});

watch(() => configStore.config.general.shortcuts, (keys, _, onCleanup) => {
  onCleanup(installWindowShortcuts({
    search: () => { showTrash.value = false; searchInput.value?.focus(); },
    showWindow: () => void invoke("show_sticky").catch(console.error),
  }, keys));
}, { immediate: true });

function close() { return invoke("hide_window", { label: "popup" }); }
</script>

<template>
  <div class="popup" @keydown.esc="close">
    <header class="popup-head">
      <input ref="searchInput" v-model="keyword" type="search" aria-label="搜索任务" placeholder="搜索标题和备注" class="search" @input="showTrash = false" />
      <button type="button" :aria-pressed="showTrash" @click="showTrash = !showTrash">回收站</button>
      <button type="button" aria-label="关闭任务列表" @click="close">✕</button>
    </header>
    <p v-if="taskStore.loadError" class="empty" role="alert">{{ taskStore.loadError }}</p>
    <TrashView v-if="showTrash" />
    <div v-else class="popup-list">
      <TaskItem v-for="task in filtered" :key="task.id" :task="task" />
      <div v-if="!filtered.length" class="empty">无匹配任务</div>
    </div>
  </div>
</template>

<style scoped>
.popup { display:flex; flex-direction:column; height:100vh; background:var(--bg); border-radius:var(--radius); overflow:hidden; border:1px solid var(--border); }
.popup-head { display:flex; gap:6px; padding:8px; border-bottom:1px solid var(--border); }
.search { flex:1; min-width:0; background:var(--bg-soft); border-radius:6px; padding:6px 10px; }
.popup-head button { padding:0 6px; color:var(--text-soft); }
.popup-head button[aria-pressed=true] { color:var(--primary); text-decoration:underline; }
.popup-list { flex:1; min-height:0; overflow-y:auto; padding:4px 0; }
.empty { text-align:center; color:var(--text-soft); padding:20px; font-size:12px; }
</style>
