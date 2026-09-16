<template>
  <button
    class="md-list-item"
    :class="{ 'md-list-item--active': active }"
    :aria-current="active ? 'true' : undefined"
    type="button"
  >
    <span class="md-list-item__icon">
      <MdIcon v-if="icon" :name="icon" :size="24" />
      <slot v-else name="icon" />
    </span>
    <span class="md-list-item__label"><slot>{{ label }}</slot></span>
    <span v-if="$slots.trailing" class="md-list-item__trailing"><slot name="trailing" /></span>
  </button>
</template>

<script setup lang="ts">
import { MdIcon, type IconName } from './icons';

defineProps<{
  label?: string;
  icon?: IconName;
  active?: boolean;
}>();
</script>

<style scoped>
.md-list-item {
  position: relative;
  isolation: isolate;
  display: flex;
  align-items: center;
  gap: 16px;
  width: calc(100% - 16px);
  margin: 2px 8px;
  height: 56px;
  padding: 0 16px;
  border: none;
  border-radius: var(--md-shape-full);
  background: transparent;
  color: var(--md-on-surface-variant);
  font-size: var(--md-label-large-size);
  line-height: var(--md-label-large-line);
  font-weight: var(--md-label-large-weight);
  text-align: left;
  cursor: pointer;
  transition: color var(--md-motion-duration-extra-short) var(--md-motion-easing-standard);
}

.md-list-item::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: -1;
  border-radius: inherit;
  background: currentColor;
  opacity: 0;
  transition: opacity var(--md-motion-duration-extra-short) var(--md-motion-easing-standard);
}

.md-list-item:hover:not(.md-list-item--active)::before {
  opacity: var(--md-state-hover);
}

.md-list-item:focus-visible:not(.md-list-item--active)::before {
  opacity: var(--md-state-focus);
}

.md-list-item:active::before {
  opacity: var(--md-state-pressed);
}

.md-list-item--active {
  background: var(--md-primary-container);
  color: var(--md-on-primary-container);
}

.md-list-item__icon {
  display: inline-flex;
  flex-shrink: 0;
}

.md-list-item__label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.md-list-item__trailing {
  display: inline-flex;
  flex-shrink: 0;
}
</style>
