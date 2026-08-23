import { defineStore } from "pinia";
import type { Config } from "@/types";
import { invoke } from "@tauri-apps/api/core";

const DEFAULT_CONFIG: Config = {
  window: {
    x: 0,
    y: 20,
    width: 360,
    height: 480,
    alwaysOnTop: false,
    opacity: 100,
    autoHide: false,
    autoHideDelay: 30,
  },
  taskbar: {
    enabled: false,
    position: "right",
    visibleCount: 3,
    title: "📌 今日要务",
  },
  notification: {
    enabled: true,
    reminderType: "1hour",
    soundEnabled: true,
  },
  general: {
    launchOnStartup: false,
    singleInstance: true,
    fontFamily: "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
  },
  theme: "light",
  version: "1.0.0",
};

export const useConfigStore = defineStore("config", {
  state: () => ({
    config: DEFAULT_CONFIG,
    loaded: false,
  }),
  actions: {
    async load() {
      try {
        const cfg = await invoke<Config>("get_config");
        this.config = {
          ...DEFAULT_CONFIG,
          ...cfg,
          general: { ...DEFAULT_CONFIG.general, ...cfg.general },
          taskbar: { ...DEFAULT_CONFIG.taskbar, ...cfg.taskbar, enabled: false },
        };
      } catch {
        this.config = DEFAULT_CONFIG;
      }
      this.loaded = true;
    },
    async save() {
      try {
        await invoke("set_config", { config: this.config });
      } catch (e) {
        console.error("save config failed", e);
      }
    },
    async update(patch: Partial<Config>) {
      this.config = { ...this.config, ...patch };
      await this.save();
    },
  },
});
