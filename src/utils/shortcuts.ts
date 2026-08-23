// JustToDo 应用级快捷键：窗口内 keydown 监听
// 全局快捷键（任意应用聚焦触发）需 tauri-plugin-global-shortcut，MVP 暂用窗口内监听。

export interface ShortcutHandlers {
  newTask?: () => void;
  newTab?: () => void;
  search?: () => void;
  switchTab?: (n: number) => void;
}

export function installWindowShortcuts(handlers: ShortcutHandlers): () => void {
  const onKey = (e: KeyboardEvent) => {
    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;
    const key = e.key.toLowerCase();
    if (key === "n" && !e.shiftKey) {
      e.preventDefault();
      handlers.newTask?.();
    } else if (key === "t" && e.shiftKey) {
      e.preventDefault();
      handlers.newTab?.();
    } else if (key === "f") {
      e.preventDefault();
      handlers.search?.();
    } else if (/^[1-9]$/.test(key)) {
      e.preventDefault();
      handlers.switchTab?.(parseInt(key, 10));
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
