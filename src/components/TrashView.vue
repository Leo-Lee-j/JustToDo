<script setup lang="ts">
import { ref } from "vue";
import { useTaskStore } from "@/stores/taskStore";

const store = useTaskStore();
const pendingPurge = ref<string>();
const busy = ref(false);
const error = ref("");

async function perform(action: () => Promise<unknown>) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try { await action(); pendingPurge.value = undefined; }
  catch (err) { error.value = String(err); }
  finally { busy.value = false; }
}

function purge(id: string) {
  if (pendingPurge.value !== id) {
    pendingPurge.value = id;
    return;
  }
  void perform(() => store.purge(id));
}
</script>

<template>
  <section class="trash" aria-label="回收站">
    <h2>回收站</h2>
    <p v-if="pendingPurge" class="notice">再次点击即可彻底删除。 <button @click="pendingPurge = undefined">取消</button></p>
    <p v-if="error" class="notice" role="alert">{{ error }}</p>
    <p v-if="!store.trashed.length" class="empty">回收站为空</p>
    <ul v-else>
      <li v-for="task in store.trashed" :key="task.id">
        <span :title="task.title">{{ task.title }}</span>
        <button type="button" :disabled="busy" @click="perform(() => store.restore(task.id))">恢复</button>
        <button type="button" class="danger" :disabled="busy" @click="purge(task.id)">{{ pendingPurge === task.id ? '确认彻底删除' : '彻底删除' }}</button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.trash { flex:1; min-height:0; overflow:auto; padding:8px 10px; }
h2 { font-size:13px; margin-bottom:8px; }
li { display:flex; gap:8px; padding:8px 0; align-items:center; border-bottom:1px solid var(--border); }
li span { flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
li button { flex:none; font-size:12px; }
.empty { color:var(--text-soft); padding:16px; text-align:center; font-size:12px; }
.notice, .danger { color:var(--danger); font-size:12px; overflow-wrap:anywhere; }
.notice button { text-decoration:underline; }
</style>
