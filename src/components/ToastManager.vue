<script setup lang="ts">
import { useToast } from "@/composables/useToast";

const { toasts, removeToast } = useToast();
</script>

<template>
  <div class="toast-container">
    <TransitionGroup name="toast">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        :class="['toast', `toast-${toast.type}`]"
      >
        <span class="toast-message">{{ toast.message }}</span>
        <button
          v-if="toast.action"
          class="toast-action"
          @click="toast.action.handler(); removeToast(toast.id)"
        >
          {{ toast.action.label }}
        </button>
        <button
          class="toast-close"
          @click="removeToast(toast.id)"
          aria-label="关闭"
        >
          ✕
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-container {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 9999;
  display: flex;
  flex-direction: column-reverse;
  gap: 8px;
  pointer-events: none;
}

.toast {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 8px;
  background: var(--bg-soft);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  font-size: 13px;
  line-height: 1.4;
  min-width: 280px;
  max-width: 420px;
  pointer-events: auto;
}

.toast-success {
  background: #d7f0dc;
  color: #166534;
  border-left: 3px solid #10b981;
}

.toast-error {
  background: #fecaca;
  color: #991b1b;
  border-left: 3px solid #d0021b;
}

.toast-warning {
  background: #fef3c7;
  color: #92400e;
  border-left: 3px solid #f59e0b;
}

.toast-info {
  background: var(--bg-soft);
  color: var(--text);
  border-left: 3px solid var(--primary);
}

.toast-message {
  flex: 1;
  word-break: break-word;
}

.toast-action {
  padding: 4px 12px;
  border-radius: 4px;
  background: var(--primary);
  color: #fff;
  font-weight: 600;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}

.toast-action:hover {
  opacity: 0.9;
}

.toast-close {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  color: var(--text-soft);
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
  flex-shrink: 0;
}

.toast-close:hover {
  background: rgba(0, 0, 0, 0.1);
}

/* 动画 */
.toast-enter-active,
.toast-leave-active {
  transition: all 0.3s ease;
}

.toast-enter-from {
  opacity: 0;
  transform: translateY(20px);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(100px);
}

.toast-move {
  transition: transform 0.3s ease;
}
</style>
