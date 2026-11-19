import type { ShortcutConfig } from "@/types";

export interface ShortcutHandlers {
  newTask?: () => void;
  newTab?: () => void;
  search?: () => void;
  showWindow?: () => void;
  switchTab?: (n: number) => void;
  escape?: () => void;
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
  return /^(ctrl|meta)[+][1-9]$/.test(normalized) || Object.entries(bindings).some(([key, value]) =>
    key !== field && value.trim().toLowerCase() === normalized);
}

export function installWindowShortcuts(handlers: ShortcutHandlers, bindings: ShortcutConfig): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.defaultPrevented || e.isComposing || e.repeat) return;
    const key = eventKey(e).toLowerCase();
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
