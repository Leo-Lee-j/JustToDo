<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref, watch } from "vue";
import draggable from "vuedraggable";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import Search16 from "@carbon/icons-vue/lib/search/16.js";
import RecentlyViewed16 from "@carbon/icons-vue/lib/recently-viewed/16.js";
import Close16 from "@carbon/icons-vue/lib/close/16.js";
import TrashCan16 from "@carbon/icons-vue/lib/trash-can/16.js";
import TextAlignLeft16 from "@carbon/icons-vue/lib/text--align--left/16.js";
import Settings16 from "@carbon/icons-vue/lib/settings/16.js";
import Subtract16 from "@carbon/icons-vue/lib/subtract/16.js";
import { useTaskStore } from "@/stores/taskStore";
import { useTabStore } from "@/stores/tabStore";
import { useConfigStore } from "@/stores/configStore";
import type { Tab, Task } from "@/types";
import TaskItem from "./TaskItem.vue";
import SettingsWindow from "./SettingsWindow.vue";
import TrashView from "./TrashView.vue";
import { installWindowShortcuts } from "@/utils/shortcuts";

const taskStore = useTaskStore();
const tabStore = useTabStore();
const configStore = useConfigStore();
const windowApi = getCurrentWindow();
const view = ref<"tasks" | "history" | "trash">("tasks");
const showSettings = ref(false);
const showSearch = ref(false);
const search = ref("");
const searchInput = ref<HTMLInputElement>();
const newTitle = ref("");
const newNotes = ref("");
const newPriority = ref(2);
const newDueDate = ref("");
const taskTitleInput = ref<HTMLInputElement>();
const taskListEl = ref<HTMLElement>();
const showComposerNotes = ref(false);
const creatingTask = ref(false);
const tabEditor = ref<{ id: string | null; name: string }>();
const tabNameInput = ref<HTMLInputElement>();
const editingTab = ref(false);
const confirmDeleteTabId = ref<string>();
const error = ref("");
const ordering = ref(false);
const draggingTab = ref<string>();
const collapsed = ref(false);
const changingSize = ref(false);
const selecting = ref(false);
const selected = ref(new Set<string>());
const batchBusy = ref(false);
const pendingCount = computed(() => taskStore.tasks.filter((task) => !task.deletedAt && (task.status === "todo" || task.status === "doing")).length);
const activeTasks = computed(() => taskStore.tasks.filter((task) => !task.deletedAt && task.tabId === tabStore.activeTabId).sort((a, b) => a.order - b.order));
const searchResults = computed(() => {
  const query = search.value.trim().toLowerCase();
  return query ? taskStore.tasks.filter((task) => !task.deletedAt && (task.title.toLowerCase().includes(query) || task.notes.toLowerCase().includes(query))) : activeTasks.value;
});
const trashedIds = computed(() => new Set(taskStore.trashed.map((task) => task.id)));
const historyLabels: Record<string, string> = { created: "新增", updated: "更新", deleted: "删除", restored: "恢复" };

function formatTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false });
}

async function perform(action: () => Promise<void>) {
  error.value = "";
  try { await action(); } catch (err) { error.value = String(err); }
}

async function addTask() {
  if (creatingTask.value || !newTitle.value.trim() || !tabStore.activeTabId) return;
  creatingTask.value = true;
  await perform(async () => {
    const dueDate = newDueDate.value ? new Date(newDueDate.value).toISOString() : null;
    const task = await taskStore.create(newTitle.value, tabStore.activeTabId, dueDate, newNotes.value, newPriority.value);
    newTitle.value = "";
    newNotes.value = "";
    newPriority.value = 2;
    newDueDate.value = "";
    showComposerNotes.value = false;
    await nextTick();
    Array.from(taskListEl.value?.querySelectorAll<HTMLElement>("[data-task-id]") ?? [])
      .find((item) => item.dataset.taskId === task.id)?.scrollIntoView({ block: "nearest" });
  });
  creatingTask.value = false;
}

function showTasks() {
  showSettings.value = false;
  view.value = "tasks";
}

let idleTimer: ReturnType<typeof setTimeout> | undefined;
let hoverTimer: ReturnType<typeof setTimeout> | undefined;
let unlistenExpand: (() => void) | undefined;
let expandRequested = false;

async function setCollapsed(value: boolean) {
  if (changingSize.value || value === collapsed.value) return;
  changingSize.value = true;
  try {
    await invoke("set_sticky_collapsed", { collapsed: value });
    collapsed.value = value;
  } catch (err) {
    error.value = String(err);
  } finally {
    changingSize.value = false;
  }
  if (expandRequested) {
    expandRequested = false;
    await setCollapsed(false);
  }
}

function requestExpand() {
  clearTimeout(idleTimer);
  clearTimeout(hoverTimer);
  if (changingSize.value) expandRequested = true;
  else void setCollapsed(false);
}

function onPointerEnter() {
  clearTimeout(idleTimer);
  if (collapsed.value && !changingSize.value) hoverTimer = setTimeout(requestExpand, 300);
}

function onPointerLeave() {
  clearTimeout(hoverTimer);
  clearTimeout(idleTimer);
  if (!configStore.config.window.autoHide || showSettings.value || collapsed.value || changingSize.value) return;
  const delay = Math.max(10, Math.min(120, configStore.config.window.autoHideDelay));
  idleTimer = setTimeout(() => void setCollapsed(true), delay * 1000);
}

watch([() => configStore.config.window.autoHide, showSettings], ([enabled, settingsOpen]) => {
  if (!enabled || settingsOpen) requestExpand();
});

function switchTab(id: string) {
  showTasks();
  exitSelection();
  search.value = "";
  confirmDeleteTabId.value = undefined;
  tabStore.setActive(id);
}

function exitSelection() {
  selecting.value = false;
  selected.value.clear();
}

function selectTask(id: string) {
  if (batchBusy.value) return;
  if (selected.value.has(id)) selected.value.delete(id);
  else selected.value.add(id);
}

function startSelection(id: string) {
  selected.value = new Set([id]);
  selecting.value = true;
}

async function applySelection(action: "complete" | "delete") {
  if (batchBusy.value) return;
  batchBusy.value = true;
  const ids = searchResults.value.filter(({ id }) => selected.value.has(id)).map(({ id }) => id);
  await perform(async () => {
    if (action === "delete") await taskStore.softDeleteMany(ids);
    else for (const id of ids) await taskStore.setStatus(id, "done");
    exitSelection();
  });
  batchBusy.value = false;
}

watch([search, view, showSettings, () => tabStore.activeTabId], exitSelection);

async function focusSearch() {
  showTasks();
  showSearch.value = true;
  await nextTick();
  searchInput.value?.focus();
}

async function focusNewTask() {
  showTasks();
  await nextTick();
  taskTitleInput.value?.focus();
}

async function openTabEditor(tab?: Tab) {
  showTasks();
  tabEditor.value = { id: tab?.id ?? null, name: tab?.name ?? "" };
  await nextTick();
  tabNameInput.value?.focus();
  tabNameInput.value?.select();
}

async function saveTab() {
  const editor = tabEditor.value;
  if (editingTab.value || !editor?.name.trim()) return;
  editingTab.value = true;
  await perform(async () => {
    if (editor.id) await tabStore.rename(editor.id, editor.name);
    else switchTab((await tabStore.create(editor.name)).id);
    tabEditor.value = undefined;
  });
  editingTab.value = false;
}

async function deleteTab(id: string) {
  if (editingTab.value) return;
  if (confirmDeleteTabId.value !== id) {
    confirmDeleteTabId.value = id;
    return;
  }
  editingTab.value = true;
  await perform(async () => {
    await tabStore.remove(id);
    confirmDeleteTabId.value = undefined;
    if (tabEditor.value?.id === id) tabEditor.value = undefined;
  });
  editingTab.value = false;
}

async function restore(id: string) {
  await perform(async () => switchTab((await taskStore.restore(id)).tabId));
}

async function reorder(tasks: Task[]) {
  const tabId = draggingTab.value;
  if (!tabId || ordering.value || tabId !== tabStore.activeTabId) return;
  ordering.value = true;
  await perform(() => taskStore.reorder(tabId, tasks.map((task) => task.id)));
  ordering.value = false;
}

const EDGE_MARGIN = { left: 12, top: 12, right: 8, bottom: 12 };
const SNAP_THRESHOLD = 24;
let snapping = false;
let snapTimer: ReturnType<typeof setTimeout> | undefined;
let unlistenMoved: (() => void) | undefined;
async function snapToEdge(movedPosition?: { x: number; y: number }) {
  if (snapping || collapsed.value || changingSize.value) return;
  const monitor = await currentMonitor();
  if (!monitor) return;
  const [position, size] = await Promise.all([
    movedPosition ? Promise.resolve(movedPosition) : windowApi.outerPosition(),
    windowApi.outerSize(),
  ]);
  // Use the monitor work area so the window does not end up behind the
  // Windows taskbar or outside the visible desktop.
  const area = monitor.workArea;
  const left = area.position.x;
  const top = area.position.y;
  const right = left + area.size.width - size.width;
  const bottom = top + area.size.height - size.height;
  const snapLeft = left + EDGE_MARGIN.left;
  const snapTop = top + EDGE_MARGIN.top;
  const snapRight = right - EDGE_MARGIN.right;
  const snapBottom = bottom - EDGE_MARGIN.bottom;
  const distances = [
    { edge: "left", distance: Math.abs(position.x - snapLeft) },
    { edge: "right", distance: Math.abs(position.x - snapRight) },
    { edge: "top", distance: Math.abs(position.y - snapTop) },
    { edge: "bottom", distance: Math.abs(position.y - snapBottom) },
  ];
  const nearest = distances.reduce((a, b) => (a.distance <= b.distance ? a : b));
  const outside = position.x < left || position.x > right || position.y < top || position.y > bottom;
  let x = position.x;
  let y = position.y;
  if (outside || nearest.distance <= SNAP_THRESHOLD) {
    if (nearest.edge === "left" || position.x < left) x = snapLeft;
    if (nearest.edge === "right" || position.x > right) x = snapRight;
    if (nearest.edge === "top" || position.y < top) y = snapTop;
    if (nearest.edge === "bottom" || position.y > bottom) y = snapBottom;
  }
  // A window dragged past a corner is restored fully into the work area.
  if (outside) {
    x = Math.max(snapLeft, Math.min(snapRight, x));
    y = Math.max(snapTop, Math.min(snapBottom, y));
  }
  if (x === position.x && y === position.y) return;
  snapping = true;
  try {
    await invoke("move_sticky", { x, y });
  } finally {
    snapping = false;
  }
}


watch(() => configStore.config.general.shortcuts, (bindings, _, onCleanup) => {
  onCleanup(installWindowShortcuts({
    newTask: focusNewTask,
    newTab: () => void openTabEditor(),
    search: focusSearch,
    showWindow: () => { requestExpand(); showTasks(); },
    switchTab: (index) => {
      const tab = tabStore.sortedTabs[index - 1];
      if (tab) switchTab(tab.id);
    },
    escape: () => {
      if (selecting.value) exitSelection();
      else if (showSettings.value) showSettings.value = false;
      else if (tabEditor.value) tabEditor.value = undefined;
      else if (showComposerNotes.value) showComposerNotes.value = false;
    },
  }, bindings));
}, { immediate: true });

onMounted(async () => {
  unlistenExpand = await listen("sticky:expand", requestExpand);
  unlistenMoved = await windowApi.onMoved(({ payload }) => {
    if (collapsed.value || changingSize.value) return;
    if (snapTimer) clearTimeout(snapTimer);
    snapTimer = setTimeout(() => void snapToEdge(payload).catch(console.error), 120);
  });
});

onBeforeUnmount(() => {
  clearTimeout(idleTimer);
  clearTimeout(hoverTimer);
  unlistenExpand?.();
  if (snapTimer) clearTimeout(snapTimer);
  unlistenMoved?.();
});
</script>

<template>
  <div class="sticky" @pointerenter="onPointerEnter" @pointerleave="onPointerLeave">
    <button v-if="collapsed" type="button" class="compact-note" aria-label="展开便签" @click="requestExpand">
      <span>{{ pendingCount }} 待办</span><span v-if="taskStore.topPriorityTasks[0]">{{ taskStore.topPriorityTasks[0].title }}</span>
    </button>
    <div v-show="!collapsed" class="full-note">
    <header class="titlebar" @mousedown.left="windowApi.startDragging()">
      <span class="logo">JustToDo</span>
      <div class="actions" @mousedown.stop>
        <button class="pin-btn" type="button" aria-label="搜索任务" title="搜索任务" @click="focusSearch"><Search16 /></button>
        <button class="pin-btn" type="button" aria-label="任务历史" title="任务历史" @click="view = view === 'history' ? 'tasks' : 'history'"><RecentlyViewed16 /></button>
        <button class="pin-btn" type="button" aria-label="回收站" title="回收站" @click="view = view === 'trash' ? 'tasks' : 'trash'"><TrashCan16 /></button>
        <button class="settings-btn" type="button" aria-label="设置" title="设置" @click="showSettings = true"><Settings16 /></button>
        <button class="minimize-btn" type="button" aria-label="隐藏到托盘" title="隐藏到托盘" @click="perform(() => invoke('hide_window', { label: 'sticky' }))"><Subtract16 /></button>
      </div>
    </header>
    <SettingsWindow v-if="showSettings" @close="showSettings = false" @open-trash="showSettings = false; view = 'trash'" />
    <p v-if="error || taskStore.loadError || configStore.error" class="feedback" role="alert">{{ error || taskStore.loadError || configStore.error }}</p>
    <section v-if="view === 'history'" class="history-panel" aria-label="任务历史">
      <div v-if="!taskStore.history.length" class="history-empty">暂无任务记录</div>
      <div v-for="entry in taskStore.history" :key="entry.id" class="history-item" :class="{ 'history-done': entry.status === 'done' && !entry.deleted, 'history-deleted': entry.deleted }">
        <span class="history-content">
          <span class="history-title"><span v-if="entry.deleted" class="history-emoji" aria-hidden="true">😞</span><span v-else-if="entry.status === 'done'" class="history-emoji" aria-hidden="true">🎉</span>{{ entry.title }}</span>
          <span class="history-time">{{ historyLabels[entry.operation] || entry.operation }} · {{ formatTime(entry.timestamp) }}</span>
          <span v-if="entry.dueDate" class="history-time">到期 · {{ formatTime(entry.dueDate) }}</span>
        </span>
        <button v-if="entry.operation === 'deleted' && trashedIds.has(entry.taskId)" class="history-restore" type="button" @click="restore(entry.taskId)">恢复</button>
      </div>
    </section>
    <TrashView v-else-if="view === 'trash'" />
    <template v-else>
      <nav class="tabs" aria-label="任务 Tab">
        <div v-for="tab in tabStore.sortedTabs" :key="tab.id" class="tab" :class="{ active: tab.id === tabStore.activeTabId }" :style="{ borderBottomColor: tab.id === tabStore.activeTabId ? tab.color : 'transparent' }">
          <button class="tab-label" type="button" :aria-current="tab.id === tabStore.activeTabId" title="双击重命名" @click="switchTab(tab.id)" @dblclick="openTabEditor(tab)">{{ tab.name }}</button>
          <button v-if="tabStore.tabs.length > 1" class="tab-delete" type="button" :class="{ confirm: confirmDeleteTabId === tab.id }" :title="confirmDeleteTabId === tab.id ? '再次点击将任务移入回收站' : '删除 Tab'" :aria-label="confirmDeleteTabId === tab.id ? '确认删除 Tab' : '删除 Tab'" @click="deleteTab(tab.id)"><component :is="confirmDeleteTabId === tab.id ? TrashCan16 : Close16" /></button>
        </div>
        <button class="new-tab-btn" type="button" aria-label="新建 Tab" title="新建 Tab" @click="openTabEditor()">+</button>
      </nav>
      <form v-if="tabEditor" class="new-tab-form" @submit.prevent="saveTab" @keydown.esc="tabEditor = undefined">
        <input ref="tabNameInput" v-model="tabEditor.name" maxlength="30" aria-label="Tab 名称" placeholder="Tab 名称" required />
        <button type="button" @click="tabEditor = undefined">取消</button><button type="submit" :disabled="editingTab">保存</button>
      </form>
      <p v-if="confirmDeleteTabId" class="feedback">再次点击删除可确认，任务将移入回收站。 <button type="button" @click="confirmDeleteTabId = undefined">取消</button></p>
      <div v-if="showSearch" class="search-bar" @keydown.esc="showSearch = false; search = ''">
        <input ref="searchInput" v-model="search" type="search" placeholder="搜索标题和备注" aria-label="搜索任务" />
        <button type="button" aria-label="关闭搜索" @click="showSearch = false; search = ''"><Close16 /></button>
      </div>
      <div ref="taskListEl" class="list">
        <div v-if="selecting" class="selection-bar">
          <span>已选 {{ selected.size }} 项</span>
          <button type="button" :disabled="batchBusy" @click="selected = new Set(searchResults.map(task => task.id))">全选</button>
          <button type="button" :disabled="batchBusy || !selected.size" @click="applySelection('complete')">完成</button>
          <button type="button" :disabled="batchBusy || !selected.size" @click="applySelection('delete')">删除</button>
          <button type="button" :disabled="batchBusy" @click="exitSelection">退出</button>
        </div>
        <draggable :model-value="searchResults" item-key="id" handle=".drag-handle" :animation="150" :disabled="ordering || selecting || !!search.trim()" @start="draggingTab = tabStore.activeTabId" @end="draggingTab = undefined" @update:model-value="reorder">
          <template #item="{ element }">
            <div class="task-row" @click="selecting && selectTask(element.id)">
              <input v-if="selecting" type="checkbox" :checked="selected.has(element.id)" :aria-label="'选择 ' + element.title" @click.stop="selectTask(element.id)" />
              <TaskItem :task="element" :sortable="!search.trim()" :selecting="selecting" @long-press="startSelection(element.id)" />
            </div>
          </template>
        </draggable>
        <div v-if="!searchResults.length" class="empty">{{ search.trim() ? '无匹配任务' : '暂无任务，输入标题后回车添加' }}</div>
      </div>
      <form class="input-bar" @submit.prevent="addTask">
        <div v-if="showComposerNotes" class="composer-notes-wrap"><textarea v-model="newNotes" maxlength="255" placeholder="添加备注" aria-label="添加备注" @keydown.ctrl.enter.prevent="addTask" /></div>
        <div class="composer-main">
          <input ref="taskTitleInput" v-model="newTitle" maxlength="255" placeholder="输入新任务标题…" aria-label="新任务标题" required @keydown.ctrl.enter.prevent="addTask" />
          <button class="composer-priority" type="button" :class="'priority-' + newPriority" aria-label="调整新任务优先级" title="调整优先级" @click="newPriority = newPriority >= 4 ? 0 : newPriority + 1">P{{ newPriority }}</button>
          <input v-model="newDueDate" class="composer-due" type="datetime-local" title="设置到期时间" aria-label="新任务到期时间" />
          <button class="composer-notes-toggle" type="button" aria-label="备注" :aria-expanded="showComposerNotes" @click="showComposerNotes = !showComposerNotes"><TextAlignLeft16 /></button>
          <button class="add-btn" type="submit" aria-label="添加任务" :disabled="creatingTask || !tabStore.activeTabId || !newTitle.trim()">添加</button>
        </div>
      </form>
    </template>
    </div>
  </div>
</template>

<style scoped>
.sticky {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg);
  border-radius: var(--radius);
  overflow: hidden;
  border: 1px solid var(--border);
}
.full-note { display:contents; }
.compact-note { display:flex; align-items:center; flex:1; gap:8px; padding:6px 10px; text-align:left; white-space:nowrap; }
.compact-note span:last-child { overflow:hidden; text-overflow:ellipsis; }
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  background: var(--bg-soft);
  cursor: grab;
}
.logo { font-weight:600; font-size:12px; }
.actions {
  display: flex;
  gap: 4px;
}

.settings-btn { width: 24px; height: 24px; display: inline-flex; align-items: center; justify-content: center; color: var(--text-soft); border-radius: 4px; }
.settings-btn:hover { background: rgba(0, 0, 0, 0.06); color: var(--text); }
.settings-btn :deep(svg) { width: 16px; height: 16px; }
.minimize-btn { width: 24px; height: 24px; display: inline-flex; align-items: center; justify-content: center; color: var(--text-soft); border-radius: 4px; }
.minimize-btn:hover { background: rgba(0, 0, 0, 0.06); color: var(--text); }
.minimize-btn :deep(svg) { width: 16px; height: 16px; }

.pin-btn {
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-soft);
  border-radius: 4px;
}
.pin-btn:hover {
  background: rgba(0, 0, 0, 0.06);
  color: var(--text);
}
.pin-btn :deep(svg) { width:16px; height:16px; }
.history-panel { flex: 1; min-height: 0; overflow-y: auto; background: var(--bg); }
.history-item { display: flex; align-items: center; gap: 6px; min-height: 30px; padding: 5px 8px; border-bottom: 1px solid var(--border); }
.history-item.history-done { background: linear-gradient(90deg, #d7f0dc 0%, #f3fbf4 100%); }
.history-item.history-deleted { background: linear-gradient(90deg, #f8d7da 0%, #fff5f5 100%); }
.history-content { flex: 1; min-width: 0; }
.history-title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.history-emoji { display: inline-block; margin-right: 4px; }
.history-time { display: block; margin-top: 2px; color: var(--text-soft); font-size: 10px; line-height: 1.2; }
.history-restore { display: inline-flex; color: var(--primary); padding: 2px; }
.history-restore :deep(svg) { width: 14px; height: 14px; }
.history-empty { padding: 14px 8px; text-align: center; color: var(--text-soft); font-size: 12px; }
.tabs {
  display: flex;
  gap: 2px;
  padding: 0 6px;
  background: var(--bg);
  border-bottom: 1px solid var(--border);
  overflow-x: auto;
  flex-wrap: nowrap;
  scrollbar-width: none;
}
.tabs::-webkit-scrollbar {
  display: none;
}
.tab {
  display: flex;
  align-items: center;
  padding: 6px 10px;
  font-size: 12px;
  white-space: nowrap;
  border-bottom: 2px solid transparent;
  color: var(--text-soft);
  flex: 0 0 auto;
}
.tab.active {
  color: var(--text);
  font-weight: 600;
}
.tab-label {
  overflow: hidden;
  text-overflow: ellipsis;
}
.tab-delete {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  margin-left: 4px;
  color: var(--text-soft);
  border-radius: 2px;
  opacity: 0;
}
.tab:hover .tab-delete,
.tab-delete.confirm {
  opacity: 1;
}
.tab-delete:hover,
.tab-delete.confirm {
  color: var(--danger);
  background: rgba(208, 2, 27, 0.08);
}
.tab-delete :deep(svg) {
  width: 12px;
  height: 12px;
}
.new-tab-btn {
  flex: 0 0 auto;
  width: 26px;
  height: 28px;
  color: var(--text-soft);
  font-size: 18px;
  line-height: 1;
}
.new-tab-btn:hover {
  color: var(--text);
  background: var(--bg-soft);
}
.new-tab-form {
  flex: 0 0 auto;
}
.new-tab-form input {
  width: 100%;
  height: 24px;
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--bg-soft);
  font-size: 12px;
}
.list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
  scrollbar-width: none;
}
.list::-webkit-scrollbar { display: none; }
.selection-bar { display:flex; align-items:center; gap:6px; padding:6px 10px; font-size:12px; background:var(--bg-soft); }
.selection-bar span { margin-right:auto; }
.selection-bar button { padding:4px 6px; border:1px solid var(--border); border-radius:4px; }
.selection-bar button:disabled { opacity:.45; }
.task-row { display:flex; align-items:center; }
.task-row > input { margin-left:10px; }
.task-row > :last-child { flex:1; min-width:0; }
.empty {
  text-align: center;
  color: var(--text-soft);
  padding: 40px 16px;
  font-size: 12px;
}
.input-bar {
  display: flex;
  flex-direction: column;
  position: relative;
  padding: 8px 10px;
  border-top: 1px solid var(--border);
  gap: 6px;
}
.composer-main { display:flex; align-items:center; flex-wrap:wrap; width:100%; min-width:0; gap:6px; }
.composer-main > input:first-child { flex:1 0 100%; min-width:0; background:var(--bg-soft); border-radius:6px; padding:6px 10px; font-size:13px; }
.composer-due { flex:1 1 190px; min-width:165px; max-width:165px; padding:5px 6px; border:1px solid var(--border); border-radius:6px; background:var(--bg-soft); color:var(--text); font-size:10px; }
.composer-priority { display:inline-flex; align-items:center; justify-content:center; min-width:26px; height:24px; padding:0 5px; border-radius:4px; font-size:10px; font-weight:600; line-height:1; }
.composer-priority.priority-0 { background:#fde2e2; color:#b42318; }
.composer-priority.priority-1 { background:#ffead5; color:#b54708; }
.composer-priority.priority-2 { background:#fff4cc; color:#946200; }
.composer-priority.priority-3 { background:#dcecff; color:#175cd3; }
.composer-priority.priority-4 { background:#eceff1; color:#5f6368; }
.composer-notes-toggle { display:inline-flex; align-items:center; justify-content:center; width:24px; height:24px; color:var(--text-soft); border-radius:4px; flex-shrink:0; }
.composer-notes-toggle:hover, .composer-notes-toggle.active { background:var(--bg-soft); color:var(--text); }
.composer-notes-toggle :deep(svg) { width:16px; height:16px; }
.composer-notes-wrap { position:absolute; z-index:5; left:0; right:0; bottom:100%; height:72px; transform-origin:bottom center; }
.composer-notes-wrap textarea { display:block; width:100%; height:100%; resize:vertical; padding:8px; border:1px solid var(--border); border-radius:0; background:var(--bg-soft); box-shadow:none; color:inherit; font:inherit; font-size:12px; line-height:1.4; }
.new-tab-form, .search-bar { display:flex; flex:none; gap:6px; padding:6px 10px; align-items:center; }
.new-tab-form input, .search-bar input { flex:1; min-width:0; padding:4px 6px; border-radius:4px; border:1px solid var(--border); background:var(--bg-soft); }
.new-tab-form button, .search-bar button { flex-shrink:0; padding:4px; }
.tab-delete:focus-visible { opacity:1; }
.add-btn { padding:5px 8px; background:var(--primary); color:white; border-radius:4px; }
.add-btn:disabled { opacity:.45; }
.feedback { color:var(--danger); padding:6px 10px; font-size:12px; overflow-wrap:anywhere; }
.feedback button { text-decoration:underline; }
</style>
