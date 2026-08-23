<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Task } from "@/types";
import { useTaskStore } from "@/stores/taskStore";
import { useTabStore } from "@/stores/tabStore";
import TrashCan16 from "@carbon/icons-vue/lib/trash-can/16.js";
import Checkbox16 from "@carbon/icons-vue/lib/checkbox/16.js";
import CheckboxChecked16 from "@carbon/icons-vue/lib/checkbox--checked/16.js";

const props = defineProps<{ task: Task }>();
const taskStore = useTaskStore();
const tabStore = useTabStore();
const expanded = ref(false);
const notes = ref(props.task.notes || "");
const category = computed(() => tabStore.categoryMap[props.task.categoryId]);
const isDone = computed(() => props.task.status === "done");
const priority = computed(() => Math.max(0, Math.min(4, props.task.priority)));
const swipeX = ref(0);
const swiping = ref(false);
const pointerStartX = ref(0);
const pointerStartY = ref(0);
const pointerId = ref<number | null>(null);
const suppressClick = ref(false);
const pointerMoved = ref(false);
const revealed = ref(false);
const deleteWidth = 56;
const updatedLabel = computed(() => {
  const date = new Date(props.task.updatedAt);
  if (Number.isNaN(date.getTime())) return "";
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const time = date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  if (sameDay) return `今天 ${time}`;
  if (date.toDateString() === yesterday.toDateString()) return `昨天 ${time}`;
  return `${date.getMonth() + 1}月${date.getDate()}日 ${time}`;
});
const dueLabel = computed(() => {
  if (!props.task.dueDate) return "";
  const date = new Date(props.task.dueDate);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false });
});
watch(() => props.task.notes, (value) => { notes.value = value || ""; });

async function saveNotes() {
  const value = notes.value.slice(0, 255);
  notes.value = value;
  if (value !== props.task.notes) await taskStore.update(props.task.id, { notes: value });
}
async function toggle() { await taskStore.setStatus(props.task.id, isDone.value ? "todo" : "done"); }
async function cyclePriority() { await taskStore.setPriority(props.task.id, priority.value >= 4 ? 0 : priority.value + 1); }
async function del() { await taskStore.softDelete(props.task.id); }
function onPointerDown(event: PointerEvent) {
  if (event.pointerType === "mouse" && event.button !== 0) return;
  if ((event.target as HTMLElement).closest("button")) return;
  revealed.value = false;
  pointerId.value = event.pointerId;
  pointerStartX.value = event.clientX;
  pointerStartY.value = event.clientY;
  swiping.value = false;
  pointerMoved.value = false;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}
function onPointerMove(event: PointerEvent) {
  if (pointerId.value !== event.pointerId) return;
  const dx = event.clientX - pointerStartX.value;
  const dy = event.clientY - pointerStartY.value;
  if (Math.hypot(dx, dy) > 6) pointerMoved.value = true;
  if (!swiping.value && Math.abs(dy) > Math.abs(dx) && Math.abs(dy) > 6) return;
  if (dx <= 0 && !swiping.value) return;
  if (Math.abs(dx) > 6) {
    swiping.value = true;
    event.preventDefault();
  }
  swipeX.value = Math.min(deleteWidth, Math.max(0, dx));
}
async function onPointerUp(event: PointerEvent) {
  if (pointerId.value !== event.pointerId) return;
  pointerId.value = null;
  const shouldReveal = swiping.value && swipeX.value >= deleteWidth * 0.4;
  if (swiping.value || pointerMoved.value) {
    suppressClick.value = true;
    window.setTimeout(() => { suppressClick.value = false; }, 220);
  }
  revealed.value = shouldReveal;
  swipeX.value = shouldReveal ? deleteWidth : 0;
  swiping.value = false;
  pointerMoved.value = false;
}
function onTaskClick() {
  if (suppressClick.value || swipeX.value !== 0) return;
  expanded.value = !expanded.value;
}
function closeSwipe(event: PointerEvent) {
  const target = event.target as Node;
  if (!target || (target as HTMLElement).closest?.(`[data-task-id="${props.task.id}"]`)) return;
  revealed.value = false;
  swipeX.value = 0;
  expanded.value = false;
}
onMounted(() => document.addEventListener("pointerdown", closeSwipe));
onBeforeUnmount(() => document.removeEventListener("pointerdown", closeSwipe));
</script>

<template>
  <div class="task-wrap" :class="{ expanded }" :data-task-id="task.id">
    <div class="task-surface">
      <button class="swipe-delete" :class="{ revealed }" :style="{ opacity: Math.min(1, swipeX / deleteWidth) }" @click.stop="del" aria-label="删除任务" title="删除任务">
        <TrashCan16 />
      </button>
      <div class="task" :title="`更新时间：${updatedLabel}`" :class="{ done: isDone, swiping }" :style="{ background: isDone ? 'linear-gradient(90deg, #d7f0dc 0%, #f3fbf4 100%)' : (category ? category.color + '14' : 'transparent'), transform: `translateX(${swipeX}px)` }" @click="onTaskClick" @pointerdown="onPointerDown" @pointermove="onPointerMove" @pointerup="onPointerUp" @pointercancel="onPointerUp">
      <div class="color-bar" :style="{ background: category?.color || '#9B9B9B' }"></div>
      <button class="check" @click.stop="toggle" :aria-label="isDone ? '标记未完成' : '标记完成'">{{ isDone ? "✓" : "○" }}</button>
       <button class="check-modern" @click.stop="toggle" :aria-label="isDone ? '标记未完成' : '标记完成'" :title="isDone ? '标记未完成' : '标记完成'"><component :is="isDone ? CheckboxChecked16 : Checkbox16" /></button>
       <div class="content">
        <span class="title">{{ task.title }}</span>
        <span v-if="dueLabel" class="updated-at">结束：{{ dueLabel }}</span>
      </div>
      <button class="priority-tag" :class="`priority-${priority}`" @click.stop="cyclePriority" title="调整优先级" aria-label="调整优先级">
        P{{ priority }}
      </button>
      </div>
    </div>
    <div v-if="expanded" class="notes-panel" @click.stop>
      <textarea v-model="notes" maxlength="255" placeholder="添加备注..." @blur="saveNotes(); expanded = false" />
      <span class="notes-count">{{ notes.length }}/255</span>
    </div>
  </div>
</template>

<style scoped>
.task-wrap { margin: 2px 6px; position:relative; }
.task-surface { position:relative; overflow:hidden; border-radius:6px; }
.task { display:flex; align-items:center; gap:6px; padding:6px 2px 6px 0; border-radius:6px; position:relative; cursor:pointer; touch-action:pan-y; will-change:transform, background; transition:background 180ms ease; }
.task:not(.swiping) { transition:transform 180ms ease, background 180ms ease; }
.swipe-delete { position:absolute; inset:0 auto 0 0; width:56px; height:100%; display:flex; align-items:center; justify-content:center; background:var(--danger); color:#fff; border-radius:6px 0 0 6px; opacity:0; pointer-events:none; transition:opacity 80ms linear; }
.swipe-delete.revealed { pointer-events:auto; }
.swipe-delete :deep(svg) { width:16px; height:16px; }
.color-bar { width:3px; height:24px; border-radius:2px; flex-shrink:0; }
.check { display:none; }
.check-modern { display:inline-flex; align-items:center; justify-content:center; width:22px; height:22px; color:var(--text-soft); border-radius:4px; flex-shrink:0; }
.check-modern:hover { background:var(--bg-soft); color:var(--primary); }
.task.done .check-modern { color:var(--primary); }
.check-modern :deep(svg) { width:16px; height:16px; }
.content { flex:1; min-width:0; }
.title { font-size:13px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; display:block; }
.updated-at { display:block; margin-top:2px; color:var(--text-soft); font-size:10px; line-height:1.2; }
.task.done .title { text-decoration:line-through; color:var(--text-soft); }
.priority-tag { display:inline-flex; align-items:center; justify-content:center; min-width:26px; height:18px; padding:0 5px; border-radius:4px; font-size:10px; font-weight:600; line-height:1; flex-shrink:0; }
.priority-0 { background:#fde2e2; color:#b42318; }
.priority-1 { background:#ffead5; color:#b54708; }
.priority-2 { background:#fff4cc; color:#946200; }
.priority-3 { background:#dcecff; color:#175cd3; }
.priority-4 { background:#eceff1; color:#5f6368; }
.notes-panel { position:relative; width:100%; padding:4px 0 8px; }
.notes-panel textarea { width:100%; min-height:48px; max-height:100px; resize:vertical; padding:6px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); font:inherit; font-size:12px; line-height:1.4; }
.notes-count { position:absolute; right:4px; bottom:12px; font-size:10px; color:var(--text-soft); }
</style>
