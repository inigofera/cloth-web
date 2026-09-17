<template>
  <header :class="['md-appbar', { 'md-appbar--elevated': elevation > 0 }]">
    <div class="md-appbar__start">
      <MdIconButton
        v-if="$slots.nav || showNav"
        :icon="navIcon"
        label="Open navigation"
        @click="$emit('nav-click')"
      >
        <slot name="nav" />
      </MdIconButton>

      <h1 v-if="title || $slots.title" class="md-appbar__title">
        <slot name="title">{{ title }}</slot>
      </h1>
    </div>

    <div class="md-appbar__end">
      <slot name="actions" />
    </div>
  </header>
</template>

<script setup lang="ts">
import MdIconButton from './MdIconButton.vue';
import type { IconName } from './icons';

withDefaults(
  defineProps<{
    title?: string;
    elevation?: 0 | 1 | 2;
    showNav?: boolean;
    navIcon?: IconName;
  }>(),
  {
    title: '',
    elevation: 0,
    showNav: true,
    navIcon: 'menu',
  },
);

defineEmits<{
  (e: 'nav-click'): void;
}>();
</script>

<style scoped>
.md-appbar {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: var(--md-appbar-height);
  padding: 0 8px 0 4px;
  background: var(--md-surface);
  flex-shrink: 0;
  z-index: 10;
}

.md-appbar--elevated {
  box-shadow: var(--md-elevation-1);
}

.md-appbar__start {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.md-appbar__title {
  font-size: var(--md-title-large-size);
  line-height: var(--md-title-large-line);
  font-weight: var(--md-title-large-weight);
  letter-spacing: 0px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.md-appbar__end {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
