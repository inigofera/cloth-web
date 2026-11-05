<template>
  <div class="insights-view">
    <div v-if="loading" class="loading">Loading insights…</div>
    <div v-else-if="loadError" class="error">{{ loadError }}</div>

    <template v-else>
      <div class="insights-header">
        <h2 class="insights-title">Insights</h2>
        <div class="insights-actions">
          <template v-if="confirmingReset">
            <span class="reset-confirm-prompt">Reset widgets to default?</span>
            <MdButton variant="text" @click="confirmResetLayout">Yes, reset</MdButton>
            <MdButton variant="text" @click="confirmingReset = false">Cancel</MdButton>
          </template>
          <template v-else>
            <MdButton variant="text" @click="confirmingReset = true">Reset layout</MdButton>
            <MdButton variant="tonal" @click="paletteOpen = true">Add widget</MdButton>
          </template>
        </div>
      </div>

      <FilterBar
        :filters="filters"
        :categories="filterOptions.categories"
        :colors="filterOptions.colors"
        :brands="filterOptions.brands"
        @update="filters = $event"
      />

      <!--
        Data-completeness notice (UX_ANALYSIS.md P1-A1 / P1-B3): driven by the
        server-reported X-Total-Count, so it only appears when a snapshot is
        genuinely incomplete and always states real numbers.
      -->
      <div v-if="truncationNotice" class="cap-notice">{{ truncationNotice }}</div>

      <!-- Polite live region: announces layout changes (widget moves) to
           screen readers and other assistive technology. -->
      <div aria-live="polite" role="status" class="sr-only">{{ layoutAnnouncement }}</div>

      <WidgetGrid
        v-if="widgets.length > 0"
        :widgets="widgets"
        :data="filteredData"
        @reorder="reorderWidgets"
        @remove="removeWidget"
        @move="(id: string, dir: -1 | 1) => moveWidget(id, dir)"
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
import { computed, nextTick, onActivated, onMounted, ref, watch } from 'vue';
import { MdButton } from '../ui';
import { formatTruncationNotice, useWardrobe, WARDROBE_STALE_MS } from '../lib/wardrobe';
import FilterBar from './insights/FilterBar.vue';
import WidgetGrid from './insights/WidgetGrid.vue';
import WidgetPalette from './insights/WidgetPalette.vue';
import { WIDGET_CATALOG } from '../lib/insights/catalog';
import { applyFilters, withDerived } from '../lib/insights/metrics';
import { createInstance, defaultLayout, loadLayout, saveLayout } from '../lib/insights/store';
import { parseOutfitDateKey } from '../lib/dates';
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

const wardrobe = useWardrobe();
const { loading, error: loadError } = wardrobe;

// Rebuild the denormalized dataset whenever the shared wardrobe changes. This
// keeps Insights in sync with items/outfits added or deleted in other tabs.
const dataset = computed<InsightDataset | null>(() => {
  if (!wardrobe.loaded.value) return null;
  return buildDataset(
    wardrobe.items.value,
    wardrobe.outfits.value,
    wardrobe.colors.value,
    wardrobe.categories.value,
    wardrobe.subcategories.value,
    wardrobe.brands.value,
  );
});

const filters = ref<GlobalFilters>({ ...DEFAULT_FILTERS });
const paletteOpen = ref(false);
const widgets = ref<WidgetInstance[]>(loadLayout(WIDGET_CATALOG));

watch(widgets, () => saveLayout(widgets.value), { deep: true });

// One derived pass per filtered dataset: the wear and last-worn maps plus the
// four range views ride along on this object, so the ~20 widgets read shared
// views instead of each rebuilding (and re-copying) the same data.
const filteredData = computed<FilteredData>(() =>
  dataset.value ? withDerived(applyFilters(dataset.value, filters.value)) : { items: [], outfits: [] },
);

// Derived from the server-reported totals (X-Total-Count). Replaces the old
// hard-coded "capped at 100" banner, which stated a number the API never had.
const truncationNotice = computed(() =>
  formatTruncationNotice(
    wardrobe.items.value.length,
    wardrobe.itemTotal.value,
    wardrobe.outfits.value.length,
    wardrobe.outfitTotal.value
  ),
);

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

// Screen-reader feedback for layout changes. Clearing the message first
// guarantees that repeating the same announcement is read again.
const layoutAnnouncement = ref('');

function announceLayout(msg: string): void {
  layoutAnnouncement.value = '';
  void nextTick().then(() => {
    layoutAnnouncement.value = msg;
  });
}

function widgetTitle(type: string, fallback: string): string {
  return WIDGET_CATALOG.get(type)?.title ?? fallback;
}

function moveWidget(id: string, direction: -1 | 1): void {
  const idx = widgets.value.findIndex(w => w.id === id);
  if (idx === -1) return;
  const target = idx + direction;
  if (target < 0 || target >= widgets.value.length) return; // already at the edge
  reorderWidgets(idx, target);
}

function reorderWidgets(from: number, to: number) {
  const arr = [...widgets.value];
  const [moved] = arr.splice(from, 1);
  arr.splice(to, 0, moved);
  widgets.value = arr;
  if (moved) {
    announceLayout(
      `${widgetTitle(moved.type, 'Widget')} moved from position ${from + 1} to position ${to + 1} of ${arr.length}.`,
    );
  }
}

const confirmingReset = ref(false);

function confirmResetLayout() {
  widgets.value = defaultLayout(WIDGET_CATALOG);
  confirmingReset.value = false;
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

  // Items are nodes, outfits are edges: the wire carries only ids, so each item
  // is enriched exactly once and the SAME object is shared by every outfit that
  // wears it. A garment seen in fifty outfits used to cost fifty copies of a
  // fifteen-field record; now it costs one record plus fifty references.
  const enrichedById = new Map<string, EnrichedItem>(items.map(item => [item.id, item]));

  const joinOutfitItems = (itemIds: string[] | undefined): EnrichedItem[] => {
    if (!itemIds || itemIds.length === 0) return [];
    const joined: EnrichedItem[] = [];
    for (const id of itemIds) {
      const item = enrichedById.get(id);
      // An id whose item row never arrived is skipped: metrics would otherwise
      // carry a phantom garment through every aggregation.
      if (item) joined.push(item);
    }
    return joined;
  };

  const outfits: EnrichedOutfit[] = rawOutfits.map(o => ({
    id: o.id,
    date: o.date,
    dateKey: parseOutfitDateKey(o.date),
    notes: o.notes,
    items: joinOutfitItems(o.item_ids),
  }));

  return { items, outfits, colors: colorMap, categories: catMap, subcategories: subMap, brands: brandMap };
}

onMounted(() => {
  void wardrobe.load();
});

onActivated(() => {
  void wardrobe.load({ maxAgeMs: WARDROBE_STALE_MS });
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

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
</style>
