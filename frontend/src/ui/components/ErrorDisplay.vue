<template>
  <div 
    class="error-display"
    :class="[`error-display--${effectiveType}`, { 'error-display--compact': compact }]"
    role="alert"
    aria-live="polite"
  >
    <span class="error-icon" aria-hidden="true">⚠️</span>
    <div class="error-content">
      <p class="error-message">{{ message }}</p>
      
      <!-- Retry button for network/recoverable errors -->
      <button 
        v-if="showRetry && !disabled"
        type="button"
        class="retry-btn"
        @click="$emit('retry')"
      >
        Retry
      </button>
    </div>

    <button
      v-if="dismissible"
      type="button"
      class="dismiss-btn"
      aria-label="Dismiss error"
      @click="$emit('dismiss')"
    >
      ✕
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  message: string;
  type?: 'network' | 'validation' | 'auth' | 'unknown';
  compact?: boolean;
  disabled?: boolean;
  dismissible?: boolean;
  canRetry?: boolean;
}>();

defineEmits<{
  retry: [];
  dismiss: [];
}>();

// Auto-detect type if not provided
const effectiveType = computed(() => props.type || 'unknown');
const showRetry = computed(() => props.canRetry ?? effectiveType.value === 'network');
</script>

<style scoped>
.error-display {
  padding: 0.75rem 1rem;
  border-radius: var(--md-shape-medium, 8px);
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  box-sizing: border-box;
}

/* Severity-based colors */
.error-display--network {
  background: #e3f2fd;
  border-left: 4px solid var(--md-primary, #6750a4);
  color: #1565c0;
}

.error-display--validation {
  background: #fff3e0;
  border-left: 4px solid #ff9800;
  color: #e65100;
}

.error-display--auth {
  background: #ffebee;
  border-left: 4px solid var(--md-error, #b3261e);
  color: #b3261e;
}

.error-display--unknown {
  background: #fdecea;
  border-left: 4px solid #d32f2f;
  color: #c0392b;
}

.error-icon {
  font-size: 1.1rem;
  flex-shrink: 0;
  line-height: 1.4;
}

.error-content {
  flex: 1;
  min-width: 0;
}

.error-message {
  margin: 0;
  font-size: 0.88rem;
  line-height: 1.4;
  word-break: break-word;
}

.retry-btn {
  margin-top: 0.35rem;
  padding: 0.25rem 0.6rem;
  border: 1px solid currentColor;
  border-radius: 4px;
  background: transparent;
  color: inherit;
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
}

.retry-btn:hover:not(:disabled) {
  background: rgba(0, 0, 0, 0.05);
}

.dismiss-btn {
  background: transparent;
  border: none;
  font-size: 0.9rem;
  color: inherit;
  cursor: pointer;
  padding: 0.1rem 0.3rem;
  line-height: 1;
  opacity: 0.7;
}

.dismiss-btn:hover {
  opacity: 1;
}

.error-display--compact {
  padding: 0.4rem 0.75rem;
  font-size: 0.82rem;
}

.error-display--compact .error-message {
  font-size: 0.82rem;
}

.error-display--compact .retry-btn {
  font-size: 0.75rem;
  padding: 0.15rem 0.45rem;
}
</style>