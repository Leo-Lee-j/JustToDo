<script setup lang="ts">
import { computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTaskStore } from "@/stores/taskStore";
import { useConfigStore } from "@/stores/configStore";
import { useTabStore } from "@/stores/tabStore";
import TaskItem from "./TaskItem.vue";

const taskStore = useTaskStore();
const configStore = useConfigStore();
const tabStore = useTabStore();

const topTasks = computed(() => taskStore.topPriorityTasks);

async function showPopup() {
  await invoke("show_popup");
}

onMounted(async () => {
  await configStore.load();
  await tabStore.load();
  await taskStore.load();
});
</script>

<template>
  <div class="taskbar-pin" @click="showPopup">
    <div class="pin-title">{{ configStore.config.taskbar.title }}</div>
    <div class="pin-list">
      <div v-if="!topTasks.length" class="empty">暂无待办 🎉</div>
      <TaskItem v-for="t in topTasks" :key="t.id" :task="t" />
    </div>
  </div>
</template>

<style scoped>
.taskbar-pin {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 6px;
  box-shadow: var(--shadow);
  cursor: pointer;
  height: 100vh;
  overflow: hidden;
}
.pin-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-soft);
  padding: 2px 4px 4px;
  border-bottom: 1px solid var(--border);
}
.pin-list {
  overflow-y: auto;
}
.empty {
  font-size: 11px;
  color: var(--text-soft);
  padding: 12px 8px;
  text-align: center;
}
</style>
