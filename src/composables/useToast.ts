import { ref } from "vue";

export type ToastType = "success" | "error" | "warning" | "info";

export interface ToastAction {
  label: string;
  handler: () => void;
}

export interface Toast {
  id: string;
  message: string;
  type: ToastType;
  duration: number;
  action?: ToastAction;
}

export interface ShowToastOptions {
  message: string;
  type?: ToastType;
  duration?: number;
  action?: ToastAction;
}

const toasts = ref<Toast[]>([]);
const MAX_TOASTS = 3;
let idCounter = 0;

export function useToast() {
  function showToast(options: ShowToastOptions) {
    const toast: Toast = {
      id: `toast-${++idCounter}`,
      message: options.message,
      type: options.type || "info",
      duration: options.duration || 4000,
      action: options.action,
    };

    // 限制最多 3 条，移除最早的
    if (toasts.value.length >= MAX_TOASTS) {
      toasts.value.shift();
    }

    toasts.value.push(toast);

    // 自动移除
    setTimeout(() => {
      removeToast(toast.id);
    }, toast.duration);

    return toast.id;
  }

  function removeToast(id: string) {
    const index = toasts.value.findIndex((t) => t.id === id);
    if (index >= 0) toasts.value.splice(index, 1);
  }

  return {
    toasts,
    showToast,
    removeToast,
  };
}
