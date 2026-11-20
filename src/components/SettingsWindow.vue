<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import ChevronDown16 from "@carbon/icons-vue/lib/chevron--down/16.js";
import Close16 from "@carbon/icons-vue/lib/close/16.js";
import { useConfigStore } from "@/stores/configStore";
import { eventKey, shortcutConflict } from "@/utils/shortcuts";
import type { ShortcutConfig } from "@/types";
import type { Category } from "@/types";
import { useTabStore } from "@/stores/tabStore";

const configStore = useConfigStore();
const tabStore = useTabStore();
const windowApi = getCurrentWindow();
const emit = defineEmits<{ close: []; openTrash: [] }>();
const currentVersion = ref(configStore.config.version);
const availableVersion = ref("");
const updateNotes = ref("");
const updateProgress = ref(0);
const updateStatus = ref("");
const updateBusy = ref(false);
const showUpdateConfirmation = ref(false);
const notificationPermission = ref<"granted" | "denied" | "unknown">("unknown");
const systemFonts = ref<string[]>([]);
const fontSearch = ref("");
const fontPickerOpen = ref(false);
const fontLoading = ref(false);
const recordingShortcut = ref<keyof ShortcutConfig | null>(null);
const shortcutError = ref("");
const notificationStatus = ref("");
type Backup = { id: string; savedAt: string; activeTasks: number; trashedTasks: number };
const backups = ref<Backup[]>([]);
const backupsOpen = ref(false);
const backupBusy = ref(false);
const backupId = ref("");
const confirmBackup = ref(false);
const backupStatus = ref("");
const categoryDraft = ref({ id: "", name: "", color: "#9B9B9B" });
const categoryBusy = ref(false);
const categoryStatus = ref("");
const deleteCategoryId = ref("");

async function loadBackups() {
  backupsOpen.value = true;
  confirmBackup.value = false;
  backupBusy.value = true;
  try {
    backups.value = await invoke<Backup[]>("list_backups");
    backupId.value = backups.value[0]?.id ?? "";
  } catch (error) {
    backupStatus.value = String(error);
  } finally {
    backupBusy.value = false;
  }
}

function backupLabel(backup: Backup) {
  const type = backup.id.startsWith("daily-") ? "每日备份" : backup.id.startsWith("purge-") ? "永久删除前" : "恢复前";
  return [new Date(backup.savedAt).toLocaleString("zh-CN"), type, backup.activeTasks + " 项任务", backup.trashedTasks + " 项回收站任务"].join(" · ");
}

async function restoreBackup() {
  if (!backupId.value || backupBusy.value) return;
  backupBusy.value = true;
  confirmBackup.value = false;
  try {
    await invoke("restore_backup", { id: backupId.value });
    backupStatus.value = "任务已恢复；恢复前的当前数据也已备份。";
    backups.value = await invoke<Backup[]>("list_backups");
  } catch (error) {
    backupStatus.value = String(error);
  } finally {
    backupBusy.value = false;
  }
}

function editCategory(category: Category) {
  deleteCategoryId.value = "";
  categoryDraft.value = { id: category.id, name: category.name, color: category.color };
}

async function saveCategory() {
  if (categoryBusy.value || !categoryDraft.value.name.trim()) return;
  categoryBusy.value = true;
  categoryStatus.value = "";
  try {
    if (categoryDraft.value.id) {
      await tabStore.updateCategory(categoryDraft.value.id, categoryDraft.value.name, categoryDraft.value.color);
    } else {
      await tabStore.createCategory(categoryDraft.value.name, categoryDraft.value.color);
    }
    categoryDraft.value = { id: "", name: "", color: "#9B9B9B" };
  } catch (error) {
    categoryStatus.value = String(error);
  } finally {
    categoryBusy.value = false;
  }
}

async function removeCategory(category: Category) {
  if (categoryBusy.value) return;
  if (deleteCategoryId.value !== category.id) {
    deleteCategoryId.value = category.id;
    categoryStatus.value = "再次点击将删除“" + category.name + "”，所属任务会改用其他分类。";
    return;
  }
  categoryBusy.value = true;
  try {
    await tabStore.removeCategory(category.id);
    deleteCategoryId.value = "";
    if (categoryDraft.value.id === category.id) categoryDraft.value = { id: "", name: "", color: "#9B9B9B" };
    categoryStatus.value = "分类已删除；关联任务已转到其他分类。";
  } catch (error) {
    categoryStatus.value = String(error);
  } finally {
    categoryBusy.value = false;
  }
}
const fallbackFonts = ["Microsoft YaHei", "Arial", "Segoe UI", "sans-serif", "serif", "monospace"];
let pendingUpdate: Awaited<ReturnType<typeof check>> = null;

const filteredFonts = computed(() => {
  const query = fontSearch.value.trim().toLowerCase();
  return query ? systemFonts.value.filter((font) => font.toLowerCase().includes(query)) : systemFonts.value;
});

async function loadSystemFonts() {
  if (fontLoading.value || systemFonts.value.length) return;

  fontLoading.value = true;
  try {
    const fonts = await invoke<string[]>("list_system_fonts");
    systemFonts.value = fonts.length ? fonts : fallbackFonts;
  } catch {
    systemFonts.value = fallbackFonts;
  } finally {
    fontLoading.value = false;
  }
}

watch(fontPickerOpen, (isOpen) => {
  if (isOpen) void loadSystemFonts();
});

async function setFontFamily(value: string) {
  await configStore.update(({ general }) => ({ general: { ...general, fontFamily: value } }));
  fontPickerOpen.value = false;
}

async function setOpacity(value: number) {
  if (await configStore.update(({ window }) => ({ window: { ...window, opacity: value } }))) {
    await invoke("set_opacity", { label: "sticky", opacity: value });
  }
}

async function setLaunchOnStartup(enabled: boolean) {
  const previous = configStore.config.general.launchOnStartup;
  try {
    if (enabled) await enableAutostart(); else await disableAutostart();
    if (!await configStore.update(({ general }) => ({ general: { ...general, launchOnStartup: enabled } }))) {
      if (previous) await enableAutostart(); else await disableAutostart();
    }
  } catch {
    updateStatus.value = "无法修改开机启动设置";
  }
}

async function setNotificationsEnabled(enabled: boolean) {
  if (await configStore.update(({ notification }) => ({ notification: { ...notification, enabled } })) && enabled) {
    await invoke("check_notifications").catch(console.error);
  }
}

async function setReminderHours(value: number) {
  const reminderHours = Math.max(0, Math.min(168, Math.trunc(value) || 0));
  if (await configStore.update(({ notification }) => ({ notification: { ...notification, reminderHours } })) && configStore.config.notification.enabled) {
    await invoke("check_notifications").catch(console.error);
  }
}

async function setSoundEnabled(enabled: boolean) {
  await configStore.update(({ notification }) => ({ notification: { ...notification, soundEnabled: enabled } }));
}

async function setTaskCompletionMode(taskCompletionMode: typeof configStore.config.general.taskCompletionMode) {
  await configStore.update(({ general }) => ({ general: { ...general, taskCompletionMode } }));
}

async function refreshNotificationPermission() {
  try {
    notificationPermission.value = (await isPermissionGranted()) ? "granted" : "denied";
  } catch {
    notificationPermission.value = "unknown";
  }
}

async function testNotification() {
  notificationStatus.value = "";
  let granted = await isPermissionGranted().catch(() => false);
  if (!granted) granted = (await requestPermission().catch(() => "denied")) === "granted";
  notificationPermission.value = granted ? "granted" : "denied";
  if (!granted) return;
  try {
    await invoke("send_test_notification");
    notificationStatus.value = "测试通知已发送";
  } catch (error) {
    notificationStatus.value = "测试通知发送失败：" + String(error);
  }
}

function getUpdateErrorStatus(error: unknown, phase: "check" | "install") {
  const detail = String(error ?? "");
  console.error(`update ${phase} failed`, error);

  if (phase === "check") {
    if (/404|not found|未找到/i.test(detail)) return "更新源暂不可用（尚未发布更新）";
    if (/fetch|network|connect|网络/i.test(detail)) return "无法连接更新服务，请检查网络";
    return "检查更新失败，请稍后重试";
  }

  if (/fetch|network|connect|网络/i.test(detail)) return "下载更新失败，请检查网络";
  return "更新安装失败，请稍后重试";
}

async function checkForUpdates() {
  if (updateBusy.value) return;

  updateBusy.value = true;
  updateStatus.value = "正在检查更新...";
  updateProgress.value = 0;
  availableVersion.value = "";
  updateNotes.value = "";
  pendingUpdate = null;
  showUpdateConfirmation.value = false;

  try {
    const update = await check();
    if (!update) {
      updateStatus.value = `当前已是最新版本（v${currentVersion.value}）`;
      return;
    }

    pendingUpdate = update;
    availableVersion.value = update.version;
    updateNotes.value = update.body || "暂无更新说明";
    updateStatus.value = `发现新版本 v${update.version}`;
    showUpdateConfirmation.value = true;
  } catch (error) {
    updateStatus.value = getUpdateErrorStatus(error, "check");
  } finally {
    updateBusy.value = false;
  }
}

function postponeUpdate() {
  if (updateBusy.value) return;
  showUpdateConfirmation.value = false;
  pendingUpdate = null;
  updateStatus.value = `已暂不下载 v${availableVersion.value}`;
}

async function downloadAndInstallUpdate() {
  const update = pendingUpdate;
  if (updateBusy.value || !update) return;

  updateBusy.value = true;
  showUpdateConfirmation.value = false;
  updateProgress.value = 0;

  try {
    let downloaded = 0;
    let totalBytes = 0;
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        downloaded = 0;
        totalBytes = event.data.contentLength || 0;
        updateStatus.value = `正在下载 v${update.version}...`;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        if (totalBytes) updateProgress.value = Math.min(100, Math.round(downloaded / totalBytes * 100));
      } else if (event.event === "Finished") {
        updateProgress.value = 100;
        updateStatus.value = "下载完成，正在重启安装...";
      }
    });
    pendingUpdate = null;
    await relaunch();
  } catch (error) {
    pendingUpdate = null;
    updateStatus.value = getUpdateErrorStatus(error, "install");
  } finally {
    updateBusy.value = false;
  }
}

function close() {
  emit("close");
}

async function dragSettings(event: PointerEvent) {
  if ((event.target as HTMLElement).closest("button")) return;
  await windowApi.startDragging();
}

async function captureShortcut(field: keyof ShortcutConfig, event: KeyboardEvent) {
  if (["Control", "Meta", "Alt", "Shift"].includes(event.key)) return;
  if (!event.ctrlKey && !event.metaKey && !event.altKey) {
    shortcutError.value = "请使用 Ctrl、Alt 或 Win 组合键";
    return;
  }
  const binding = eventKey(event);
  const { general } = configStore.config;
  if (shortcutConflict(binding, general.shortcuts, field)) {
    shortcutError.value = "该组合键已被本应用使用";
    return;
  }
  recordingShortcut.value = null;
  shortcutError.value = "";
  await configStore.update(({ general }) => ({ general: { ...general, shortcuts: { ...general.shortcuts, [field]: binding } } }));
}

function beginShortcutRecording(field: keyof ShortcutConfig) {
  shortcutError.value = "";
  recordingShortcut.value = field;
}

async function onShortcutKeydown(event: KeyboardEvent) {
  const field = recordingShortcut.value;
  if (!field) return;
  event.preventDefault();
  event.stopImmediatePropagation();
  if (event.key === "Escape") {
    recordingShortcut.value = null;
    shortcutError.value = "";
    return;
  }
  if (event.isComposing || event.repeat) return;
  await captureShortcut(field, event);
}

onMounted(async () => {
  window.addEventListener("keydown", onShortcutKeydown, true);
  currentVersion.value = await getVersion().catch(() => configStore.config.version);
  await refreshNotificationPermission();
});

onBeforeUnmount(() => window.removeEventListener("keydown", onShortcutKeydown, true));
</script>

<template>
  <main class="settings-window">
    <header class="settings-header" @pointerdown="dragSettings"><strong>设置</strong><button class="close-btn" title="关闭" aria-label="关闭" @click="close"><Close16 /></button></header>
    <section class="settings-body">
      <label class="settings-label">字体</label>
      <div class="font-picker" :class="{ open: fontPickerOpen }">
        <button class="font-picker-trigger" @click="fontPickerOpen = !fontPickerOpen" :style="{ fontFamily: configStore.config.general.fontFamily }"><span>{{ configStore.config.general.fontFamily }}</span><ChevronDown16 /></button>
        <div v-if="fontPickerOpen" class="font-picker-menu">
          <span v-if="fontLoading" class="font-loading">正在加载系统字体...</span>
          <template v-else>
            <input v-model="fontSearch" class="font-search" placeholder="搜索字体..." />
            <div class="font-options"><button v-for="font in filteredFonts" :key="font" class="font-option" :class="{ selected: font === configStore.config.general.fontFamily }" :style="{ fontFamily: font }" @click="setFontFamily(font)">{{ font }}</button><span v-if="!filteredFonts.length" class="font-empty">未找到字体</span></div>
          </template>
        </div>
      </div>
      <label class="settings-label">背景透明度 {{ configStore.config.window.opacity }}%</label>
      <input type="range" min="20" max="100" :value="configStore.config.window.opacity" @change="setOpacity(Number(($event.target as HTMLInputElement).value))" />
      <label class="settings-check"><input type="checkbox" :checked="configStore.config.window.autoHide" @change="configStore.update(({ window }) => ({ window: { ...window, autoHide: ($event.target as HTMLInputElement).checked } }))" /> 鼠标离开后收起便签</label>
      <label v-if="configStore.config.window.autoHide" class="settings-label">收起延迟
        <select class="settings-select" :value="configStore.config.window.autoHideDelay" @change="configStore.update(({ window }) => ({ window: { ...window, autoHideDelay: Number(($event.target as HTMLSelectElement).value) } }))">
          <option v-for="delay in [10, 30, 60, 120]" :key="delay" :value="delay">{{ delay }} 秒</option>
        </select>
      </label>
      <label for="task-completion-mode" class="settings-label">任务完成方式</label>
      <select id="task-completion-mode" class="settings-select" :value="configStore.config.general.taskCompletionMode" @change="setTaskCompletionMode(($event.target as HTMLSelectElement).value as typeof configStore.config.general.taskCompletionMode)">
        <option value="checkbox">仅复选框</option>
        <option value="gesture">仅手势</option>
        <option value="both">复选框 + 手势</option>
      </select>
      <details class="category-manager">
        <summary>管理任务分类（{{ tabStore.categories.length }}）</summary>
        <div v-for="category in tabStore.categories" :key="category.id" class="category-row">
          <span class="category-swatch" :style="{ backgroundColor: category.color }"></span>
          <span class="category-name">{{ category.name }}</span>
          <button type="button" @click="editCategory(category)">编辑</button>
          <button type="button" :class="{ confirm: deleteCategoryId === category.id }" :disabled="tabStore.categories.length <= 1 || categoryBusy" @click="removeCategory(category)">{{ deleteCategoryId === category.id ? '确认删' : '删除' }}</button>
        </div>
        <form class="category-edit" @submit.prevent="saveCategory">
          <input v-model="categoryDraft.name" maxlength="30" :placeholder="categoryDraft.id ? '分类名称' : '新增分类名称'" aria-label="分类名称" required />
          <input v-model="categoryDraft.color" type="color" aria-label="分类颜色" title="分类颜色" />
          <button type="submit" :disabled="categoryBusy">{{ categoryDraft.id ? '保存修改' : '新增' }}</button>
          <button v-if="categoryDraft.id" type="button" @click="categoryDraft = { id: '', name: '', color: '#9B9B9B' }">取消</button>
        </form>
        <p v-if="categoryStatus" class="update-status" role="status">{{ categoryStatus }}</p>
      </details>
      <label class="settings-check"><input type="checkbox" :checked="configStore.config.general.launchOnStartup" @change="setLaunchOnStartup(($event.target as HTMLInputElement).checked)" /> 开机自启动</label>
      <label class="settings-label">快捷键</label>
      <div class="shortcut-list">
        <div v-for="item in ([['newTask', '新建任务'], ['newTab', '新建 Tab'], ['search', '搜索'], ['showWindow', '打开主界面']] as const)" :key="item[0]" class="shortcut-row">
          <span>{{ item[1] }}</span>
          <button class="shortcut-input" @click="beginShortcutRecording(item[0])">
            {{ recordingShortcut === item[0] ? '请按键...' : (configStore.config.general.shortcuts[item[0]] || '未绑定') }}
          </button>
        </div>
      </div>
      <p v-if="shortcutError" class="update-status" role="alert">{{ shortcutError }}</p>
      <label class="settings-check"><input type="checkbox" :checked="configStore.config.notification.enabled" @change="setNotificationsEnabled(($event.target as HTMLInputElement).checked)" /> 桌面通知</label>
      <label class="settings-label">提前通知小时数</label>
      <input class="hours-input" type="number" min="0" max="168" step="1" :value="configStore.config.notification.reminderHours" @change="setReminderHours(Number(($event.target as HTMLInputElement).value))" />
      <label class="settings-check"><input type="checkbox" :checked="configStore.config.notification.soundEnabled" @change="setSoundEnabled(($event.target as HTMLInputElement).checked)" /> 通知声音</label>
      <div class="notification-actions"><span>权限：{{ notificationPermission === "granted" ? "已允许" : notificationPermission === "denied" ? "未允许" : "未知" }}</span><button class="update-btn" @click="testNotification">测试通知</button></div>
      <p v-if="notificationStatus" class="update-status" role="status">{{ notificationStatus }}</p>
      <button class="update-btn" type="button" @click="emit('openTrash')">打开回收站</button>
      <button class="update-btn" type="button" :aria-expanded="backupsOpen" :disabled="backupBusy" @click="backupsOpen ? backupsOpen = false : loadBackups()">任务备份</button>
      <div v-if="backupsOpen">
        <label v-if="backups.length" class="settings-label">选择备份
          <select v-model="backupId" class="settings-select" :disabled="backupBusy" @change="confirmBackup = false">
            <option v-for="backup in backups" :key="backup.id" :value="backup.id">{{ backupLabel(backup) }}</option>
          </select>
        </label>
        <p v-else-if="!backupBusy">暂无可用的任务备份</p>
        <p v-if="confirmBackup">恢复会替换任务、列表、分类和回收站内容；当前数据会先另存为恢复前备份。</p>
        <button v-if="backupId" class="update-btn" type="button" :disabled="backupBusy" @click="confirmBackup ? restoreBackup() : confirmBackup = true">{{ confirmBackup ? '确认恢复' : '恢复所选备份' }}</button>
        <button v-if="confirmBackup" class="update-btn" type="button" :disabled="backupBusy" @click="confirmBackup = false">取消</button>
      </div>
      <p v-if="backupStatus" class="update-status" role="status">{{ backupStatus }}</p>
      <p v-if="configStore.error" class="update-status" role="alert">{{ configStore.error }}</p>
      <div class="version-label">当前版本 v{{ currentVersion }}</div>
      <button class="update-btn" :disabled="updateBusy" @click="checkForUpdates">{{ updateBusy ? "处理中..." : "检查更新" }}</button>
      <div v-if="availableVersion" class="update-release">新版本 v{{ availableVersion }}<div class="update-notes">{{ updateNotes }}</div></div>
      <div v-if="updateBusy && updateProgress > 0" class="update-progress"><span :style="{ width: `${updateProgress}%` }"></span></div>
      <span v-if="updateStatus" class="update-status">{{ updateStatus }}</span>
    </section>

    <div v-if="showUpdateConfirmation" class="update-dialog-backdrop">
      <section class="update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-confirmation-title">
        <strong id="update-confirmation-title">发现新版本 v{{ availableVersion }}</strong>
        <p>确认后将下载更新，并在重启应用时开始安装。</p>
        <div class="update-confirm-notes">{{ updateNotes }}</div>
        <div class="update-dialog-actions">
          <button class="update-dialog-cancel" @click="postponeUpdate">稍后再说</button>
          <button class="update-dialog-confirm" @click="downloadAndInstallUpdate">立即下载并安装</button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.settings-window { position:absolute; z-index:20; inset:0; height:100%; min-height:0; display:flex; flex-direction:column; box-sizing:border-box; color:var(--text); background:var(--bg); border:1px solid var(--border); overflow:hidden; }
.settings-header { flex:0 0 36px; height:36px; display:flex; align-items:center; justify-content:space-between; padding:0 12px; background:var(--bg-soft); border-bottom:1px solid var(--border); cursor:grab; }
.settings-header:active { cursor:grabbing; }
.settings-header strong { font-size:13px; }
.close-btn { width:24px; height:24px; display:inline-flex; align-items:center; justify-content:center; color:var(--text-soft); border-radius:4px; }
.close-btn:hover { background:rgba(0,0,0,.06); color:var(--text); }
.close-btn :deep(svg) { width:16px; height:16px; }
.settings-body { flex:1 1 auto; min-height:0; padding:12px; font-size:11px; overflow-y:auto; scrollbar-width:none; }
.settings-body::-webkit-scrollbar { display:none; }
.shortcut-list { display:grid; gap:5px; }
.shortcut-row { display:grid; grid-template-columns:1fr 130px; gap:6px; align-items:center; }
.shortcut-input { min-width:0; padding:5px 6px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:11px; text-align:center; }
.shortcut-input:focus { border-color:var(--accent); outline:none; }
.settings-label { display:block; margin:8px 0 4px; color:var(--text-soft); }
.settings-body input[type=range] { display:block; width:100%; }
.hours-input { width:72px; padding:5px 6px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:11px; }
.settings-select { width:100%; padding:5px 6px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:11px; }
.font-picker { position:relative; }
.font-picker-trigger { width:100%; display:flex; justify-content:space-between; align-items:center; padding:6px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:11px; }
.font-picker-trigger :deep(svg) { width:14px; height:14px; }
.font-picker-menu { position:absolute; z-index:3; left:0; right:0; top:100%; margin-top:3px; padding:5px; background:var(--bg); border:1px solid var(--border); box-shadow:0 4px 12px rgba(0,0,0,.12); }
.font-search { width:100%; box-sizing:border-box; padding:5px; border:1px solid var(--border); background:var(--bg-soft); color:var(--text); font-size:11px; }
.font-options { max-height:130px; overflow:auto; }
.font-option { display:block; width:100%; padding:5px; text-align:left; color:var(--text); font-size:11px; }
.font-option:hover,.font-option.selected { background:var(--bg-soft); }
.font-loading,.font-empty { display:block; padding:6px; color:var(--text-soft); font-size:11px; }
.settings-check { display:flex; align-items:center; gap:6px; margin-top:12px; }
.category-manager { margin-top:12px; padding:7px; border:1px solid var(--border); border-radius:4px; }
.category-manager summary { cursor:pointer; color:var(--text); font-weight:600; }
.category-row { display:flex; align-items:center; gap:5px; margin-top:6px; }
.category-swatch { width:12px; height:12px; border-radius:50%; flex:0 0 12px; }
.category-name { flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; }
.category-row button,.category-edit button { padding:4px 6px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:10px; }
.category-row button.confirm { color:#b42318; border-color:#b42318; }
.category-edit { display:flex; align-items:center; gap:4px; margin-top:8px; }
.category-edit input[type="text"],.category-edit input:not([type]) { flex:1; min-width:0; padding:5px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:10px; }
.category-edit input[type="color"] { width:26px; height:25px; padding:1px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); }
.category-edit button:disabled { opacity:.45; }
.version-label { margin-top:14px; color:var(--text-soft); font-size:10px; }
.update-btn { width:100%; margin-top:8px; padding:6px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:11px; }
.update-release { margin-top:6px; font-size:11px; }
.update-notes { margin-top:3px; white-space:pre-wrap; max-height:80px; overflow:auto; color:var(--text-soft); font-size:10px; }
.update-progress { height:3px; margin-top:6px; background:var(--border); }
.update-progress span { display:block; height:100%; background:var(--accent); }
.update-status { display:block; margin-top:6px; color:var(--text-soft); font-size:10px; }
.update-dialog-backdrop { position:absolute; z-index:30; inset:0; display:flex; align-items:center; justify-content:center; padding:16px; background:rgba(0,0,0,.28); }
.update-dialog { width:min(292px, 100%); max-height:100%; overflow:auto; box-sizing:border-box; padding:14px; border:1px solid var(--border); border-radius:6px; background:var(--bg); box-shadow:0 10px 28px rgba(0,0,0,.24); }
.update-dialog strong { display:block; font-size:13px; }
.update-dialog p { margin:8px 0; color:var(--text-soft); font-size:11px; line-height:1.5; }
.update-confirm-notes { max-height:92px; overflow:auto; padding:7px; border-radius:4px; background:var(--bg-soft); color:var(--text-soft); font-size:10px; line-height:1.45; white-space:pre-wrap; }
.update-dialog-actions { display:flex; gap:8px; margin-top:12px; }
.update-dialog-actions button { flex:1; padding:6px 8px; border:1px solid var(--border); border-radius:4px; font-size:11px; }
.update-dialog-cancel { background:var(--bg-soft); color:var(--text); }
.update-dialog-confirm { border-color:var(--accent) !important; background:var(--accent); color:#fff; }
</style>
