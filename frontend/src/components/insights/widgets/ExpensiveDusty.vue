<template>
  <div v-if="rows.length" class="ed-list">
    <div v-for="r in rows" :key="r.id" class="ed-row">
      <img v-if="thumb(r.image_path)" :src="thumb(r.image_path)!" class="ed-thumb" alt="" />
      <div v-else class="ed-thumb ed-thumb--empty"></div>
      <div class="ed-main">
        <span class="ed-name" :title="r.name">{{ r.name }}</span>
        <span class="ed-sub">{{ r.wears }} wear{{ r.wears === 1 ? '' : 's' }}</span>
      </div>
      <span class="ed-price">{{ formatPrice(r.price) }}</span>
    </div>
  </div>
  <div v-else class="widget-empty">No expensive items left unworn.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { expensiveDusty, type ExpensiveDustySort } from '../../../lib/insights/metrics';
import { formatPrice } from '../../../lib/insights/format';
import { imageUrl } from '../../../api/client';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const minPrice = computed(() => {
  const v = Number(props.config.minPrice);
  return Number.isFinite(v) && v >= 0 ? v : 50;
});

const maxWears = computed(() => {
  const v = Number(props.config.maxWears);
  return Number.isFinite(v) && v >= 0 ? Math.min(20, Math.round(v)) : 0;
});

const sortBy = computed<ExpensiveDustySort>(() => {
  const v = props.config.sortBy;
  return v === 'wears' || v === 'cpw' ? v : 'price';
});

const topN = computed(() => {
  const v = Number(props.config.topN);
  return Number.isFinite(v) && v > 0 ? Math.min(20, Math.round(v)) : 8;
});

const rows = computed(() =>
  expensiveDusty(props.data, minPrice.value, maxWears.value, {
    sortBy: sortBy.value,
    topN: topN.value,
  }),
);

function thumb(path: string | null): string | null {
  return imageUrl(path, { width: 56 });
}
</script>

<style scoped>
.ed-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.ed-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.ed-thumb {
  width: 32px;
  height: 32px;
  object-fit: cover;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.ed-thumb--empty {
  background: var(--md-surface-container-highest);
}

.ed-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.ed-name {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ed-sub {
  font-size: var(--md-label-small-size);
  color: var(--md-on-surface-variant);
}

.ed-price {
  font-size: var(--md-label-large-size);
  font-weight: var(--md-label-large-weight);
  color: var(--md-error);
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
