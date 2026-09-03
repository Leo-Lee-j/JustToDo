// JustToDo 应用级快捷键：窗口内 keydown 监听
// 全局快捷键（任意应用聚焦触发）需 tauri-plugin-global-shortcut，MVP 暂用窗口内监听。

import type { ShortcutConfig } from "@/types";

export interface ShortcutHandlers {
  newTask?: () => void;
  newTab?: () => void;
  search?: () => void;
  switchTab?: (n: number) => void;
  complete?: () => void;
  delete?: () => void;
  escape?: () => void;
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
    // 新建任务
    if (matches(e, keys.newTask)) {
      if (!handlers.newTask) return;
      e.preventDefault();
      handlers.newTask?.();
    }
    // 新建标签页
    else if (matches(e, keys.newTab)) {
      if (!handlers.newTab) return;
      e.preventDefault();
      handlers.newTab?.();
    }
    // 搜索
    else if (matches(e, keys.search)) {
      if (!handlers.search) return;
      e.preventDefault();
      handlers.search?.();
    }
    // 完成任务 (Space)
    else if (e.key === " " && !e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey) {
      const target = e.target as HTMLElement;
      // 只在非输入元素且非按钮上触发
      if (target.tagName !== "INPUT" && target.tagName !== "TEXTAREA" && target.tagName !== "BUTTON") {
        if (!handlers.complete) return;
        e.preventDefault();
        handlers.complete?.();
      }
    }
    // 删除任务 (Ctrl+D 或 Delete)
    else if ((e.ctrlKey || e.metaKey) && e.key === "d" && !e.shiftKey && !e.altKey) {
      if (!handlers.delete) return;
      e.preventDefault();
      handlers.delete?.();
    }
    else if (e.key === "Delete" && !e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey) {
      const target = e.target as HTMLElement;
      if (target.tagName !== "INPUT" && target.tagName !== "TEXTAREA") {
        if (!handlers.delete) return;
        e.preventDefault();
        handlers.delete?.();
      }
    }
    // 取消/关闭 (Esc)
    else if (e.key === "Escape") {
      if (!handlers.escape) return;
      e.preventDefault();
      handlers.escape?.();
    }

    // 切换标签页 (Ctrl+1-9)
    if (handlers.switchTab && (e.ctrlKey || e.metaKey) && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      handlers.switchTab(parseInt(e.key, 10));
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
