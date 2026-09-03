<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Task } from "@/types";
import { useTaskStore } from "@/stores/taskStore";
import { useTabStore } from "@/stores/tabStore";
import { useConfigStore } from "@/stores/configStore";
import { useToast } from "@/composables/useToast";
import TrashCan16 from "@carbon/icons-vue/lib/trash-can/16.js";
import Checkbox16 from "@carbon/icons-vue/lib/checkbox/16.js";
import CheckboxChecked16 from "@carbon/icons-vue/lib/checkbox--checked/16.js";

const props = defineProps<{ task: Task; multiSelectMode?: boolean }>();
const taskStore = useTaskStore();
const tabStore = useTabStore();
const configStore = useConfigStore();
const { showToast } = useToast();
const expanded = ref(false);
const editing = ref(false);
const editedTitle = ref("");
const notes = ref(props.task.notes || "");
const category = computed(() => tabStore.categoryMap[props.task.categoryId]);
const isDone = computed(() => props.task.status === "done");
const showCheckbox = computed(() => {
  const mode = configStore.config.general.taskCompletionMode;
  return mode === "checkbox" || mode === "both";
});
const enableGesture = computed(() => {
  const mode = configStore.config.general.taskCompletionMode;
  return mode === "gesture" || mode === "both";
});
const priority = computed(() => Math.max(0, Math.min(4, props.task.priority)));
const swipeX = ref(0);
const swipeDirection = ref<"left" | "right" | null>(null);
const swiping = ref(false);
const pointerStartX = ref(0);
const pointerStartY = ref(0);
const pointerId = ref<number | null>(null);
const suppressClick = ref(false);
const pointerMoved = ref(false);
const revealed = ref(false);
const deleteWidth = 56;
const longPressTimer = ref<number | null>(null);
const longPressTriggered = ref(false);
const emit = defineEmits<{
  longPress: [];
}>();
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
const dueStatus = computed(() => {
  if (!props.task.dueDate) return "normal";
  const date = new Date(props.task.dueDate);
  if (Number.isNaN(date.getTime())) return "normal";
  const now = new Date();
  const diff = date.getTime() - now.getTime();
  const hours = diff / (1000 * 60 * 60);

  if (diff < 0) return "overdue"; // 已过期
  if (hours <= 24) return "urgent"; // 24小时内
  if (hours <= 72) return "soon"; // 3天内
  return "normal";
});
watch(() => props.task.notes, (value) => { notes.value = value || ""; });

async function saveNotes() {
  const value = notes.value.slice(0, 255);
  notes.value = value;
  if (value !== props.task.notes) await taskStore.update(props.task.id, { notes: value });
}
async function saveTitle() {
  const value = editedTitle.value.trim();
  if (!value) {
    editing.value = false;
    return;
  }
  if (value !== props.task.title) {
    await taskStore.update(props.task.id, { title: value });
  }
  editing.value = false;
}
function cancelEdit() {
  editing.value = false;
  editedTitle.value = "";
}
async function toggle() {
  const willBeDone = !isDone.value;

  // 添加完成动画
  if (willBeDone) {
    const checkbox = document.querySelector(`[data-task-id="${props.task.id}"] .check-modern`) as HTMLElement;
    if (checkbox) {
      checkbox.classList.add("completing");
      setTimeout(() => checkbox.classList.remove("completing"), 400);
    }
  }

  await taskStore.setStatus(props.task.id, isDone.value ? "todo" : "done");
}
async function cyclePriority() {
  const newPriority = priority.value >= 4 ? 0 : priority.value + 1;

  // 添加动画类
  const btn = document.querySelector(`[data-task-id="${props.task.id}"] .priority-tag`) as HTMLElement;
  if (btn) {
    btn.classList.add("priority-cycling");
    setTimeout(() => btn.classList.remove("priority-cycling"), 300);
  }

  await taskStore.setPriority(props.task.id, newPriority);
}
async function del() {
  const taskId = props.task.id;

  // 添加淡出动画
  const taskWrap = document.querySelector(`[data-task-id="${taskId}"]`) as HTMLElement;
  if (taskWrap) {
    taskWrap.classList.add("deleting");
    await new Promise(resolve => setTimeout(resolve, 300));
  }

  await taskStore.softDelete(taskId);
  showToast({
    message: "任务已删除",
    type: "success",
    duration: 5000,
    action: {
      label: "撤销",
      handler: async () => {
        await taskStore.restore(taskId);
      },
    },
  });
}
function onPointerDown(event: PointerEvent) {
  if (event.pointerType === "mouse" && event.button !== 0) return;
  if ((event.target as HTMLElement).closest("button")) return;
  if (props.multiSelectMode) return; // 多选模式下禁用手势

  // 检查是否在标题上长按
  const isOnTitle = (event.target as HTMLElement).closest(".title");

  revealed.value = false;
  swipeDirection.value = null;
  pointerId.value = event.pointerId;
  pointerStartX.value = event.clientX;
  pointerStartY.value = event.clientY;
  swiping.value = false;
  pointerMoved.value = false;
  longPressTriggered.value = false;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);

  // 标题长按 600ms 触发编辑模式
  if (isOnTitle) {
    if (longPressTimer.value) clearTimeout(longPressTimer.value);
    longPressTimer.value = window.setTimeout(() => {
      if (!pointerMoved.value && pointerId.value === event.pointerId) {
        longPressTriggered.value = true;
        editing.value = true;
        editedTitle.value = props.task.title;
        suppressClick.value = true;
        // 下一帧聚焦输入框
        window.requestAnimationFrame(() => {
          const input = document.querySelector<HTMLInputElement>(`[data-task-id="${props.task.id}"] .title-input`);
          input?.focus();
          input?.select();
        });
      }
    }, 600);
  } else {
    // 非标题区域长按 800ms 触发多选模式
    if (longPressTimer.value) clearTimeout(longPressTimer.value);
    longPressTimer.value = window.setTimeout(() => {
      if (!pointerMoved.value && pointerId.value === event.pointerId) {
        longPressTriggered.value = true;
        emit("longPress");
        suppressClick.value = true;
      }
    }, 800);
  }
}
function onPointerMove(event: PointerEvent) {
  if (pointerId.value !== event.pointerId) return;
  if (props.multiSelectMode) return; // 多选模式下禁用滑动
  if (!enableGesture.value) return; // 手势禁用时不处理滑动

  const dx = event.clientX - pointerStartX.value;
  const dy = event.clientY - pointerStartY.value;
  if (Math.hypot(dx, dy) > 6) {
    pointerMoved.value = true;
    // 移动后取消长按计时器
    if (longPressTimer.value) {
      clearTimeout(longPressTimer.value);
      longPressTimer.value = null;
    }
  }
  // 垂直滑动优先：如果垂直位移大于横向且超过12px，放弃横向手势
  if (!swiping.value && Math.abs(dy) > Math.abs(dx) && Math.abs(dy) > 12) return;

  // 确定滑动方向
  if (!swiping.value && Math.abs(dx) > 6) {
    swipeDirection.value = dx > 0 ? "right" : "left";
    swiping.value = true;
    event.preventDefault();
  }

  if (!swiping.value) return;

  // 左滑删除，右滑完成/取消
  if (swipeDirection.value === "left") {
    swipeX.value = Math.max(-deleteWidth, Math.min(0, dx));
  } else if (swipeDirection.value === "right") {
    swipeX.value = Math.max(0, Math.min(deleteWidth, dx));
  }
}
async function onPointerUp(event: PointerEvent) {
  if (pointerId.value !== event.pointerId) return;
  pointerId.value = null;

  // 清除长按计时器
  if (longPressTimer.value) {
    clearTimeout(longPressTimer.value);
    longPressTimer.value = null;
  }

  // 如果触发了长按编辑，不执行其他逻辑
  if (longPressTriggered.value) {
    longPressTriggered.value = false;
    swiping.value = false;
    pointerMoved.value = false;
    swipeDirection.value = null;
    return;
  }

  // 左滑删除逻辑
  const shouldRevealDelete = enableGesture.value && swipeDirection.value === "left" && swipeX.value <= -deleteWidth * 0.4;

  // 右滑完成/取消逻辑
  const shouldToggle = enableGesture.value && swipeDirection.value === "right" && swipeX.value >= deleteWidth * 0.4;

  if (swiping.value || pointerMoved.value) {
    suppressClick.value = true;
    window.setTimeout(() => { suppressClick.value = false; }, 220);
  }

  // 执行右滑完成/取消
  if (shouldToggle) {
    await toggle();
  }

  revealed.value = shouldRevealDelete;
  swipeX.value = shouldRevealDelete ? -deleteWidth : 0;
  swiping.value = false;
  pointerMoved.value = false;
  swipeDirection.value = null;
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
  editing.value = false;
}
onMounted(() => document.addEventListener("pointerdown", closeSwipe));
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", closeSwipe);
  if (longPressTimer.value) clearTimeout(longPressTimer.value);
});
</script>

<template>
  <div class="task-wrap" :class="{ expanded }" :data-task-id="task.id" tabindex="0">
    <div class="task-surface">
      <button class="swipe-delete" :class="{ revealed }" :style="{ opacity: swipeX < 0 ? Math.min(1, Math.abs(swipeX) / deleteWidth) : 0 }" @click.stop="del" aria-label="删除任务" title="删除任务">
        <TrashCan16 />
      </button>
      <div class="task" :title="`更新时间:${updatedLabel}`" :class="{ done: isDone, swiping }" :style="{
        backgroundImage: category && !isDone ? `linear-gradient(to right, ${category.color}1a, ${category.color}0d)` : 'none',
        transform: `translateX(${swipeX}px)`
      }" @click="onTaskClick" @pointerdown="onPointerDown" @pointermove="onPointerMove" @pointerup="onPointerUp" @pointercancel="onPointerUp">
      <div class="color-bar" :style="{ background: category?.color || '#9B9B9B' }"></div>
       <button v-if="showCheckbox" class="check-modern" @click.stop="toggle" :aria-label="isDone ? '标记未完成' : '标记完成'" :title="isDone ? '标记未完成' : '标记完成'"><component :is="isDone ? CheckboxChecked16 : Checkbox16" /></button>
       <div class="content">
        <input
          v-if="editing"
          v-model="editedTitle"
          class="title-input"
          type="text"
          maxlength="100"
          @keydown.enter="saveTitle"
          @keydown.esc="cancelEdit"
          @blur="saveTitle"
          @click.stop
        />
        <span v-else class="title">{{ task.title }}</span>
        <span v-if="dueLabel" class="updated-at due-date" :class="`due-${dueStatus}`">结束：{{ dueLabel }}</span>
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
.task-wrap { margin: 2px 6px; position:relative; transition: opacity 300ms ease, transform 300ms ease; }
.task-wrap.deleting {
  opacity: 0;
  transform: translateX(-20px) scale(0.95);
  pointer-events: none;
}
.task-surface { position:relative; overflow:hidden; border-radius:6px; }
.task { display:flex; align-items:center; gap:6px; padding:6px 2px 6px 0; border-radius:6px; position:relative; cursor:pointer; touch-action:pan-y; will-change:transform, background; transition:background 180ms ease; background:var(--bg); z-index:1; overflow:hidden; }
.task:not(.swiping) { transition:transform 180ms ease, background 180ms ease; }
.task::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: linear-gradient(90deg, #d7f0dc 0%, #f3fbf4 100%);
  transform: translateX(-100%);
  z-index: -1;
  pointer-events: none;
}
.task.done::before {
  transform: translateX(0);
}
.swipe-delete { position:absolute; inset:0 0 0 auto; width:56px; height:100%; display:flex; align-items:center; justify-content:center; background:var(--danger); color:#fff; border-radius:0; opacity:0; pointer-events:none; transition:opacity 80ms linear; z-index:0; }
.swipe-delete.revealed { pointer-events:auto; }
.swipe-delete :deep(svg) { width:16px; height:16px; }
.color-bar { width:3px; height:24px; border-radius:2px; flex-shrink:0; }
.check { display:none; }
.check-modern { display:inline-flex; align-items:center; justify-content:center; width:22px; height:22px; color:var(--text-soft); border-radius:4px; flex-shrink:0; transition:transform 180ms ease, background 180ms ease, color 180ms ease; }
.check-modern:hover { background:var(--bg-soft); color:var(--primary); }
.check-modern.completing {
  animation: checkbox-complete 400ms cubic-bezier(0.34, 1.56, 0.64, 1);
}
@keyframes checkbox-complete {
  0% { transform: scale(1) rotate(0deg); }
  30% { transform: scale(0.8) rotate(-10deg); }
  60% { transform: scale(1.15) rotate(5deg); }
  100% { transform: scale(1) rotate(0deg); }
}
.task.done .check-modern { color:var(--primary); }
.check-modern :deep(svg) { width:16px; height:16px; }
.content { flex:1; min-width:0; }
.title { font-size:13px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; display:block; }
.title-input {
  font-size:13px;
  width:100%;
  padding:2px 4px;
  border:1px solid var(--primary);
  border-radius:4px;
  background:var(--bg-soft);
  font:inherit;
  outline:none;
}
.updated-at { display:block; margin-top:2px; color:var(--text-soft); font-size:10px; line-height:1.2; }
.due-date.due-overdue {
  color: #d32f2f;
  font-weight: 600;
}
.due-date.due-urgent {
  color: #f57c00;
  font-weight: 600;
}
.due-date.due-soon {
  color: #fbc02d;
  font-weight: 500;
}
.task.done .title { text-decoration:line-through; color:var(--text-soft); }
.priority-tag { display:inline-flex; align-items:center; justify-content:center; min-width:26px; height:18px; padding:0 5px; border-radius:4px; font-size:10px; font-weight:600; line-height:1; flex-shrink:0; transition:transform 180ms cubic-bezier(0.34, 1.56, 0.64, 1), background 180ms ease; }
.priority-tag.priority-cycling {
  animation: priority-cycle 300ms cubic-bezier(0.34, 1.56, 0.64, 1);
}
@keyframes priority-cycle {
  0% { transform: scale(1) rotate(0deg); }
  50% { transform: scale(1.2) rotate(8deg); }
  100% { transform: scale(1) rotate(0deg); }
}
.priority-0 { background:#fde2e2; color:#b42318; }
.priority-1 { background:#ffead5; color:#b54708; }
.priority-2 { background:#fff4cc; color:#946200; }
.priority-3 { background:#dcecff; color:#175cd3; }
.priority-4 { background:#eceff1; color:#5f6368; }
.notes-panel { position:relative; width:100%; padding:4px 0 8px; }
.notes-panel textarea { width:100%; min-height:48px; max-height:100px; resize:vertical; padding:6px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); font:inherit; font-size:12px; line-height:1.4; }
.notes-count { position:absolute; right:4px; bottom:12px; font-size:10px; color:var(--text-soft); }
</style>
