<template>
  <div
    class="widget-card"
    :data-widget-type="instance.type"
    :class="[`widget-card--${instance.size}`, { 'widget-card--dragging': dragging }]"
    :draggable="dragEnabled"
    @dragstart="onDragStart"
    @dragend="onDragEnd"
    @dragover="onDragOver"
    @drop="onDrop"
  >
    <header class="widget-card__header">
      <button
        class="widget-card__handle"
        type="button"
        title="Drag to move"
        aria-label="Drag to move"
        @mousedown="dragEnabled = true"
        @mouseup="dragEnabled = false"
      >
        <MdIcon name="drag" :size="16" />
      </button>

      <h3 class="widget-card__title">{{ def.title }}</h3>

      <div class="widget-card__menu">
        <button
          class="widget-card__menu-btn"
          type="button"
          aria-label="Widget options"
          @click="menuOpen = !menuOpen"
        >
          <MdIcon name="more-vert" :size="18" />
        </button>
        <div v-if="menuOpen" class="widget-card__menu-panel">
          <button type="button" @click="configOpen = !configOpen; menuOpen = false">
            {{ configOpen ? 'Hide settings' : 'Configure' }}
          </button>
          <button type="button" class="widget-card__menu-danger" @click="$emit('remove')">
            Remove
          </button>
        </div>
      </div>
    </header>

    <div v-if="configOpen" class="widget-card__config">
      <div class="config-row">
        <span class="config-label">Size</span>
        <div class="size-picker" role="group" aria-label="Widget size">
          <button
            v-for="s in SIZES"
            :key="s"
            type="button"
            class="size-picker__btn"
            :class="{ 'size-picker__btn--active': instance.size === s }"
            @click="$emit('resize', s)"
          >
            {{ s.toUpperCase() }}
          </button>
        </div>
      </div>

      <div v-for="field in visibleFields" :key="field.key" class="config-row">
        <span class="config-label">{{ field.label }}</span>

        <select
          v-if="field.kind === 'select'"
          class="config-input"
          :value="String(instance.config[field.key] ?? '')"
          @change="onConfigChange(field, ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="opt in field.options" :key="String(opt.value)" :value="String(opt.value)">
            {{ opt.label }}
          </option>
        </select>

        <select
          v-else-if="field.kind === 'item-select'"
          class="config-input config-input--item"
          :value="String(instance.config[field.key] ?? '')"
          @change="onConfigChange(field, ($event.target as HTMLSelectElement).value)"
        >
          <option value="">All items</option>
          <option v-for="it in itemOptions" :key="it.id" :value="it.id">{{ it.name }}</option>
        </select>

        <select
          v-else-if="field.kind === 'category-select'"
          class="config-input config-input--item"
          :value="String(instance.config[field.key] ?? '')"
          @change="onConfigChange(field, ($event.target as HTMLSelectElement).value)"
        >
          <option value="">All categories</option>
          <option v-for="c in categoryOptions" :key="c.id" :value="String(c.id)">{{ c.name }}</option>
        </select>

        <select
          v-else-if="field.kind === 'subcategory-select'"
          class="config-input config-input--item"
          :value="String(instance.config[field.key] ?? '')"
          @change="onConfigChange(field, ($event.target as HTMLSelectElement).value)"
        >
          <option value="">All subcategories</option>
          <option v-for="c in subOptionsForField(field)" :key="c.id" :value="String(c.id)">{{ c.name }}</option>
        </select>

        <input
          v-else-if="field.kind === 'number'"
          class="config-input config-input--number"
          type="number"
          :min="field.min"
          :max="field.max"
          :step="field.step ?? 1"
          :value="Number(instance.config[field.key] ?? field.min)"
          @change="onConfigChange(field, Number(($event.target as HTMLInputElement).value))"
        />

        <label v-else class="config-toggle">
          <input
            type="checkbox"
            :checked="Boolean(instance.config[field.key])"
            @change="onConfigChange(field, ($event.target as HTMLInputElement).checked)"
          />
          <span>Enabled</span>
        </label>
      </div>
    </div>

    <div class="widget-card__body">
      <component :is="def.component" :data="data" :config="instance.config" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { MdIcon } from '../../ui';
import { parseIdValue } from '../../lib/insights/metrics';
import type { ConfigField, FilteredData, WidgetDef, WidgetInstance, WidgetSize } from '../../lib/insights/types';

const SIZES: WidgetSize[] = ['sm', 'md', 'lg'];

const props = defineProps<{
  def: WidgetDef;
  instance: WidgetInstance;
  data: FilteredData;
}>();

const emit = defineEmits<{
  remove: [];
  resize: [size: WidgetSize];
  'update-config': [patch: Record<string, unknown>];
  'drag-start': [];
  'drag-end': [];
  'drag-over': [];
  drop: [];
}>();

const menuOpen = ref(false);
const configOpen = ref(false);
const dragEnabled = ref(false);
const dragging = ref(false);

const visibleFields = computed(() =>
  props.def.configSchema.filter(
    f => !f.showWhen || props.instance.config[f.showWhen.key] === f.showWhen.equals,
  ),
);

const itemOptions = computed(() =>
  [...props.data.items].sort((a, b) => a.name.localeCompare(b.name)),
);

const categoryOptions = computed(() => {
  const map = new Map<number, string>();
  for (const it of props.data.items) {
    if (it.category_id != null && it.category_name) map.set(it.category_id, it.category_name);
  }
  return [...map.entries()].map(([id, name]) => ({ id, name })).sort((a, b) => a.name.localeCompare(b.name));
});

const subcategoryOptionsAll = computed(() => {
  const map = new Map<number, { name: string; categoryId: number | null }>();
  for (const it of props.data.items) {
    if (it.subcategory_id != null && it.subcategory_name) {
      const cur = map.get(it.subcategory_id);
      if (!cur) map.set(it.subcategory_id, { name: it.subcategory_name, categoryId: it.category_id });
    }
  }
  return [...map.entries()]
    .map(([id, v]) => ({ id, name: v.name, categoryId: v.categoryId }))
    .sort((a, b) => a.name.localeCompare(b.name));
});

/** Subcategory options for a given category config value (all when no category is set). */
function subOptionsForCategory(catValue: unknown) {
  const catId = parseIdValue(catValue);
  if (catId == null) return subcategoryOptionsAll.value;
  return subcategoryOptionsAll.value.filter(c => c.categoryId === catId);
}

function subOptionsForField(field: ConfigField) {
  if (field.kind !== 'subcategory-select') return [];
  if (!field.dependsOn) return subcategoryOptionsAll.value;
  return subOptionsForCategory(props.instance.config[field.dependsOn]);
}

function onConfigChange(field: ConfigField, value: unknown) {
  const patch: Record<string, unknown> = { [field.key]: value };
  if (field.kind === 'category-select') {
    // If a subcategory field depends on this category and the currently chosen
    // subcategory no longer belongs to it, clear it to avoid an empty result.
    const subField = props.def.configSchema.find(
      f => f.kind === 'subcategory-select' && f.dependsOn === field.key,
    );
    if (subField) {
      const current = props.instance.config[subField.key];
      const stillValid = subOptionsForCategory(value).some(o => String(o.id) === String(current));
      if (!stillValid) patch[subField.key] = '';
    }
  }
  emit('update-config', patch);
}

// Clear item/category/subcategory picks that no longer exist in the dataset
// (deleted items, or ones hidden by the global filter). Without this, the
// select would display "All" while the stale id silently filters to nothing.
watch(
  () => props.data,
  () => {
    const patch: Record<string, unknown> = {};
    for (const field of props.def.configSchema) {
      const value = props.instance.config[field.key];
      if (typeof value !== 'string' || value === '') continue;
      const exists =
        field.kind === 'item-select'
          ? props.data.items.some(i => i.id === value)
          : field.kind === 'category-select'
            ? categoryOptions.value.some(c => String(c.id) === value)
            : field.kind === 'subcategory-select'
              ? subcategoryOptionsAll.value.some(c => String(c.id) === value)
              : true;
      if (!exists) patch[field.key] = '';
    }
    if (Object.keys(patch).length > 0) emit('update-config', patch);
  },
);

function onDragStart(e: DragEvent) {
  if (!dragEnabled.value) {
    e.preventDefault();
    return;
  }
  dragging.value = true;
  emit('drag-start');
}

function onDragEnd() {
  dragEnabled.value = false;
  dragging.value = false;
  emit('drag-end');
}

function onDragOver(e: DragEvent) {
  e.preventDefault();
  emit('drag-over');
}

function onDrop(e: DragEvent) {
  e.preventDefault();
  emit('drop');
}
</script>

<style scoped>
.widget-card {
  display: flex;
  flex-direction: column;
  background: var(--md-surface-container-low);
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-medium);
  box-shadow: var(--md-elevation-1);
  overflow: visible;
  transition: box-shadow var(--md-motion-duration-short) var(--md-motion-easing-standard);
}

.widget-card:hover {
  box-shadow: var(--md-elevation-2);
}

.widget-card--dragging {
  opacity: 0.5;
}

.widget-card--drag-over {
  outline: 2px solid var(--md-primary);
  outline-offset: 2px;
}

.widget-card--sm {
  grid-column: span 4;
}

.widget-card--md {
  grid-column: span 6;
}

.widget-card--lg {
  grid-column: span 12;
}

.widget-card__header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.6rem 0.75rem 0.4rem 0.5rem;
}

.widget-card__handle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: var(--md-shape-extra-small);
  background: transparent;
  color: var(--md-on-surface-variant);
  cursor: grab;
  flex-shrink: 0;
}

.widget-card__handle:hover {
  background: var(--md-surface-container-highest);
}

.widget-card__handle:active {
  cursor: grabbing;
}

.widget-card__title {
  flex: 1;
  margin: 0;
  font-size: var(--md-title-small-size);
  line-height: var(--md-title-small-line);
  font-weight: var(--md-title-small-weight);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.widget-card__menu {
  position: relative;
  flex-shrink: 0;
}

.widget-card__menu-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--md-shape-full);
  background: transparent;
  color: var(--md-on-surface-variant);
  cursor: pointer;
}

.widget-card__menu-btn:hover {
  background: var(--md-surface-container-highest);
}

.widget-card__menu-panel {
  position: absolute;
  right: 0;
  top: 32px;
  z-index: 20;
  display: flex;
  flex-direction: column;
  min-width: 140px;
  background: var(--md-surface-container-high);
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-small);
  box-shadow: var(--md-elevation-3);
  padding: 4px;
}

.widget-card__menu-panel button {
  border: none;
  background: transparent;
  text-align: left;
  font-size: var(--md-label-large-size);
  color: var(--md-on-surface);
  padding: 8px 12px;
  border-radius: var(--md-shape-extra-small);
  cursor: pointer;
}

.widget-card__menu-panel button:hover {
  background: var(--md-surface-container-highest);
}

.widget-card__menu-danger {
  color: var(--md-error) !important;
}

.widget-card__config {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin: 0 0.75rem 0.5rem;
  padding: 0.6rem 0.75rem;
  background: var(--md-surface-container);
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-small);
}

.config-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.config-label {
  font-size: var(--md-label-medium-size);
  color: var(--md-on-surface-variant);
  flex-shrink: 0;
}

.config-input {
  max-width: 180px;
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  background: var(--md-surface-container-low);
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-extra-small);
  padding: 4px 8px;
}

.config-input--number {
  max-width: 90px;
}

.config-input--item {
  max-width: 220px;
}

.config-toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  cursor: pointer;
}

.size-picker {
  display: inline-flex;
  gap: 2px;
  background: var(--md-surface-container-highest);
  border-radius: var(--md-shape-full);
  padding: 2px;
}

.size-picker__btn {
  border: none;
  background: transparent;
  font-size: var(--md-label-small-size);
  font-weight: var(--md-label-small-weight);
  color: var(--md-on-surface-variant);
  padding: 3px 10px;
  border-radius: var(--md-shape-full);
  cursor: pointer;
}

.size-picker__btn--active {
  background: var(--md-primary);
  color: var(--md-on-primary);
}

.widget-card__body {
  flex: 1;
  padding: 0.25rem 1rem 1rem;
  min-width: 0;
}

@media (max-width: 1100px) {
  .widget-card--sm {
    grid-column: span 6;
  }
}

@media (max-width: 900px) {
  .widget-card--sm,
  .widget-card--md {
    grid-column: span 12;
  }
}
</style>
