<template>
  <div class="widget-grid">
    <WidgetCard
      v-for="(w, i) in widgets"
      :key="w.id"
      :def="catalog.get(w.type)!"
      :instance="w"
      :data="data"
      :class="{ 'widget-card--drag-over': overIndex === i && dragIndex !== null && dragIndex !== i }"
      @remove="$emit('remove', w.id)"
      @resize="s => $emit('resize', w.id, s)"
      @update-config="patch => $emit('update-config', w.id, patch)"
      @drag-start="onDragStart(i)"
      @drag-end="onDragEnd"
      @drag-over="onDragOver(i)"
      @drop="onDrop(i)"
    />

    <button type="button" class="add-tile" @click="$emit('add-request')">
      <MdIcon name="plus" :size="20" />
      <span>Add widget</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import WidgetCard from './WidgetCard.vue';
import { MdIcon } from '../../ui';
import { WIDGET_CATALOG } from '../../lib/insights/catalog';
import type { FilteredData, WidgetInstance, WidgetSize } from '../../lib/insights/types';

const catalog = WIDGET_CATALOG;

defineProps<{
  widgets: WidgetInstance[];
  data: FilteredData;
}>();

const emit = defineEmits<{
  reorder: [from: number, to: number];
  remove: [id: string];
  resize: [id: string, size: WidgetSize];
  'update-config': [id: string, patch: Record<string, unknown>];
  'add-request': [];
}>();

const dragIndex = ref<number | null>(null);
const overIndex = ref<number | null>(null);

function onDragStart(i: number) {
  dragIndex.value = i;
}

function onDragOver(i: number) {
  if (dragIndex.value === null) return;
  overIndex.value = i;
}

function onDrop(i: number) {
  if (dragIndex.value !== null && dragIndex.value !== i) {
    emit('reorder', dragIndex.value, i);
  }
  cleanup();
}

function onDragEnd() {
  cleanup();
}

function cleanup() {
  dragIndex.value = null;
  overIndex.value = null;
}
</script>

<style scoped>
.widget-grid {
  display: grid;
  grid-template-columns: repeat(12, 1fr);
  gap: 1rem;
  align-items: start;
}

.add-tile {
  grid-column: span 4;
  min-height: 120px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  border: 1px dashed var(--md-outline-variant);
  border-radius: var(--md-shape-medium);
  background: transparent;
  color: var(--md-on-surface-variant);
  font-size: var(--md-label-large-size);
  cursor: pointer;
  transition:
    background var(--md-motion-duration-short) var(--md-motion-easing-standard),
    border-color var(--md-motion-duration-short) var(--md-motion-easing-standard);
}

.add-tile:hover {
  background: var(--md-surface-container-low);
  border-color: var(--md-primary);
  color: var(--md-primary);
}

@media (max-width: 1100px) {
  .add-tile {
    grid-column: span 6;
  }
}

@media (max-width: 900px) {
  .add-tile {
    grid-column: span 12;
  }
}
</style>
