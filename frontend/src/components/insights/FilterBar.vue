<template>
  <div class="filter-bar">
    <FilterSelect
      label="Period"
      :options="PERIOD_OPTIONS"
      :selected="[filters.rangePreset]"
      :active="filters.rangePreset !== 'all'"
      :badge-text="filters.rangePreset === 'all' ? null : filters.rangePreset"
      @update="onPeriod"
    />

    <template v-if="filters.rangePreset === 'custom'">
      <input v-model="from" type="date" class="filter-date" aria-label="From date" />
      <span class="filter-sep">–</span>
      <input v-model="to" type="date" class="filter-date" aria-label="To date" />
    </template>

    <FilterSelect
      label="Category"
      :options="categoryOptions"
      :selected="filters.categories.map(String)"
      :multi="true"
      :active="filters.categories.length > 0"
      :badge-text="filters.categories.length || null"
      @update="onCategories"
    />

    <FilterSelect
      label="Color"
      :options="colorOptions"
      :selected="filters.colors"
      :multi="true"
      :active="filters.colors.length > 0"
      :badge-text="filters.colors.length || null"
      @update="onColors"
    />

    <FilterSelect
      label="Brand"
      :options="brandOptions"
      :selected="filters.brands"
      :multi="true"
      :active="filters.brands.length > 0"
      :badge-text="filters.brands.length || null"
      @update="onBrands"
    />

    <button
      type="button"
      class="filter-chip"
      :class="{ 'filter-chip--active': filters.activeOnly }"
      @click="patch({ activeOnly: !filters.activeOnly })"
    >
      Active only
    </button>

    <button v-if="anyActive" type="button" class="filter-clear" @click="onClear">
      Clear
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import FilterSelect, { type FilterOption } from './FilterSelect.vue';
import { DEFAULT_FILTERS, type GlobalFilters } from '../../lib/insights/types';

const props = defineProps<{
  filters: GlobalFilters;
  categories: { id: number; name: string }[];
  colors: { id: string; hex: string | null }[];
  brands: { id: string; name: string }[];
}>();

const emit = defineEmits<{
  update: [filters: GlobalFilters];
}>();

const PERIOD_OPTIONS: FilterOption[] = [
  { value: '30d', label: 'Last 30 days' },
  { value: '90d', label: 'Last 90 days' },
  { value: '1y', label: 'Last year' },
  { value: 'all', label: 'All time' },
  { value: 'custom', label: 'Custom range' },
];

function patch(p: Partial<GlobalFilters>) {
  emit('update', { ...props.filters, ...p });
}

const categoryOptions = computed<FilterOption[]>(() =>
  props.categories.map(c => ({ value: String(c.id), label: c.name })),
);

const colorOptions = computed<FilterOption[]>(() =>
  props.colors.map(c => ({ value: c.id, label: c.id, swatch: c.hex })),
);

const brandOptions = computed<FilterOption[]>(() =>
  props.brands.map(b => ({ value: b.id, label: b.name })),
);

const anyActive = computed(
  () =>
    props.filters.rangePreset !== 'all' ||
    props.filters.categories.length > 0 ||
    props.filters.colors.length > 0 ||
    props.filters.brands.length > 0 ||
    props.filters.activeOnly,
);

const from = computed({
  get: () => props.filters.from ?? '',
  set: (v: string) => patch({ from: v || null }),
});

const to = computed({
  get: () => props.filters.to ?? '',
  set: (v: string) => patch({ to: v || null }),
});

function onPeriod(selected: string[]) {
  patch({ rangePreset: (selected[0] as GlobalFilters['rangePreset']) ?? 'all' });
}

function onCategories(selected: string[]) {
  patch({ categories: selected.map(Number).filter(n => Number.isFinite(n)) });
}

function onColors(selected: string[]) {
  patch({ colors: selected });
}

function onBrands(selected: string[]) {
  patch({ brands: selected });
}

function onClear() {
  emit('update', { ...DEFAULT_FILTERS });
}
</script>

<style scoped>
.filter-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-bottom: 1.25rem;
}

.filter-date {
  height: 34px;
  padding: 0 8px;
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-full);
  background: var(--md-surface-container-low);
  color: var(--md-on-surface);
  font-size: var(--md-label-large-size);
}

.filter-sep {
  color: var(--md-on-surface-variant);
}

.filter-chip {
  height: 34px;
  padding: 0 14px;
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-full);
  background: var(--md-surface-container-low);
  color: var(--md-on-surface-variant);
  font-size: var(--md-label-large-size);
  cursor: pointer;
}

.filter-chip:hover {
  background: var(--md-surface-container);
}

.filter-chip--active {
  background: var(--md-secondary-container);
  border-color: var(--md-secondary);
  color: var(--md-on-secondary-container);
}

.filter-clear {
  height: 34px;
  padding: 0 14px;
  border: none;
  border-radius: var(--md-shape-full);
  background: transparent;
  color: var(--md-primary);
  font-size: var(--md-label-large-size);
  font-weight: var(--md-label-large-weight);
  cursor: pointer;
}

.filter-clear:hover {
  background: var(--md-primary-container);
}
</style>
