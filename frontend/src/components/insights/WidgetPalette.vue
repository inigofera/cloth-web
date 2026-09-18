<template>
  <div class="palette-overlay" @click.self="$emit('close')">
    <div class="palette" role="dialog" aria-label="Add widget">
      <header class="palette__header">
        <h3 class="palette__title">Add widget</h3>
        <button type="button" class="palette__close" aria-label="Close" @click="$emit('close')">
          <MdIcon name="close" :size="18" />
        </button>
      </header>

      <div class="palette__groups">
        <section v-for="group in groups" :key="group.id" class="palette__group">
          <h4 class="palette__group-title">{{ group.label }}</h4>
          <div class="palette__items">
            <button
              v-for="def in group.defs"
              :key="def.type"
              type="button"
              class="palette__item"
              @click="$emit('add', def.type)"
            >
              <span class="palette__item-title">{{ def.title }}</span>
              <span class="palette__item-desc">{{ def.description }}</span>
            </button>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { MdIcon } from '../../ui';
import { WIDGET_CATALOG } from '../../lib/insights/catalog';
import { WIDGET_CATEGORIES, type WidgetDef } from '../../lib/insights/types';

defineEmits<{
  add: [type: string];
  close: [];
}>();

const groups = computed(() =>
  WIDGET_CATEGORIES.map(cat => ({
    id: cat.id,
    label: cat.label,
    defs: [...WIDGET_CATALOG.values()].filter((d): d is WidgetDef => d.category === cat.id),
  })).filter(g => g.defs.length > 0),
);
</script>

<style scoped>
.palette-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  background: color-mix(in srgb, var(--md-scrim) 40%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
}

.palette {
  width: 100%;
  max-width: 640px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  background: var(--md-surface-container-low);
  border-radius: var(--md-shape-large);
  box-shadow: var(--md-elevation-4);
  overflow: hidden;
}

.palette__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1.25rem 0.75rem;
}

.palette__title {
  margin: 0;
  font-size: var(--md-title-large-size);
  line-height: var(--md-title-large-line);
  color: var(--md-on-surface);
}

.palette__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: var(--md-shape-full);
  background: transparent;
  color: var(--md-on-surface-variant);
  cursor: pointer;
}

.palette__close:hover {
  background: var(--md-surface-container-highest);
}

.palette__groups {
  overflow-y: auto;
  padding: 0 1.25rem 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.palette__group-title {
  margin: 0 0 0.5rem;
  font-size: var(--md-label-large-size);
  font-weight: var(--md-label-large-weight);
  color: var(--md-primary);
  text-transform: uppercase;
  letter-spacing: 0.4px;
}

.palette__items {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 0.5rem;
}

.palette__item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  text-align: left;
  padding: 0.6rem 0.75rem;
  background: var(--md-surface-container);
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-small);
  cursor: pointer;
  transition:
    border-color var(--md-motion-duration-short) var(--md-motion-easing-standard),
    background var(--md-motion-duration-short) var(--md-motion-easing-standard);
}

.palette__item:hover {
  border-color: var(--md-primary);
  background: var(--md-primary-container);
}

.palette__item-title {
  font-size: var(--md-body-medium-size);
  font-weight: var(--md-body-medium-weight);
  color: var(--md-on-surface);
}

.palette__item-desc {
  font-size: var(--md-body-small-size);
  line-height: var(--md-body-small-line);
  color: var(--md-on-surface-variant);
}
</style>
