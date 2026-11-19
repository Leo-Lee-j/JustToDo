<script setup lang="ts">
import { computed, ref } from "vue";
import { useTaskStore } from "@/stores/taskStore";

const store = useTaskStore();
const busy = ref(false);
const error = ref("");
const deleted = computed(() => store.trashed.some((task) => task.id === store.recentDelete?.id) ? store.recentDelete : undefined);

async function undo() {
  if (!deleted.value || busy.value) return;
  busy.value = true;
  error.value = "";
  try { await store.restore(deleted.value.id); } catch (err) { error.value = String(err); }
  finally { busy.value = false; }
}
</script>

<template>
  <div v-if="deleted" class="undo-delete" role="status"><span>已删除：{{ deleted.title }}</span><button type="button" :disabled="busy" @click="undo">撤销</button></div>
  <div v-if="error" class="undo-delete" role="alert"><span>{{ error }}</span><button type="button" @click="error = ''">关闭</button></div>
</template>

<style scoped>
.undo-delete { flex:none; display:flex; justify-content:space-between; gap:8px; padding:8px 10px; background:var(--bg-soft); font-size:12px; }
.undo-delete span { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.undo-delete button { flex:none; text-decoration:underline; }
</style>
