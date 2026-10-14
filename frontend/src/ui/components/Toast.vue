<template>
  <transition name="toast-fade">
    <div v-if="show" class="toast" role="status" aria-live="polite">
      <span class="toast-message">{{ message }}</span>
      <button 
        v-if="undoAction" 
        type="button"
        class="toast-undo-btn" 
        @click="$emit('undo')"
      >
        Undo
      </button>
      <button
        type="button"
        class="toast-close-btn"
        aria-label="Close notification"
        @click="dismiss"
      >
        ✕
      </button>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { watch } from 'vue';

const props = withDefaults(
  defineProps<{
    show: boolean;
    message: string;
    undoAction?: any;
    duration?: number;
  }>(),
  {
    duration: 5000,
  }
);

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void;
  (e: 'update', value: boolean): void;
  (e: 'undo'): void;
}>();

let timer: ReturnType<typeof setTimeout> | null = null;

function dismiss() {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  emit('update:show', false);
  emit('update', false);
}

watch(
  () => props.show,
  (val) => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    if (val && props.duration > 0) {
      timer = setTimeout(() => {
        dismiss();
      }, props.duration);
    }
  },
  { immediate: true }
);
</script>

<style scoped>
.toast {
  position: fixed;
  bottom: 2rem;
  right: 2rem;
  min-width: 280px;
  max-width: 400px;
  padding: 0.85rem 1.15rem;
  background: var(--md-surface-container-high, #2b2930);
  color: var(--md-on-surface, #ffffff);
  border-radius: var(--md-shape-medium, 8px);
  box-shadow: 
    0 4px 12px rgba(0, 0, 0, 0.2),
    0 1px 3px rgba(0, 0, 0, 0.25);
  display: flex;
  align-items: center;
  gap: 0.75rem;
  z-index: 9999;
  box-sizing: border-box;
}

.toast-message {
  font-size: var(--md-body-medium-size, 0.875rem);
  line-height: var(--md-body-medium-line, 1.25rem);
  color: inherit;
  flex: 1;
}

.toast-undo-btn {
  padding: 0.25rem 0.6rem;
  border: 1px solid var(--md-primary-container, #d0bcff);
  border-radius: 4px;
  background: transparent;
  color: var(--md-primary-container, #d0bcff);
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
}

.toast-undo-btn:hover {
  background: rgba(208, 188, 255, 0.15);
}

.toast-close-btn {
  background: transparent;
  border: none;
  color: inherit;
  font-size: 0.85rem;
  cursor: pointer;
  padding: 0.2rem;
  line-height: 1;
  opacity: 0.7;
}

.toast-close-btn:hover {
  opacity: 1;
}

.toast-fade-enter-active,
.toast-fade-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}

.toast-fade-enter-from,
.toast-fade-leave-to {
  opacity: 0;
  transform: translateY(0.75rem);
}
</style>