<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTaskStore } from "@/stores/taskStore";
import { useConfigStore } from "@/stores/configStore";
import { installWindowShortcuts } from "@/utils/shortcuts";

const taskStore = useTaskStore();
const configStore = useConfigStore();
const error = ref("");

watch(() => configStore.config.general.shortcuts, (keys, _, onCleanup) => {
  onCleanup(installWindowShortcuts({ showWindow: () => void invoke("show_sticky").catch(console.error) }, keys));
}, { immediate: true });

function showPopup() { return invoke("show_popup"); }

async function complete(id: string) {
  try {
    error.value = "";
    await taskStore.setStatus(id, "done");
  } catch (err) {
    error.value = String(err);
  }
}
</script>

<template>
  <section class="taskbar-pin" :aria-label="configStore.config.taskbar.title">
    <button type="button" class="pin-title" aria-label="打开任务列表" @click="showPopup">{{ configStore.config.taskbar.title }} <span aria-hidden="true">↗</span></button>
    <span v-if="error || taskStore.loadError" class="pin-error" role="alert">{{ error || taskStore.loadError }}</span>
    <span v-else-if="!taskStore.topPriorityTasks.length" class="empty">暂无待办</span>
    <div v-for="task in taskStore.topPriorityTasks" :key="task.id" class="pin-row">
      <button type="button" class="complete" :aria-label="'完成 ' + task.title" title="标记完成" @click="complete(task.id)">□</button>
      <strong>P{{ task.priority }}</strong><button type="button" class="pin-task" :title="task.title" @click="showPopup">{{ task.title }}</button>
    </div>
  </section>
</template>

<style scoped>
.taskbar-pin { width:100%; height:100%; display:flex; flex-direction:column; background:var(--bg); border:1px solid var(--border); border-radius:var(--radius); padding:4px 8px; text-align:left; }
.pin-title { width:100%; font-size:12px; font-weight:600; color:var(--text-soft); height:20px; flex:none; border-bottom:1px solid var(--border); text-align:left; }
.pin-title span { float:right; }
.pin-row { display:flex; gap:8px; align-items:center; height:21px; flex:none; width:100%; }
.pin-row strong { font-size:11px; color:var(--text-soft); }
.pin-task { font-size:12px; min-width:0; flex:1; text-align:left; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.complete { font-size:16px; color:var(--text-soft); }
.pin-error { font-size:11px; color:var(--danger); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.empty { font-size:12px; color:var(--text-soft); padding:8px 0; }
</style>
