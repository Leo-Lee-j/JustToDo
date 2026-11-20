import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { Tab } from "@/types";
import { useTaskStore } from "./taskStore";

export const useTabStore = defineStore("tab", () => {
  const taskStore = useTaskStore();
  const selectedTab = ref("");
  const tabs = computed(() => taskStore.data.tabs);
  const sortedTabs = computed(() => [...tabs.value].sort((a, b) => Number(b.pinned) - Number(a.pinned) || a.order - b.order));
  const categories = computed(() => taskStore.data.categories);
  const categoryMap = computed(() => Object.fromEntries(categories.value.map((category) => [category.id, category])));
  const activeTabId = computed(() => tabs.value.some((tab) => tab.id === selectedTab.value) ? selectedTab.value : sortedTabs.value[0]?.id ?? "");

  return {
    tabs, sortedTabs, categories, categoryMap, activeTabId,
    setActive: (id: string) => { selectedTab.value = id; },
    create: (name: string) => taskStore.mutate<Tab>("create_tab", { name }),
    rename: (id: string, name: string) => taskStore.mutate<void>("rename_tab", { id, name }),
    update: (tab: Pick<Tab, "id" | "name" | "color" | "pinned">) => taskStore.mutate<void>("update_tab", tab),
    reorder: (ids: string[]) => taskStore.mutate<void>("reorder_tabs", { orderedIds: ids }),
    createCategory: (name: string, color: string) => taskStore.mutate("create_category", { name, color }),
    updateCategory: (id: string, name: string, color: string) => taskStore.mutate<void>("update_category", { id, name, color }),
    removeCategory: (id: string) => taskStore.mutate<void>("delete_category", { id }),
    remove: (id: string) => taskStore.mutate<void>("delete_tab", { id }),
  };
});
