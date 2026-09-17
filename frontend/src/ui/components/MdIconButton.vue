<template>
  <button
    :class="['md-icon-btn', `md-icon-btn--${variant}`]"
    :disabled="disabled"
    :aria-label="label"
    type="button"
  >
    <MdIcon v-if="icon" :name="icon" :size="24" />
    <slot v-else />
  </button>
</template>

<script setup lang="ts">
import { MdIcon, type IconName } from './icons';

defineProps<{
  icon?: IconName;
  label?: string;
  variant?: 'standard' | 'tonal';
  disabled?: boolean;
}>();
</script>

<style scoped>
.md-icon-btn {
  position: relative;
  isolation: isolate;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  padding: 0;
  border: none;
  border-radius: var(--md-shape-full);
  color: var(--md-on-surface-variant);
  background: transparent;
  cursor: pointer;
  flex-shrink: 0;
}

.md-icon-btn::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: -1;
  border-radius: inherit;
  background: currentColor;
  opacity: 0;
  transition: opacity var(--md-motion-duration-extra-short) var(--md-motion-easing-standard);
}

.md-icon-btn:hover:not(:disabled)::before {
  opacity: var(--md-state-hover);
}

.md-icon-btn:focus-visible::before {
  opacity: var(--md-state-focus);
}

.md-icon-btn:active:not(:disabled)::before {
  opacity: var(--md-state-pressed);
}

.md-icon-btn--tonal {
  background: var(--md-secondary-container);
  color: var(--md-on-secondary-container);
}

.md-icon-btn:disabled {
  color: color-mix(in srgb, var(--md-on-surface) 38%, transparent);
  cursor: default;
}
</style>
