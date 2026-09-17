<template>
  <button
    :class="['md-btn', `md-btn--${variant}`]"
    :disabled="disabled"
    type="button"
  >
    <span v-if="$slots.icon" class="md-btn__icon"><slot name="icon" /></span>
    <span class="md-btn__label"><slot /></span>
  </button>
</template>

<script setup lang="ts">
defineProps<{
  variant?: 'filled' | 'tonal' | 'outlined' | 'text' | 'error';
  disabled?: boolean;
}>();
</script>

<style scoped>
.md-btn {
  position: relative;
  isolation: isolate;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 40px;
  padding: 0 24px;
  border: none;
  border-radius: var(--md-shape-full);
  font-size: var(--md-label-large-size);
  line-height: var(--md-label-large-line);
  font-weight: var(--md-label-large-weight);
  letter-spacing: 0.1px;
  cursor: pointer;
  user-select: none;
  transition:
    box-shadow var(--md-motion-duration-short) var(--md-motion-easing-standard);
}

/* state layer */
.md-btn::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: -1;
  border-radius: inherit;
  background: currentColor;
  opacity: 0;
  transition: opacity var(--md-motion-duration-extra-short) var(--md-motion-easing-standard);
}

.md-btn:hover:not(:disabled)::before {
  opacity: var(--md-state-hover);
}

.md-btn:focus-visible::before {
  opacity: var(--md-state-focus);
}

.md-btn:active:not(:disabled)::before {
  opacity: var(--md-state-pressed);
}

/* filled */
.md-btn--filled {
  background: var(--md-primary);
  color: var(--md-on-primary);
}

.md-btn--filled:not(:disabled) {
  box-shadow: var(--md-elevation-0);
}

.md-btn--filled:not(:disabled):hover {
  box-shadow: var(--md-elevation-1);
}

/* tonal */
.md-btn--tonal {
  background: var(--md-secondary-container);
  color: var(--md-on-secondary-container);
}

/* outlined */
.md-btn--outlined {
  background: transparent;
  color: var(--md-primary);
  box-shadow: inset 0 0 0 1px var(--md-outline);
}

/* error (filled) */
.md-btn--error {
  background: var(--md-error);
  color: var(--md-on-error);
}

.md-btn--error:not(:disabled) {
  box-shadow: var(--md-elevation-0);
}

.md-btn--error:not(:disabled):hover {
  box-shadow: var(--md-elevation-1);
}

/* text */
.md-btn--text {
  background: transparent;
  color: var(--md-primary);
  padding: 0 12px;
}

.md-btn:disabled {
  background: var(--md-surface-container-highest);
  color: color-mix(in srgb, var(--md-on-surface) 38%, transparent);
  cursor: default;
  box-shadow: none;
}

.md-btn__icon {
  display: inline-flex;
}
</style>
