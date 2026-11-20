import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { Channel, invoke } from "@tauri-apps/api/core";

type UpdateInfo = { version: string; notes: string; size: number };
type UpdateProgress = { phase: "downloading" | "verifying" | "installing"; downloaded: number; total: number };
type Phase = "idle" | "checking" | "available" | "latest" | UpdateProgress["phase"];

// Keep an in-flight update alive when the settings panel is closed and reopened.
export const useUpdateStore = defineStore("update", () => {
  const update = ref<UpdateInfo | null>(null);
  const phase = ref<Phase>("idle");
  const error = ref("");
  const downloaded = ref(0);
  const total = ref(0);
  const busy = computed(() => ["checking", "downloading", "verifying", "installing"].includes(phase.value));
  const percent = computed(() => total.value > 0 ? Math.min(100, Math.floor(downloaded.value / total.value * 100)) : 0);
  const status = computed(() => {
    switch (phase.value) {
      case "checking": return "正在检查新版本…";
      case "available": return `发现新版本 v${update.value?.version}`;
      case "latest": return "当前已是最新版本";
      case "downloading": return `正在下载安装包 ${percent.value}%（${(downloaded.value / 1048576).toFixed(1)} / ${(total.value / 1048576).toFixed(1)} MB）`;
      case "verifying": return "正在校验安装包…";
      case "installing": return "正在启动安装程序，完成后将自动重启…";
      default: return "";
    }
  });

  async function check() {
    if (busy.value) return;
    phase.value = "checking";
    error.value = "";
    update.value = null;
    try {
      update.value = await invoke<UpdateInfo | null>("check_app_update");
      phase.value = update.value ? "available" : "latest";
    } catch (err) {
      phase.value = "idle";
      error.value = String(err);
    }
  }

  async function install() {
    if (busy.value || !update.value) return;
    phase.value = "downloading";
    error.value = "";
    downloaded.value = 0;
    total.value = update.value.size;
    const onProgress = new Channel<UpdateProgress>();
    let receivingProgress = true;
    onProgress.onmessage = (progress) => {
      if (!receivingProgress) return;
      phase.value = progress.phase;
      downloaded.value = progress.downloaded;
      total.value = progress.total;
    };
    try {
      await invoke("install_app_update", { onProgress });
      phase.value = "installing";
    } catch (err) {
      phase.value = "available";
      error.value = String(err);
    } finally {
      receivingProgress = false;
    }
  }

  return { update, phase, error, busy, percent, status, check, install };
});
