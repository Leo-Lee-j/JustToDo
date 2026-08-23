<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed, nextTick, watch } from "vue";
import draggable from "vuedraggable";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, currentMonitor, PhysicalPosition } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import pin16 from "@carbon/icons/es/pin/16.js";
import pinFilled16 from "@carbon/icons/es/pin--filled/16.js";
import RecentlyViewed16 from "@carbon/icons-vue/lib/recently-viewed/16.js";
import Undo16 from "@carbon/icons-vue/lib/undo/16.js";
import Home16 from "@carbon/icons-vue/lib/home/16.js";
import Close16 from "@carbon/icons-vue/lib/close/16.js";
import TrashCan16 from "@carbon/icons-vue/lib/trash-can/16.js";
import TextAlignLeft16 from "@carbon/icons-vue/lib/text--align--left/16.js";
import Settings16 from "@carbon/icons-vue/lib/settings/16.js";
import ChevronDown16 from "@carbon/icons-vue/lib/chevron--down/16.js";
import Subtract16 from "@carbon/icons-vue/lib/subtract/16.js";
import { enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { isPermissionGranted, requestPermission, sendNotification as sendNativeNotification } from "@tauri-apps/plugin-notification";
import { useTaskStore } from "@/stores/taskStore";
import { useTabStore } from "@/stores/tabStore";
import { useConfigStore } from "@/stores/configStore";
import type { Task } from "@/types";
import TaskItem from "./TaskItem.vue";
import SettingsWindow from "./SettingsWindow.vue";
import { installWindowShortcuts } from "@/utils/shortcuts";

const taskStore = useTaskStore();
const tabStore = useTabStore();
const configStore = useConfigStore();

const newTitle = ref("");
const newNotes = ref("");
const newPriority = ref(2);
const newDueDate = ref("");
const dueDateInput = ref<HTMLInputElement | null>(null);
const showComposerNotes = ref(false);
const creatingTask = ref(false);
const drag = ref(false);
const showSettings = ref(false);
const showNewTab = ref(false);
const newTabName = ref("");
const creatingTab = ref(false);
const confirmDeleteTabId = ref<string | null>(null);
const showHistory = ref(false);
const updateStatus = ref("");
const currentVersion = ref("0.0.2");
const availableVersion = ref("");
const updateNotes = ref("");
const updateProgress = ref(0);
const updateBusy = ref(false);
const taskListEl = ref<HTMLElement | null>(null);
const systemFonts = ref<string[]>([]);
const fontSearch = ref("");
const fontPickerOpen = ref(false);

const tabs = computed(() => tabStore.sortedTabs);
const filteredFonts = computed(() => {
  const query = fontSearch.value.trim().toLowerCase();
  return query ? systemFonts.value.filter((font) => font.toLowerCase().includes(query)) : systemFonts.value;
});
const windowApi = getCurrentWindow();
let unlistenMoved: (() => void) | undefined;
let unlistenNotification: (() => void) | undefined;
let uninstallShortcuts: (() => void) | undefined;
let stopShortcutWatch: (() => void) | undefined;
let snapTimer: ReturnType<typeof setTimeout> | undefined;
let snapping = false;
let notificationBatchRemaining = 0;
let notificationBatchBody = "";
function sendNotification(options: { title: string; body: string }) {
  if (notificationBatchRemaining > 1) {
    notificationBatchRemaining -= 1;
    return;
  }
  sendNativeNotification({ title: "任务提醒", body: notificationBatchBody || options.body });
  notificationBatchRemaining = 0;
}
// Physical window bounds include the frame/shadow differently on each edge.
// Keep the visual gap consistent with small per-edge adjustments.
const EDGE_MARGIN = { left: 12, top: 12, right: 8, bottom: 12 };
const SNAP_THRESHOLD = 24;

function formatHistoryTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const now = new Date();
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const time = date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  if (date.toDateString() === now.toDateString()) return `今天 ${time}`;
  if (date.toDateString() === yesterday.toDateString()) return `昨天 ${time}`;
  return `${date.getMonth() + 1}月${date.getDate()}日 ${time}`;
}

function formatRemainingTime(dueAt: number, now: number) {
  const minutes = Math.ceil((dueAt - now) / 60000);
  if (minutes <= 0) return "已到期";
  if (minutes < 60) return `剩余 ${minutes} 分钟`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `剩余 ${hours} 小时 ${rest} 分钟` : `剩余 ${hours} 小时`;
}

async function loadData() {
  await tabStore.load();
  await taskStore.load();
  if (tabStore.activeTabId) taskStore.activeTabId = tabStore.activeTabId;
}

async function checkNotifications(candidate?: Task) {
  if (!configStore.config.notification.enabled) return;
  let granted = await isPermissionGranted().catch(() => false);
  if (!granted) granted = (await requestPermission().catch(() => "denied")) === "granted";
  if (!granted) return;
  const hours = Math.max(0, configStore.config.notification.reminderHours ?? 1);
  const now = Date.now();
  const tasks = candidate ? [candidate, ...taskStore.tasks.filter((item) => item.id !== candidate.id)] : taskStore.tasks;
  const eligible = tasks.filter((task) => task.dueDate && !task.deletedAt && task.status !== "done" && task.status !== "cancelled" && !task.notifiedAt && !Number.isNaN(new Date(task.dueDate).getTime()) && new Date(task.dueDate).getTime() - now <= hours * 3600_000);
  if (!eligible.length) {
    notificationBatchRemaining = 0;
    notificationBatchBody = "";
    return;
  }
  notificationBatchRemaining = eligible.length;
  const lines = eligible.slice(0, 5).map((task) => {
    const due = new Date(task.dueDate as string).getTime();
    return `${task.title} · ${formatRemainingTime(due, now)}`;
  });
  if (eligible.length > 5) lines.push(`还有 ${eligible.length - 5} 个任务`);
  notificationBatchBody = lines.join("\n");
  for (const task of tasks) {
    if (!task.dueDate || task.deletedAt || task.status === "done" || task.status === "cancelled" || task.notifiedAt) continue;
    const due = new Date(task.dueDate).getTime();
    if (Number.isNaN(due) || due - now > hours * 3600_000) continue;
    sendNotification({ title: "JustToDo", body: `任务即将结束：${task.title}` });
    await taskStore.update(task.id, { notifiedAt: new Date().toISOString() });
  }
}

async function addTask() {
  if (creatingTask.value) return;
  const t = newTitle.value.trim();
  if (!t) return;
  creatingTask.value = true;
  try {
    const dueDate = newDueDate.value ? new Date(newDueDate.value).toISOString() : null;
    const task = await taskStore.create(t, tabStore.activeTabId, dueDate);
    if (newNotes.value.trim() || newPriority.value !== 2) {
      await taskStore.update(task.id, { notes: newNotes.value.slice(0, 255), priority: newPriority.value });
    }
    await invoke("check_notifications").catch(() => undefined);
    newTitle.value = "";
    newNotes.value = "";
    newPriority.value = 2;
    newDueDate.value = "";
    showComposerNotes.value = false;
    await nextTick();
    taskListEl.value?.querySelector<HTMLElement>(`[data-task-id="${task.id}"]`)?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  } finally {
    creatingTask.value = false;
  }
}

function closeDueDatePicker() {
  requestAnimationFrame(() => dueDateInput.value?.blur());
}

async function switchTab(id: string) {
  await tabStore.setActive(id);
  taskStore.activeTabId = id;
}

async function restoreFromHistory(taskId: string) {
  await taskStore.restore(taskId);
  await tabStore.load();
  await taskStore.load();
}

async function createTab() {
  if (creatingTab.value) return;
  const name = newTabName.value.trim();
  if (!name) return;
  creatingTab.value = true;
  try {
    const tab = await tabStore.create(name);
    newTabName.value = "";
    showNewTab.value = false;
    await switchTab(tab.id);
  } finally {
    creatingTab.value = false;
  }
}

async function deleteTab(id: string) {
  if (tabStore.tabs.length <= 1) return;
  if (confirmDeleteTabId.value !== id) {
    confirmDeleteTabId.value = id;
    return;
  }
  await tabStore.remove(id);
  confirmDeleteTabId.value = null;
  taskStore.activeTabId = tabStore.activeTabId;
}

async function toggleAlwaysOnTop() {
  await configStore.update({
    ...configStore.config,
    window: {
      ...configStore.config.window,
      alwaysOnTop: !configStore.config.window.alwaysOnTop,
    },
  });
  await invoke("set_always_on_top", { label: "sticky", on: configStore.config.window.alwaysOnTop });
}

async function setOpacity(v: number) {
  await configStore.update({
    ...configStore.config,
    window: { ...configStore.config.window, opacity: v },
  });
  await invoke("set_opacity", { label: "sticky", opacity: v });
}

async function setFontFamily(value: string) {
  await configStore.update({ general: { ...configStore.config.general, fontFamily: value } });
  fontPickerOpen.value = false;
}

async function loadSystemFonts() {
  try {
    systemFonts.value = await invoke<string[]>("list_system_fonts");
  } catch {
    systemFonts.value = ["Microsoft YaHei", "Arial", "Segoe UI", "sans-serif", "serif", "monospace"];
  }
}

async function setLaunchOnStartup(enabled: boolean) {
  try {
    if (enabled) await enableAutostart();
    else await disableAutostart();
    await configStore.update({ general: { ...configStore.config.general, launchOnStartup: enabled } });
  } catch {
    updateStatus.value = "无法修改开机启动设置";
  }
}

async function checkForUpdates() {
  if (updateBusy.value) return;
  updateBusy.value = true;
  updateStatus.value = "正在检查更新...";
  try {
    const update = await check();
    if (!update) {
      updateStatus.value = `当前已是最新版本（v${currentVersion.value}）`;
      return;
    }
    availableVersion.value = update.version;
    updateNotes.value = update.body || "暂无更新说明";
    let downloaded = 0;
    let totalBytes = 0;
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        downloaded = 0;
        totalBytes = event.data.contentLength || 0;
        updateProgress.value = 0;
        updateStatus.value = `正在下载 v${update.version}...`;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        if (totalBytes) updateProgress.value = Math.min(100, Math.round(downloaded / totalBytes * 100));
      } else if (event.event === "Finished") {
        updateProgress.value = 100;
        updateStatus.value = "下载完成，正在重启安装...";
      }
    });
    await relaunch();
  } catch (error) {
    console.error("update check failed", error);
    updateStatus.value = "检查更新失败，请稍后重试";
  } finally {
    updateBusy.value = false;
  }
}

async function toggleSettings() {
  showSettings.value = true;
}

async function onReorder() {
  const ids = taskStore.activeTasks.map((t) => t.id);
  await taskStore.reorder(ids);
}

async function startDragWindow() {
  // 无边框窗口拖动
  await windowApi.startDragging();
}

async function performDragWindow() {
  await windowApi.startDragging();
}

async function snapToEdge(movedPosition?: { x: number; y: number }) {
  if (snapping) return;
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
    await windowApi.setPosition(new PhysicalPosition(x, y));
  } finally {
    snapping = false;
  }
}

onMounted(async () => {
  currentVersion.value = await getVersion().catch(() => "0.0.2");
  document.addEventListener("pointerdown", closeComposerNotesOnOutside);
  await configStore.load();
  await invoke("sync_taskbar");
  await loadData();
  const installShortcuts = () => {
    uninstallShortcuts?.();
    uninstallShortcuts = installWindowShortcuts({
      newTask: () => document.querySelector<HTMLInputElement>(".composer-main input")?.focus(),
      newTab: () => { showNewTab.value = true; },
      switchTab: (index) => {
        const tab = tabs.value[index - 1];
        if (tab) void switchTab(tab.id);
      },
    }, configStore.config.general.shortcuts);
  };
  installShortcuts();
  stopShortcutWatch = watch(() => configStore.config.general.shortcuts, installShortcuts, { deep: true });
  unlistenNotification = await listen("notification:sent", () => void taskStore.load());
  unlistenMoved = await windowApi.onMoved(({ payload }) => {
    if (snapTimer) clearTimeout(snapTimer);
    snapTimer = setTimeout(() => void snapToEdge(payload), 120);
  });
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", closeComposerNotesOnOutside);
  if (snapTimer) clearTimeout(snapTimer);
  unlistenMoved?.();
  unlistenNotification?.();
  uninstallShortcuts?.();
  stopShortcutWatch?.();
});

function closeComposerNotesOnOutside(event: PointerEvent) {
  const target = event.target as HTMLElement;
  if (target.closest(".settings-window")) return;
  if (!target.closest(".input-bar")) showComposerNotes.value = false;
  if (!target.closest(".settings-pop, .settings-btn")) {
    showSettings.value = false;
    fontPickerOpen.value = false;
  }
}
</script>

<template>
  <div class="sticky" :style="{ fontFamily: configStore.config.general.fontFamily }">
    <!-- 标题栏（可拖拽） -->
    <header class="titlebar" @mousedown="performDragWindow" @dblclick.prevent.stop>
      <span class="logo">📌 JustToDo</span>
      <div class="actions" @mousedown.stop>
        <button class="history-toggle" @click="showHistory = !showHistory" :title="showHistory ? '返回主界面' : '任务历史'" :aria-label="showHistory ? '返回主界面' : '任务历史'"><component :is="showHistory ? Home16 : RecentlyViewed16" /></button>
        <button class="pin-btn" @click="showHistory = !showHistory" title="任务历史" aria-label="任务历史"><RecentlyViewed16 /></button>
        <button class="pin-btn" @click="toggleAlwaysOnTop" :title="configStore.config.window.alwaysOnTop ? '取消置顶' : '置顶'">
          <svg class="pin-icon" viewBox="0 0 32 32" aria-hidden="true">
            <path :d="(configStore.config.window.alwaysOnTop ? pinFilled16 : pin16).content[0].attrs.d" />
          </svg>
        </button>
        <button class="icon-btn" @click="toggleAlwaysOnTop" :title="'置顶'">
          {{ configStore.config.window.alwaysOnTop ? "📌" : "📍" }}
        </button>
        <button class="settings-btn" @click="toggleSettings" title="设置" aria-label="设置"><Settings16 /></button>
        <button class="minimize-btn" @click="invoke('hide_window', { label: 'sticky' })" title="最小化" aria-label="最小化"><Subtract16 /></button>
      </div>
    </header>
    <SettingsWindow v-if="showSettings" @close="showSettings = false" />
    <section v-if="showHistory" class="history-panel">
      <div v-if="!taskStore.history.length" class="history-empty">暂无任务记录</div>
      <div v-for="entry in taskStore.history" :key="entry.id" class="history-item" :class="{ 'history-done': entry.status === 'done' && !entry.deleted, 'history-deleted': entry.deleted }">
        <span class="history-content">
          <span class="history-title"><span v-if="entry.deleted" class="history-emoji" aria-hidden="true">😞</span><span v-else-if="entry.status === 'done'" class="history-emoji" aria-hidden="true">🎉</span>{{ entry.title }}</span>
          <span class="history-time">更新时间：{{ formatHistoryTime(entry.updatedAt || entry.timestamp) }}</span>
          <span v-if="entry.dueDate" class="history-time">结束时间：{{ formatHistoryTime(entry.dueDate) }}</span>
        </span>
        <button v-if="entry.deleted" class="history-restore" title="恢复任务" aria-label="恢复任务" @click="restoreFromHistory(entry.taskId)"><Undo16 /></button>
      </div>
    </section>

    <!-- 设置浮层 -->
    <div v-if="false" class="settings-pop">
      <div class="settings-title">设置</div>
      <label class="settings-label">字体</label>
      <div class="font-picker" :class="{ open: fontPickerOpen }">
        <button class="font-picker-trigger" @click="fontPickerOpen = !fontPickerOpen" :style="{ fontFamily: configStore.config.general.fontFamily }">
          <span>{{ configStore.config.general.fontFamily }}</span><ChevronDown16 class="font-picker-chevron" aria-hidden="true" />
        </button>
        <div v-if="fontPickerOpen" class="font-picker-menu">
          <input v-model="fontSearch" class="font-search" placeholder="搜索字体..." @click.stop />
          <div class="font-options">
            <button v-for="font in filteredFonts" :key="font" class="font-option" :class="{ selected: font === configStore.config.general.fontFamily }" :style="{ fontFamily: font }" @click="setFontFamily(font)">{{ font }}</button>
            <span v-if="!filteredFonts.length" class="font-empty">未找到字体</span>
          </div>
        </div>
      </div>
      <label class="settings-label">背景透明度 {{ configStore.config.window.opacity }}%</label>
      <input
        type="range"
        min="20"
        max="100"
        :value="configStore.config.window.opacity"
        @input="setOpacity(Number(($event.target as HTMLInputElement).value))"
      />
      <label class="settings-check"><input type="checkbox" :checked="configStore.config.general.launchOnStartup" @change="setLaunchOnStartup(($event.target as HTMLInputElement).checked)" /> 开机自启动</label>
      <div class="version-label">当前版本 v{{ currentVersion }}</div>
      <button class="update-btn" :disabled="updateBusy" @click="checkForUpdates">{{ updateBusy ? "处理中..." : "检查更新" }}</button>
      <div v-if="availableVersion" class="update-release">新版本 v{{ availableVersion }}<div class="update-notes">{{ updateNotes }}</div></div>
      <div v-if="updateBusy && updateProgress > 0" class="update-progress"><span :style="{ width: `${updateProgress}%` }"></span></div>
      <span v-if="updateStatus" class="update-status">{{ updateStatus }}</span>
    </div>

    <!-- Tab 栏 -->
    <nav v-if="!showHistory" class="tabs">
      <div
        v-for="t in tabs"
        :key="t.id"
        class="tab"
        role="tab"
        tabindex="0"
        :class="{ active: t.id === tabStore.activeTabId }"
        :style="{ borderBottomColor: t.id === tabStore.activeTabId ? t.color : 'transparent' }"
        @click="switchTab(t.id)"
        @keydown.enter="switchTab(t.id)"
      >
        <span class="tab-label">{{ t.name }}</span>
        <button
          v-if="tabStore.tabs.length > 1"
          class="tab-delete"
          :class="{ confirm: confirmDeleteTabId === t.id }"
          :title="confirmDeleteTabId === t.id ? '再次点击确认删除' : '删除 tab'"
          :aria-label="confirmDeleteTabId === t.id ? '再次点击确认删除' : '删除 tab'"
          @click.stop="deleteTab(t.id)"
        >
          <component :is="confirmDeleteTabId === t.id ? TrashCan16 : Close16" />
        </button>
      </div>
      <form v-if="showNewTab" class="new-tab-form" @submit.prevent="createTab">
        <input v-model="newTabName" autofocus maxlength="30" placeholder="名称" @keydown.esc="showNewTab = false" />
      </form>
      <button v-else class="new-tab-btn" title="新建 tab" aria-label="新建 tab" @click="showNewTab = true">+</button>
    </nav>

    <!-- 任务列表 -->
    <div v-if="!showHistory" ref="taskListEl" class="list">
      <draggable
        v-model="taskStore.activeTasks"
        item-key="id"
        handle=".drag-handle"
        :animation="150"
        :class="{ dragging: drag }"
        @start="drag = true"
        @end="drag = false; onReorder()"
      >
        <template #item="{ element }">
          <TaskItem :task="element" />
        </template>
      </draggable>
      <div v-if="!taskStore.activeTasks.length" class="empty">暂无任务，回车添加一个吧 ✨</div>
    </div>

    <!-- 输入栏 -->
    <footer v-if="!showHistory" class="input-bar" :class="{ expanded: showComposerNotes }">
      <div class="composer-main">
        <input v-model="newTitle" placeholder="输入新任务标题..." @blur="showComposerNotes = false" @keydown.ctrl.enter.prevent="addTask" />
        <button class="composer-priority" :class="`priority-${newPriority}`" @click="newPriority = newPriority >= 4 ? 0 : newPriority + 1" title="调整优先级" aria-label="调整优先级">
          P{{ newPriority }}
        </button>
        <input ref="dueDateInput" v-model="newDueDate" class="composer-due" type="datetime-local" step="3600" title="设置结束时间" aria-label="设置结束时间" @keydown.prevent @change="closeDueDatePicker" />
        <button class="composer-notes-toggle" :class="{ active: showComposerNotes }" @mousedown.prevent @click="showComposerNotes = !showComposerNotes" :title="showComposerNotes ? '关闭备注' : '添加备注'" :aria-label="showComposerNotes ? '关闭备注' : '展开备注'">
          <component :is="showComposerNotes ? Close16 : TextAlignLeft16" />
        </button>
      </div>
      <Transition name="composer-notes">
        <div v-if="showComposerNotes" class="composer-notes-wrap">
          <textarea v-model="newNotes" maxlength="255" placeholder="添加备注..." @blur="showComposerNotes = false" @keydown.ctrl.enter.prevent="addTask" />
        </div>
      </Transition>
      <input
        v-model="newTitle"
        placeholder="输入新任务并回车..."
        @keydown.enter.prevent="addTask"
      />
      <button class="add-btn" @click="addTask">➕</button>
    </footer>
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
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  background: var(--bg-soft);
  cursor: grab;
}
.logo {
  font-size: 0;
  font-weight: 600;
}
.logo::after {
  content: "JustToDo";
  font-size: 12px;
}
.actions {
  display: flex;
  gap: 4px;
}
.actions .icon-btn {
  display: none;
}
.settings-btn { width: 24px; height: 24px; display: inline-flex; align-items: center; justify-content: center; color: var(--text-soft); border-radius: 4px; }
.settings-btn:hover { background: rgba(0, 0, 0, 0.06); color: var(--text); }
.settings-btn :deep(svg) { width: 16px; height: 16px; }
.minimize-btn { width: 24px; height: 24px; display: inline-flex; align-items: center; justify-content: center; color: var(--text-soft); border-radius: 4px; }
.minimize-btn:hover { background: rgba(0, 0, 0, 0.06); color: var(--text); }
.minimize-btn :deep(svg) { width: 16px; height: 16px; }
.actions > .pin-btn:nth-child(2) { display: none; }
.history-toggle { width:24px; height:24px; display:inline-flex; align-items:center; justify-content:center; color:var(--text-soft); border-radius:4px; }
.history-toggle:hover { background:rgba(0,0,0,.06); color:var(--text); }
.history-toggle :deep(svg) { width:16px; height:16px; }
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
.pin-icon {
  display: block;
  width: 16px;
  height: 16px;
  fill: currentColor;
  opacity: 1;
}
.icon-btn {
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0;
  color: var(--text-soft);
}
.icon-btn:first-child::after {
  content: "置顶";
  font-size: 11px;
}
.icon-btn:not(:first-child) {
  display: none;
}
.icon-btn:hover {
  background: rgba(0, 0, 0, 0.06);
}
.settings-pop {
  position: absolute;
  right: 10px;
  top: 34px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 8px;
  z-index: 10;
  box-shadow: var(--shadow);
  width: 180px;
}
.settings-title { font-weight: 600; font-size: 12px; margin-bottom: 8px; }
.settings-label { display: block; margin: 8px 0 4px; font-size: 11px; color: var(--text-soft); }
.settings-pop input[type="range"] { display: block; width: 100%; margin: 0; }
.font-picker { position: relative; }
.font-picker-trigger { width: 100%; display: flex; align-items: center; justify-content: space-between; gap: 6px; padding: 6px 8px; border: 1px solid var(--border); border-radius: 5px; background: var(--bg-soft); text-align: left; font-size: 11px; }
.font-picker-trigger span:first-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.font-picker-chevron { width: 16px; height: 16px; flex: 0 0 16px; color: var(--text-soft); }
.font-picker-menu { position: absolute; left: 0; right: 0; top: calc(100% + 4px); z-index: 20; padding: 6px; border: 1px solid var(--border); border-radius: 5px; background: var(--bg); box-shadow: var(--shadow); }
.font-search { width: 100%; padding: 5px 7px; border: 1px solid var(--border); border-radius: 4px; background: var(--bg-soft); font: inherit; font-size: 11px; }
.font-options { max-height: 150px; margin-top: 5px; overflow-y: auto; }
.font-option { width: 100%; display: block; padding: 5px 7px; border-radius: 3px; text-align: left; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.font-option:hover, .font-option.selected { background: var(--bg-soft); color: var(--primary); }
.font-empty { display: block; padding: 8px 7px; color: var(--text-soft); font-size: 11px; text-align: center; }
.settings-check { display: flex; align-items: center; gap: 6px; margin-top: 10px; font-size: 11px; }
.update-btn { width: 100%; margin-top: 10px; padding: 5px; border: 1px solid var(--border); border-radius: 4px; color: var(--text); background: var(--bg-soft); font-size: 11px; }
.update-status { display: block; margin-top: 6px; color: var(--text-soft); font-size: 10px; }
.version-label { margin-top: 10px; color: var(--text-soft); font-size: 10px; }
.update-release { margin-top: 6px; font-size: 11px; color: var(--text); }
.update-notes { margin-top: 3px; white-space: pre-wrap; max-height: 80px; overflow: auto; color: var(--text-soft); font-size: 10px; }
.update-progress { height: 3px; margin-top: 6px; overflow: hidden; background: var(--border); border-radius: 2px; }
.update-progress span { display: block; height: 100%; background: var(--accent); transition: width .2s ease; }
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
  flex: 0 0 92px;
  padding: 3px 2px;
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
.list :deep(.task) { cursor: grab; }
.list.dragging, .list.dragging :deep(.task) { cursor: grabbing !important; }
.list::-webkit-scrollbar { display: none; }
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
.input-bar > input { display: none; }
.input-bar > .add-btn { display: none; }
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
.composer-notes-enter-active, .composer-notes-leave-active { transition: opacity 140ms ease, transform 160ms ease; }
.composer-notes-enter-from, .composer-notes-leave-to { opacity:0; transform:translateY(8px) scaleY(.94); }
.input-bar input {
  flex: 1;
  background: var(--bg-soft);
  border-radius: 6px;
  padding: 6px 10px;
  font-size: 13px;
}
.add-btn {
  padding: 0 12px;
  background: var(--primary);
  color: #fff;
  border-radius: 6px;
  font-size: 0;
  width: 34px;
  min-width: 34px;
  position: relative;
}
.add-btn::before,
.add-btn::after {
  content: "";
  position: absolute;
  left: 50%;
  top: 50%;
  width: 12px;
  height: 2px;
  background: currentColor;
  transform: translate(-50%, -50%);
}
.add-btn::after {
  transform: translate(-50%, -50%) rotate(90deg);
}
</style>
