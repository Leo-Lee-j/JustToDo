import { defineStore } from "pinia";
import { ref } from "vue";
import type { Config } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { syncSnapshot } from "@/utils/snapshot";

const DEFAULT_CONFIG: Config = {
  window: { x: 0, y: 20, width: 360, height: 480, alwaysOnTop: false, opacity: 100, autoHide: false, autoHideDelay: 30 },
  notification: { enabled: true, reminderHours: 1, reminderType: "1hour", soundEnabled: true },
  general: {
    launchOnStartup: false, singleInstance: true, fontFamily: "Microsoft YaHei",
    shortcuts: { newTask: "Ctrl+N", newTab: "Ctrl+Shift+T", search: "Ctrl+F", showWindow: "Ctrl+Shift+Space" },
    taskCompletionMode: "both",
  },
  theme: "light",
  version: "",
};

export const useConfigStore = defineStore("config", () => {
  const config = ref(DEFAULT_CONFIG);
  const error = ref("");
  const { load, refresh } = syncSnapshot("config:changed", async () => {
    config.value = await invoke<Config>("get_config");
    error.value = "";
  }, (err) => { error.value = "无法读取设置：" + String(err); });
  let saving = Promise.resolve();

  function update(patch: (current: Config) => Partial<Config>) {
    const next = saving.then(async () => {
      try {
        await invoke("set_config", { config: { ...config.value, ...patch(config.value) } });
        await refresh();
        return true;
      } catch (err) {
        error.value = "无法保存设置：" + String(err);
        return false;
      }
    });
    saving = next.then(() => undefined);
    return next;
  }

  return { config, error, load, update };
});
