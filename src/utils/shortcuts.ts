// JustToDo 应用级快捷键：窗口内 keydown 监听
// 全局快捷键（任意应用聚焦触发）需 tauri-plugin-global-shortcut，MVP 暂用窗口内监听。

import type { ShortcutConfig } from "@/types";

export interface ShortcutHandlers {
  newTask?: () => void;
  newTab?: () => void;
  search?: () => void;
  switchTab?: (n: number) => void;
}

function eventKey(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.metaKey) parts.push("Meta");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  parts.push(key === " " ? "Space" : key);
  return parts.join("+");
}

function matches(e: KeyboardEvent, binding: string): boolean {
  return Boolean(binding.trim()) && eventKey(e).toLowerCase() === binding.trim().toLowerCase();
}

export function installWindowShortcuts(handlers: ShortcutHandlers, bindings?: ShortcutConfig): () => void {
  const keys = bindings ?? { newTask: "Ctrl+N", newTab: "Ctrl+Shift+T", search: "Ctrl+F" };
  const onKey = (e: KeyboardEvent) => {
    if (matches(e, keys.newTask)) {
      if (!handlers.newTask) return;
      e.preventDefault();
      handlers.newTask?.();
    } else if (matches(e, keys.newTab)) {
      if (!handlers.newTab) return;
      e.preventDefault();
      handlers.newTab?.();
    } else if (matches(e, keys.search)) {
      if (!handlers.search) return;
      e.preventDefault();
      handlers.search?.();
    }
    if (handlers.switchTab && (e.ctrlKey || e.metaKey) && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      handlers.switchTab(parseInt(e.key, 10));
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
