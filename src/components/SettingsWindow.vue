<script setup lang="ts">
import { onMounted, ref, computed } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { invoke } from "@tauri-apps/api/core";
import ChevronDown16 from "@carbon/icons-vue/lib/chevron--down/16.js";
import Close16 from "@carbon/icons-vue/lib/close/16.js";
import { useConfigStore } from "@/stores/configStore";

const configStore = useConfigStore();
const emit = defineEmits<{ close: [] }>();
const currentVersion = ref("0.0.1");
const availableVersion = ref("");
const updateNotes = ref("");
const updateProgress = ref(0);
const updateStatus = ref("");
const updateBusy = ref(false);
const systemFonts = ref<string[]>([]);
const fontSearch = ref("");
const fontPickerOpen = ref(false);
const filteredFonts = computed(() => {
  const query = fontSearch.value.trim().toLowerCase();
  return query ? systemFonts.value.filter((font) => font.toLowerCase().includes(query)) : systemFonts.value;
});

async function loadSystemFonts() {
  try { systemFonts.value = await invoke<string[]>("list_system_fonts"); }
  catch { systemFonts.value = ["Microsoft YaHei", "Arial", "Segoe UI", "sans-serif", "serif", "monospace"]; }
}
async function setFontFamily(value: string) {
  await configStore.update({ general: { ...configStore.config.general, fontFamily: value } });
  fontPickerOpen.value = false;
}
async function setOpacity(value: number) {
  await configStore.update({ window: { ...configStore.config.window, opacity: value } });
  await invoke("set_opacity", { label: "sticky", opacity: value });
}
async function setLaunchOnStartup(enabled: boolean) {
  try {
    if (enabled) await enableAutostart(); else await disableAutostart();
    await configStore.update({ general: { ...configStore.config.general, launchOnStartup: enabled } });
  } catch { updateStatus.value = "无法修改开机启动设置"; }
}
async function checkForUpdates() {
  if (updateBusy.value) return;
  updateBusy.value = true; updateStatus.value = "正在检查更新..."; updateProgress.value = 0;
  try {
    const update = await check();
    if (!update) { updateStatus.value = `当前已是最新版本（v${currentVersion.value}）`; return; }
    availableVersion.value = update.version; updateNotes.value = update.body || "暂无更新说明";
    let downloaded = 0; let totalBytes = 0;
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") { downloaded = 0; totalBytes = event.data.contentLength || 0; updateStatus.value = `正在下载 v${update.version}...`; }
      else if (event.event === "Progress") { downloaded += event.data.chunkLength; if (totalBytes) updateProgress.value = Math.min(100, Math.round(downloaded / totalBytes * 100)); }
      else if (event.event === "Finished") { updateProgress.value = 100; updateStatus.value = "下载完成，正在重启安装..."; }
    });
    await relaunch();
  } catch { updateStatus.value = "检查更新失败，请稍后重试"; }
  finally { updateBusy.value = false; }
}
function close() { emit("close"); }
onMounted(async () => { await configStore.load(); currentVersion.value = await getVersion().catch(() => "0.0.1"); await loadSystemFonts(); });
</script>

<template>
  <main class="settings-window" :style="{ fontFamily: configStore.config.general.fontFamily }">
    <header class="settings-header"><strong>设置</strong><button class="close-btn" title="关闭" aria-label="关闭" @click="close"><Close16 /></button></header>
    <section class="settings-body">
      <label class="settings-label">字体</label>
      <div class="font-picker" :class="{ open: fontPickerOpen }">
        <button class="font-picker-trigger" @click="fontPickerOpen = !fontPickerOpen" :style="{ fontFamily: configStore.config.general.fontFamily }"><span>{{ configStore.config.general.fontFamily }}</span><ChevronDown16 /></button>
        <div v-if="fontPickerOpen" class="font-picker-menu"><input v-model="fontSearch" class="font-search" placeholder="搜索字体..." /><div class="font-options"><button v-for="font in filteredFonts" :key="font" class="font-option" :class="{ selected: font === configStore.config.general.fontFamily }" :style="{ fontFamily: font }" @click="setFontFamily(font)">{{ font }}</button></div></div>
      </div>
      <label class="settings-label">背景透明度 {{ configStore.config.window.opacity }}%</label>
      <input type="range" min="20" max="100" :value="configStore.config.window.opacity" @input="setOpacity(Number(($event.target as HTMLInputElement).value))" />
      <label class="settings-check"><input type="checkbox" :checked="configStore.config.general.launchOnStartup" @change="setLaunchOnStartup(($event.target as HTMLInputElement).checked)" /> 开机自启动</label>
      <div class="version-label">当前版本 v{{ currentVersion }}</div>
      <button class="update-btn" :disabled="updateBusy" @click="checkForUpdates">{{ updateBusy ? "处理中..." : "检查更新" }}</button>
      <div v-if="availableVersion" class="update-release">新版本 v{{ availableVersion }}<div class="update-notes">{{ updateNotes }}</div></div>
      <div v-if="updateBusy && updateProgress > 0" class="update-progress"><span :style="{ width: `${updateProgress}%` }"></span></div>
      <span v-if="updateStatus" class="update-status">{{ updateStatus }}</span>
    </section>
  </main>
</template>

<style scoped>
.settings-window { position:absolute; z-index:20; inset:0; min-height:100%; box-sizing:border-box; color:var(--text); background:var(--bg); border:1px solid var(--border); }
.settings-header { height:38px; display:flex; align-items:center; justify-content:space-between; padding:0 12px; background:var(--bg-soft); border-bottom:1px solid var(--border); }
.settings-header strong { font-size:13px; }
.close-btn { width:24px; height:24px; display:inline-flex; align-items:center; justify-content:center; color:var(--text-soft); border-radius:4px; }
.close-btn:hover { background:rgba(0,0,0,.06); color:var(--text); }
.close-btn :deep(svg) { width:16px; height:16px; }
.settings-body { padding:12px; font-size:11px; }
.settings-label { display:block; margin:8px 0 4px; color:var(--text-soft); }
.settings-body input[type=range] { display:block; width:100%; }
.font-picker { position:relative; }
.font-picker-trigger { width:100%; display:flex; justify-content:space-between; align-items:center; padding:6px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:11px; }
.font-picker-trigger :deep(svg) { width:14px; height:14px; }
.font-picker-menu { position:absolute; z-index:3; left:0; right:0; top:100%; margin-top:3px; padding:5px; background:var(--bg); border:1px solid var(--border); box-shadow:0 4px 12px rgba(0,0,0,.12); }
.font-search { width:100%; box-sizing:border-box; padding:5px; border:1px solid var(--border); background:var(--bg-soft); color:var(--text); font-size:11px; }
.font-options { max-height:130px; overflow:auto; }
.font-option { display:block; width:100%; padding:5px; text-align:left; color:var(--text); font-size:11px; }
.font-option:hover,.font-option.selected { background:var(--bg-soft); }
.settings-check { display:flex; align-items:center; gap:6px; margin-top:12px; }
.version-label { margin-top:14px; color:var(--text-soft); font-size:10px; }
.update-btn { width:100%; margin-top:8px; padding:6px; border:1px solid var(--border); border-radius:4px; background:var(--bg-soft); color:var(--text); font-size:11px; }
.update-release { margin-top:6px; font-size:11px; }.update-notes { margin-top:3px; white-space:pre-wrap; max-height:80px; overflow:auto; color:var(--text-soft); font-size:10px; }
.update-progress { height:3px; margin-top:6px; background:var(--border); }.update-progress span { display:block; height:100%; background:var(--accent); }
.update-status { display:block; margin-top:6px; color:var(--text-soft); font-size:10px; }
</style>
