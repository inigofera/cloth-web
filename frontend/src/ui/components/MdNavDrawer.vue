<template>
  <aside class="md-drawer" :class="{ 'md-drawer--closed': !open }">
    <div class="md-drawer__inner">
      <div v-if="$slots.header" class="md-drawer__header">
        <slot name="header" />
      </div>

      <nav class="md-drawer__list" aria-label="Primary">
        <MdListItem
          v-for="item in items"
          :key="item.id"
          :label="item.label"
          :icon="item.icon"
          :active="modelValue === item.id"
          @click="$emit('update:modelValue', item.id)"
        />
      </nav>

      <div v-if="$slots.footer" class="md-drawer__footer">
        <slot name="footer" />
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import MdListItem from './MdListItem.vue';
import type { DrawerItem } from '../types';

withDefaults(
  defineProps<{
    items: DrawerItem[];
    modelValue: string;
    open?: boolean;
  }>(),
  {
    open: true,
  },
);

defineEmits<{
  (e: 'update:modelValue', id: string): void;
}>();
</script>

<style scoped>
.md-drawer {
  flex: 0 0 var(--md-drawer-width);
  width: var(--md-drawer-width);
  overflow: hidden;
  transition: flex-basis var(--md-motion-duration-medium) var(--md-motion-easing-emphasized),
    width var(--md-motion-duration-medium) var(--md-motion-easing-emphasized);
}

.md-drawer--closed {
  flex-basis: 0;
  width: 0;
}

.md-drawer__inner {
  display: flex;
  flex-direction: column;
  width: var(--md-drawer-width);
  height: 100%;
  background: var(--md-surface-container-low);
}

.md-drawer__header {
  padding: 16px;
  flex-shrink: 0;
}

.md-drawer__list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 0;
  flex: 1;
  overflow-y: auto;
}

.md-drawer__footer {
  padding: 8px;
  flex-shrink: 0;
  border-top: 1px solid var(--md-outline-variant);
}
</style>
