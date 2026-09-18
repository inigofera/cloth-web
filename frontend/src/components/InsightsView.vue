<template>
  <div class="insights-view">
    <div v-if="loading" class="loading">Loading insights…</div>
    <div v-else-if="loadError" class="error">{{ loadError }}</div>

    <template v-else>
      <div class="insights-header">
        <h2 class="insights-title">Insights</h2>
        <div class="insights-actions">
          <MdButton variant="text" @click="resetLayout">Reset layout</MdButton>
          <MdButton variant="tonal" @click="paletteOpen = true">Add widget</MdButton>
        </div>
      </div>

      <FilterBar
        :filters="filters"
        :categories="filterOptions.categories"
        :colors="filterOptions.colors"
        :brands="filterOptions.brands"
        @update="filters = $event"
      />

      <div v-if="itemsCapped" class="cap-notice">
        Showing the first 100 clothing items — the API is capped at 100 rows per table.
      </div>

      <WidgetGrid
        v-if="widgets.length > 0"
        :widgets="widgets"
        :data="filteredData"
        @reorder="reorderWidgets"
        @remove="removeWidget"
        @resize="resizeWidget"
        @update-config="updateWidgetConfig"
        @add-request="paletteOpen = true"
      />
      <div v-else class="empty">No widgets yet — add one to get started.</div>

      <WidgetPalette v-if="paletteOpen" @add="addWidget" @close="paletteOpen = false" />
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { api } from '../api/client';
import { MdButton } from '../ui';
import FilterBar from './insights/FilterBar.vue';
import WidgetGrid from './insights/WidgetGrid.vue';
import WidgetPalette from './insights/WidgetPalette.vue';
import { WIDGET_CATALOG } from '../lib/insights/catalog';
import { applyFilters } from '../lib/insights/metrics';
import { createInstance, defaultLayout, loadLayout, saveLayout } from '../lib/insights/store';
import {
  DEFAULT_FILTERS,
  type EnrichedItem,
  type EnrichedOutfit,
  type FilteredData,
  type GlobalFilters,
  type InsightDataset,
  type WidgetInstance,
  type WidgetSize,
} from '../lib/insights/types';
import type {
  Brand,
  ClothingCategory,
  ClothingItem,
  ClothingSubcategory,
  Color,
  Outfit,
} from '../api/types';

const loading = ref(true);
const loadError = ref<string | null>(null);
const dataset = ref<InsightDataset | null>(null);
const filters = ref<GlobalFilters>({ ...DEFAULT_FILTERS });
const paletteOpen = ref(false);
const widgets = ref<WidgetInstance[]>(loadLayout(WIDGET_CATALOG));

watch(widgets, () => saveLayout(widgets.value), { deep: true });

const filteredData = computed<FilteredData>(() =>
  dataset.value ? applyFilters(dataset.value, filters.value) : { items: [], outfits: [] },
);

const itemsCapped = computed(() => (dataset.value?.items.length ?? 0) >= 100);

const filterOptions = computed(() => {
  const ds = dataset.value;
  if (!ds) return { categories: [] as { id: number; name: string }[], colors: [] as { id: string; hex: string | null }[], brands: [] as { id: string; name: string }[] };
  return {
    categories: [...ds.categories.entries()].map(([id, name]) => ({ id, name })),
    colors: [...ds.colors.values()].map(c => ({ id: c.id, hex: c.hex_value })),
    brands: [...ds.brands.entries()].map(([id, name]) => ({ id, name })),
  };
});

// ---------------------------------------------------------------------------
// Widget actions
// ---------------------------------------------------------------------------

function addWidget(type: string) {
  const inst = createInstance(type, WIDGET_CATALOG);
  if (inst) widgets.value.push(inst);
  paletteOpen.value = false;
}

function removeWidget(id: string) {
  widgets.value = widgets.value.filter(w => w.id !== id);
}

function resizeWidget(id: string, size: WidgetSize) {
  const w = widgets.value.find(x => x.id === id);
  if (w) w.size = size;
}

function updateWidgetConfig(id: string, patch: Record<string, unknown>) {
  const w = widgets.value.find(x => x.id === id);
  if (w) w.config = { ...w.config, ...patch };
}

function reorderWidgets(from: number, to: number) {
  const arr = [...widgets.value];
  const [moved] = arr.splice(from, 1);
  arr.splice(to, 0, moved);
  widgets.value = arr;
}

function resetLayout() {
  widgets.value = defaultLayout(WIDGET_CATALOG);
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

function buildDataset(
  rawItems: ClothingItem[],
  rawOutfits: Outfit[],
  colors: Color[],
  categories: ClothingCategory[],
  subcategories: ClothingSubcategory[],
  brands: Brand[],
): InsightDataset {
  const colorMap = new Map(colors.map(c => [c.id, c]));
  const catMap = new Map(categories.map(c => [c.id, c.name]));
  const subMap = new Map(subcategories.map(s => [s.id, { name: s.name, category_id: s.category_id }]));
  const brandMap = new Map(brands.map(b => [b.id, b.name]));

  const enrich = (raw: Record<string, unknown>): EnrichedItem => {
    const id = typeof raw.id === 'string' ? raw.id : '';
    const categoryId = typeof raw.category_id === 'number' ? raw.category_id : null;
    const subcategoryId = typeof raw.subcategory_id === 'number' ? raw.subcategory_id : null;
    const colorId = typeof raw.color_id === 'string' ? raw.color_id : null;
    const brandId = typeof raw.brand_id === 'string' ? raw.brand_id : null;
    const color = colorId ? colorMap.get(colorId) : undefined;
    return {
      id,
      name: typeof raw.name === 'string' ? raw.name : 'Unknown item',
      image_path: typeof raw.image_path === 'string' ? raw.image_path : null,
      category_id: categoryId,
      subcategory_id: subcategoryId,
      color_id: colorId,
      brand_id: brandId,
      purchase_price: typeof raw.purchase_price === 'number' ? raw.purchase_price : null,
      owned_since: typeof raw.owned_since === 'string' ? raw.owned_since : null,
      laundry_impact: typeof raw.laundry_impact === 'string' ? raw.laundry_impact : null,
      is_active: typeof raw.is_active === 'boolean' ? raw.is_active : true,
      wear_count: typeof raw.wear_count === 'number' ? raw.wear_count : null,
      created_at: typeof raw.created_at === 'string' ? raw.created_at : '',
      category_name: categoryId != null ? (catMap.get(categoryId) ?? null) : null,
      subcategory_name: subcategoryId != null ? (subMap.get(subcategoryId)?.name ?? null) : null,
      color_hex: color?.hex_value ?? null,
      brand_name: brandId ? (brandMap.get(brandId) ?? null) : null,
    };
  };

  const items = rawItems.map(enrich);
  const outfits: EnrichedOutfit[] = rawOutfits.map(o => ({
    id: o.id,
    date: o.date,
    dateKey: new Date(o.date).toISOString().slice(0, 10),
    notes: o.notes,
    items: o.items.map(enrich),
  }));

  return { items, outfits, colors: colorMap, categories: catMap, subcategories: subMap, brands: brandMap };
}

onMounted(async () => {
  try {
    loading.value = true;
    const [itemData, outfitData, colorData, categoryData, subcategoryData, brandData] =
      await Promise.all([
        api.listClothingItems(),
        api.listOutfits(),
        api.listColors(),
        api.listCategories(),
        api.listSubcategories(),
        api.listBrands(),
      ]);
    dataset.value = buildDataset(
      Array.isArray(itemData) ? itemData : [],
      Array.isArray(outfitData) ? outfitData : [],
      Array.isArray(colorData) ? colorData : [],
      Array.isArray(categoryData) ? categoryData : [],
      Array.isArray(subcategoryData) ? subcategoryData : [],
      Array.isArray(brandData) ? brandData : [],
    );
  } catch (err) {
    loadError.value = String(err);
  } finally {
    loading.value = false;
  }
});
</script>

<style scoped>
.insights-view {
  margin-top: 1.5rem;
}

.insights-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  margin-bottom: 1rem;
}

.insights-title {
  margin: 0;
  font-size: var(--md-headline-small-size);
  line-height: var(--md-headline-small-line);
  color: var(--md-on-surface);
}

.insights-actions {
  display: flex;
  gap: 0.5rem;
}

.cap-notice {
  margin-bottom: 1rem;
  padding: 0.5rem 0.9rem;
  font-size: var(--md-body-small-size);
  color: var(--md-on-secondary-container);
  background: var(--md-secondary-container);
  border-radius: var(--md-shape-small);
}

.loading {
  color: var(--md-on-surface-variant);
  padding: 1rem;
  font-style: italic;
}

.error {
  color: var(--md-on-error-container);
  padding: 0.75rem 1rem;
  background: var(--md-error-container);
  border: 1px solid var(--md-error);
  border-radius: var(--md-shape-small);
  margin: 0.75rem 0;
}

.empty {
  color: var(--md-on-surface-variant);
  font-style: italic;
  padding: 2rem 0;
  text-align: center;
}
</style>
