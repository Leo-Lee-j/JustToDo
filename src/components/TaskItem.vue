<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import type { Task } from "@/types";
import { useTaskStore } from "@/stores/taskStore";
import { useTabStore } from "@/stores/tabStore";
import { useConfigStore } from "@/stores/configStore";
import TrashCan16 from "@carbon/icons-vue/lib/trash-can/16.js";
import Checkbox16 from "@carbon/icons-vue/lib/checkbox/16.js";
import CheckboxChecked16 from "@carbon/icons-vue/lib/checkbox--checked/16.js";

const props = defineProps<{ task: Task; sortable?: boolean; selecting?: boolean }>();
const emit = defineEmits<{ longPress: [] }>();
const taskStore = useTaskStore();
const tabStore = useTabStore();
const configStore = useConfigStore();
const showCheckbox = computed(() => !props.selecting && configStore.config.general.taskCompletionMode !== "gesture");
const gesturesEnabled = computed(() => configStore.config.general.taskCompletionMode !== "checkbox");
const expanded = ref(false);
const editTitleInput = ref<HTMLInputElement>();
const editorForm = ref<HTMLFormElement>();
const draft = ref({ title: "", notes: "", due: "", reminder: "", reminderEnabled: false, status: props.task.status, categoryId: "", tabId: "", version: "" });
const busy = ref(false);
const error = ref("");
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
let longPressTimer: ReturnType<typeof setTimeout> | undefined;
let longPressed = false;
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
const reminderLabel = computed(() => {
  if (!props.task.reminderEnabled || !props.task.reminderTime) return "";
  const date = new Date(props.task.reminderTime);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false });
});
const dueStatus = computed(() => {
  if (isDone.value || props.task.status === "cancelled") return "normal";
  const hours = (Date.parse(props.task.dueDate ?? "") - Date.now()) / 3_600_000;
  return hours < 0 ? "overdue" : hours <= 24 ? "urgent" : hours <= 72 ? "soon" : "normal";
});
function localDate(raw: string | null) {
  if (!raw) return "";
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return "";
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16);
}

async function run(action: () => Promise<unknown>) {
  if (busy.value) return false;
  busy.value = true;
  error.value = "";
  try {
    await action();
    return true;
  } catch (err) {
    error.value = String(err);
    return false;
  } finally {
    busy.value = false;
  }
}

async function saveTask() {
  const { title, notes, status, categoryId, tabId, due, reminder, reminderEnabled, version } = draft.value;
  if (!title.trim()) return;
  if (reminderEnabled && !reminder) {
    error.value = "启用提醒时，请设置提醒时间";
    return;
  }
  const saved = await run(() => taskStore.update(props.task.id, {
    title, notes, status, categoryId, tabId,
    ...(due !== localDate(props.task.dueDate) ? { dueDate: due ? new Date(due).toISOString() : null } : {}),
    reminderEnabled,
    reminderTime: reminder ? new Date(reminder).toISOString() : null,
  }, version));
  if (saved) expanded.value = false;
}

function toggle() { return run(() => taskStore.setStatus(props.task.id, isDone.value ? "todo" : "done")); }
function cyclePriority() { return run(() => taskStore.setPriority(props.task.id, priority.value >= 4 ? 0 : priority.value + 1)); }
function del() { return run(() => taskStore.softDelete(props.task.id)); }
function onPointerDown(event: PointerEvent) {
  if (event.pointerType === "mouse" && event.button !== 0) return;
  const target = event.target as HTMLElement;
  if (props.selecting || target.closest("button:not(.content)")) return;
  revealed.value = false;
  pointerId.value = event.pointerId;
  pointerStartX.value = event.clientX;
  pointerStartY.value = event.clientY;
  swiping.value = false;
  pointerMoved.value = false;
  longPressed = false;
  target.setPointerCapture(event.pointerId);
  const title = !!target.closest(".content");
  clearTimeout(longPressTimer);
  longPressTimer = setTimeout(() => {
    longPressed = true;
    if (title) void openEditor();
    else emit("longPress");
  }, title ? 600 : 800);
}
function onPointerMove(event: PointerEvent) {
  if (pointerId.value !== event.pointerId || longPressed) return;
  const dx = event.clientX - pointerStartX.value;
  const dy = event.clientY - pointerStartY.value;
  if (Math.hypot(dx, dy) > 6) {
    pointerMoved.value = true;
    clearTimeout(longPressTimer);
  }
  if (!gesturesEnabled.value) return;
  if (!swiping.value && Math.abs(dy) > Math.abs(dx) && Math.abs(dy) > 6) return;
  if (Math.abs(dx) > 6) {
    swiping.value = true;
    event.preventDefault();
  }
  swipeX.value = Math.min(deleteWidth, Math.max(-deleteWidth, dx));
}
function onPointerUp(event: PointerEvent) {
  if (pointerId.value !== event.pointerId) return;
  pointerId.value = null;
  clearTimeout(longPressTimer);
  const gesture = event.type === "pointerup" && swiping.value && !longPressed && gesturesEnabled.value;
  const shouldReveal = gesture && swipeX.value <= -deleteWidth * 0.4;
  if (gesture && swipeX.value >= deleteWidth * 0.4) void toggle();
  if (swiping.value || pointerMoved.value || longPressed) {
    suppressClick.value = true;
    window.setTimeout(() => { suppressClick.value = false; }, 220);
  }
  revealed.value = shouldReveal;
  swipeX.value = shouldReveal ? -deleteWidth : 0;
  swiping.value = false;
  pointerMoved.value = false;
}
function onTaskClick(event: MouseEvent) {
  if (suppressClick.value) { event.stopPropagation(); return; }
  if (props.selecting || swipeX.value !== 0) return;
  if (expanded.value) expanded.value = false;
  else void openEditor();
}
async function openEditor() {
  if (expanded.value) return;
  expanded.value = true;
  error.value = "";
  const { title, notes, status, categoryId, tabId, dueDate, reminderTime, reminderEnabled, updatedAt } = props.task;
  draft.value = { title, notes, status, categoryId, tabId, due: localDate(dueDate), reminder: localDate(reminderTime), reminderEnabled, version: updatedAt };
  await nextTick();
  editTitleInput.value?.focus();
  editorForm.value?.scrollIntoView({ block: "end" });
}
function closeSwipe(event: PointerEvent) {
  const target = event.target as Node;
  if (!target || (target as HTMLElement).closest?.(`[data-task-id="${props.task.id}"]`)) return;
  revealed.value = false;
  swipeX.value = 0;
}
onMounted(() => document.addEventListener("pointerdown", closeSwipe));
onBeforeUnmount(() => {
  clearTimeout(longPressTimer);
  document.removeEventListener("pointerdown", closeSwipe);
});
</script>

<template>
  <div class="task-wrap" :class="{ expanded }" :data-task-id="task.id">
    <div class="task-surface">
      <button v-if="!selecting" class="swipe-delete" :class="{ revealed }" :tabindex="revealed ? 0 : -1" :style="{ opacity: Math.min(1, Math.max(0, -swipeX / deleteWidth)) }" @click.stop="del" aria-label="删除任务" title="删除任务">
        <TrashCan16 />
      </button>
      <div class="task" :title="`更新时间：${updatedLabel}`" :class="{ done: isDone, swiping }" :style="{ background: isDone ? 'linear-gradient(90deg, #d7f0dc 0%, #f3fbf4 100%)' : (category ? category.color + '14' : 'transparent'), transform: `translateX(${swipeX}px)` }" @click="onTaskClick" @pointerdown="onPointerDown" @pointermove="onPointerMove" @pointerup="onPointerUp" @pointercancel="onPointerUp">
      <div class="color-bar" :style="{ background: category?.color || '#9B9B9B' }"></div>
      <button v-if="showCheckbox" class="check-modern" :disabled="busy || expanded" @click.stop="toggle" :aria-label="isDone ? '标记未完成' : '标记完成'" :title="isDone ? '标记未完成' : '标记完成'"><component :is="isDone ? CheckboxChecked16 : Checkbox16" /></button>
      <button type="button" class="content" :tabindex="selecting ? -1 : 0" :aria-expanded="expanded" :title="selecting ? '选择任务' : '编辑任务'">
        <span class="title">{{ task.title }}</span>
        <span v-if="dueLabel" class="updated-at" :class="'due-' + dueStatus">结束：{{ dueLabel }}</span>
        <span v-if="reminderLabel" class="updated-at reminder-label">提醒：{{ reminderLabel }}</span>
      </button>
      <button v-if="!selecting" class="priority-tag" :disabled="busy || expanded" :class="'priority-' + priority" @click.stop="cyclePriority" title="调整优先级" aria-label="调整优先级">
        P{{ priority }}
      </button>
      <button v-if="sortable && !selecting" type="button" tabindex="-1" class="drag-handle" title="拖动排序" aria-label="拖动排序" @click.stop>⠿</button>
      </div>
    </div>
    <form v-if="expanded && !selecting" ref="editorForm" class="task-editor" @submit.prevent="saveTask" @keydown.esc.stop="expanded = false">
      <label>任务标题<input ref="editTitleInput" v-model="draft.title" maxlength="255" required /></label>
      <label>到期时间<input v-model="draft.due" type="datetime-local" /></label>
      <label class="reminder-toggle"><input v-model="draft.reminderEnabled" type="checkbox" /> 单独提醒</label>
      <label>提醒时间
        <span class="reminder-input">
          <input v-model="draft.reminder" type="datetime-local" :required="draft.reminderEnabled" :disabled="!draft.reminderEnabled" />
          <button type="button" :disabled="!draft.reminder" @click="draft.reminder = ''">清除</button>
        </span>
      </label>
      <label>状态<select v-model="draft.status" aria-label="状态"><option value="todo">待办</option><option value="doing">进行中</option><option value="done">已完成</option><option value="cancelled">已取消</option></select></label>
      <label>分类<select v-model="draft.categoryId" aria-label="分类"><option v-for="item in tabStore.categories" :key="item.id" :value="item.id">{{ item.name }}</option></select></label>
      <label>所属 Tab<select v-model="draft.tabId" aria-label="所属 Tab"><option v-for="item in tabStore.sortedTabs" :key="item.id" :value="item.id">{{ item.name }}</option></select></label>
      <label>备注<textarea v-model="draft.notes" maxlength="255" /></label>
      <div class="editor-actions">
        <button type="button" class="delete-btn" :disabled="busy" @click="del">删除</button>
        <button type="button" :disabled="busy" @click="expanded = false; error = ''">取消</button>
        <button type="submit" :disabled="busy || !draft.title.trim()">保存</button>
      </div>
    </form>
    <p v-if="error" class="task-error" role="alert">{{ error }}</p>
  </div>
</template>

<style scoped>
.task-wrap { margin: 2px 6px; position:relative; }
.task-surface { position:relative; overflow:hidden; border-radius:6px; }
.task { display:flex; align-items:center; gap:6px; padding:6px 2px 6px 0; border-radius:6px; position:relative; cursor:pointer; touch-action:pan-y; will-change:transform, background; transition:background 180ms ease; }
.task:not(.swiping) { transition:transform 180ms ease, background 180ms ease; }
.swipe-delete { position:absolute; inset:0 0 0 auto; width:56px; height:100%; display:flex; align-items:center; justify-content:center; background:var(--danger); color:#fff; border-radius:0 6px 6px 0; opacity:0; pointer-events:none; transition:opacity 80ms linear; }
.swipe-delete.revealed { pointer-events:auto; }
.swipe-delete :deep(svg) { width:16px; height:16px; }
.color-bar { width:3px; height:24px; border-radius:2px; flex-shrink:0; }
.check-modern { display:inline-flex; align-items:center; justify-content:center; width:22px; height:22px; color:var(--text-soft); border-radius:4px; flex-shrink:0; }
.check-modern:hover { background:var(--bg-soft); color:var(--primary); }
.task.done .check-modern { color:var(--primary); }
.check-modern :deep(svg) { width:16px; height:16px; }
.content { flex:1; min-width:0; text-align:left; }
.drag-handle { cursor:grab; padding:0 4px; color:var(--text-soft); touch-action:none; }
.drag-handle:active { cursor:grabbing; }
.title { font-size:13px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; display:block; }
.updated-at { display:block; margin-top:2px; color:var(--text-soft); font-size:10px; line-height:1.2; }
.due-overdue { color:var(--danger); }
.due-urgent { color:#b54708; }
.due-soon { color:#946200; }
.reminder-label { color:var(--primary); }
.task.done .title { text-decoration:line-through; color:var(--text-soft); }
.priority-tag { display:inline-flex; align-items:center; justify-content:center; min-width:26px; height:18px; padding:0 5px; border-radius:4px; font-size:10px; font-weight:600; line-height:1; flex-shrink:0; }
.priority-0 { background:#fde2e2; color:#b42318; }
.priority-1 { background:#ffead5; color:#b54708; }
.priority-2 { background:#fff4cc; color:#946200; }
.priority-3 { background:#dcecff; color:#175cd3; }
.priority-4 { background:#eceff1; color:#5f6368; }
.task-editor { display:grid; grid-template-columns:repeat(2, minmax(0, 1fr)); gap:6px; padding:8px; background:var(--bg-soft); border-radius:4px; font-size:12px; }
.task-editor label { display:grid; gap:3px; min-width:0; }
.task-editor label:first-child, .task-editor label:has(textarea), .editor-actions { grid-column:1 / -1; }
.task-editor .reminder-toggle { display:flex; align-items:center; gap:6px; }
.task-editor .reminder-toggle input { width:auto; }
.reminder-input { display:flex; gap:4px; min-width:0; }
.reminder-input input { flex:1; }
.reminder-input button { flex:none; padding:0 5px; border:1px solid var(--border); border-radius:4px; background:var(--bg); }
.reminder-input button:disabled { opacity:.45; }
.task-editor input, .task-editor textarea, .task-editor select { width:100%; min-width:0; padding:4px 5px; border:1px solid var(--border); border-radius:4px; background:var(--bg); color:var(--text); font:inherit; }
.task-editor textarea { min-height:42px; resize:vertical; }
.editor-actions { display:flex; gap:8px; justify-content:flex-end; }
.editor-actions button { padding:4px 8px; background:var(--bg); border:1px solid var(--border); border-radius:4px; }
.editor-actions .delete-btn { margin-right:auto; color:var(--danger); }
.task-error { padding:4px 8px; color:var(--danger); overflow-wrap:anywhere; font-size:12px; }
</style>
