<template>
  <div v-if="rows.length" class="pair-list">
    <div v-for="p in rows" :key="`${p.aId}-${p.bId}`" class="pair-row">
      <div class="pair-names">
        <span class="pair-name" :title="p.aName">{{ p.aName }}</span>
        <span class="pair-plus">+</span>
        <span class="pair-name" :title="p.bName">{{ p.bName }}</span>
      </div>
      <div class="pair-bar">
        <MiniBar :value="p.count" :max="rows[0].count" color="var(--md-secondary)" />
      </div>
      <span class="pair-count">{{ p.count }}×</span>
    </div>
  </div>
  <div v-else class="widget-empty">
    {{ focusItem ? `No pairings for ${focusItem.name} yet.` : 'Log outfits with two or more items to see pairings.' }}
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import MiniBar from '../MiniBar.vue';
import { itemPairings, rangeData, resolveItem, widgetRange } from '../../../lib/insights/metrics';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const data = computed(() => rangeData(props.data, widgetRange(props.config)));

const n = computed(() => {
  const v = Number(props.config.topN);
  return Number.isFinite(v) && v > 0 ? Math.min(15, Math.round(v)) : 5;
});

const focusItem = computed(() => resolveItem(data.value, props.config, 'focusItem'));

const minCount = computed(() => {
  const v = Number(props.config.minCount);
  return Number.isFinite(v) && v > 0 ? Math.min(20, Math.round(v)) : 1;
});

const rows = computed(() =>
  itemPairings(data.value, n.value, {
    focusItemId: focusItem.value?.id ?? null,
    minCount: minCount.value,
  }),
);
</script>

<style scoped>
.pair-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.pair-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.pair-names {
  width: 150px;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
}

.pair-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pair-plus {
  color: var(--md-on-surface-variant);
  flex-shrink: 0;
}

.pair-bar {
  flex: 1;
  min-width: 0;
}

.pair-count {
  font-size: var(--md-label-medium-size);
  color: var(--md-on-surface-variant);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.widget-empty {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-small-size);
  font-style: italic;
  padding: 1rem 0;
  text-align: center;
}
</style>
