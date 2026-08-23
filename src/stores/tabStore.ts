import { defineStore } from "pinia";
import type { Tab, Category } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const useTabStore = defineStore("tab", {
  state: () => ({
    tabs: [] as Tab[],
    categories: [] as Category[],
    activeTabId: "" as string,
    loaded: false,
  }),
  getters: {
    sortedTabs(state): Tab[] {
      return [...state.tabs]
        .sort((a, b) => Number(b.pinned) - Number(a.pinned) || a.order - b.order);
    },
    categoryMap(state): Record<string, Category> {
      return state.categories.reduce((m, c) => {
        m[c.id] = c;
        return m;
      }, {} as Record<string, Category>);
    },
    activeTab(state): Tab | undefined {
      return state.tabs.find((t) => t.id === state.activeTabId);
    },
  },
  actions: {
    async load() {
      const data = await invoke<{ tabs: Tab[]; categories: Category[] }>("get_tabs_and_categories");
      this.tabs = data.tabs;
      this.categories = data.categories;
      if (!this.activeTabId && this.tabs.length) this.activeTabId = this.sortedTabs[0].id;
      this.loaded = true;
      this.subscribe();
    },
    subscribe() {
      listen<{ tabs: Tab[] }>("tab:changed", (e) => {
        this.tabs = e.payload.tabs;
      });
    },
    async setActive(id: string) {
      this.activeTabId = id;
    },
    async create(name: string) {
      const tab = await invoke<Tab>("create_tab", { name });
      return tab;
    },
    async rename(id: string, name: string) {
      await invoke("rename_tab", { id, name });
    },
    async remove(id: string) {
      await invoke("delete_tab", { id });
      this.tabs = this.tabs.filter((t) => t.id !== id);
      if (this.activeTabId === id && this.tabs.length)
        this.activeTabId = this.sortedTabs[0].id;
    },
  },
});
