import type { ShortcutConfig } from "@/types";

export interface ShortcutHandlers {
  newTask?: () => void;
  newTab?: () => void;
  search?: () => void;
  showWindow?: () => void;
  switchTab?: (n: number) => void;
  escape?: () => void;
  undo?: () => boolean;
}

export function eventKey(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.metaKey) parts.push("Meta");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  parts.push(key === " " ? "Space" : key);
  return parts.join("+");
}

export function shortcutConflict(binding: string, bindings: ShortcutConfig, field: keyof ShortcutConfig): boolean {
  const normalized = binding.trim().toLowerCase();
  return /^(ctrl|meta)[+](?:[1-9]|z)$/.test(normalized) || Object.entries(bindings).some(([key, value]) =>
    key !== field && value.trim().toLowerCase() === normalized);
}

function isTextEditingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target.closest(
    "input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox']",
  ) !== null;
}

export function installWindowShortcuts(handlers: ShortcutHandlers, bindings: ShortcutConfig): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.defaultPrevented || e.isComposing || e.repeat) return;
    const key = eventKey(e).toLowerCase();
    if (/^(ctrl|meta)[+]z$/.test(key)) {
      const editingText = e.composedPath().some(isTextEditingTarget)
        || isTextEditingTarget(document.activeElement);
      if (!editingText && handlers.undo?.()) e.preventDefault();
      return;
    }
    for (const action of ["newTask", "newTab", "search", "showWindow"] as const) {
      const handler = handlers[action];
      if (!handler || key !== bindings[action].trim().toLowerCase()) continue;
      e.preventDefault();
      handler();
      return;
    }
    if (handlers.switchTab && /^(ctrl|meta)[+][1-9]$/.test(key)) {
      e.preventDefault();
      handlers.switchTab(parseInt(e.key, 10));
    }
    if (key === "escape") handlers.escape?.();
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
