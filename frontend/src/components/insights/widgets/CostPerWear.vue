<template>
  <div v-if="rows.length" class="cpw-list">
    <div v-for="r in rows" :key="r.id" class="cpw-row">
      <img v-if="thumb(r.image_path)" :src="thumb(r.image_path)!" class="cpw-thumb" alt="" />
      <div v-else class="cpw-thumb cpw-thumb--empty"></div>
      <div class="cpw-main">
        <span class="cpw-name" :title="r.name">{{ r.name }}</span>
        <span class="cpw-sub">{{ formatPrice(r.price) }} · {{ r.wears }} wears</span>
      </div>
      <div class="cpw-bar">
        <MiniBar :value="r.cpw" :max="rows[0].cpw" :color="barColor" />
      </div>
      <span class="cpw-value">{{ formatPrice(r.cpw) }}</span>
    </div>
  </div>
  <div v-else class="widget-empty">Need prices and logged wears to compute this.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import MiniBar from '../MiniBar.vue';
import { costPerWear } from '../../../lib/insights/metrics';
import { formatPrice } from '../../../lib/insights/format';
import { imageUrl } from '../../../api/client';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const direction = computed<'best' | 'worst'>(() =>
  props.config.direction === 'worst' ? 'worst' : 'best',
);

const n = computed(() => {
  const v = Number(props.config.topN);
  return Number.isFinite(v) && v > 0 ? Math.min(15, Math.round(v)) : 5;
});

const rows = computed(() => costPerWear(props.data, direction.value, n.value));

const barColor = computed(() =>
  direction.value === 'best' ? 'var(--md-primary)' : 'var(--md-error)',
);

function thumb(path: string | null): string | null {
  return imageUrl(path, { width: 56 });
}
</script>

<style scoped>
.cpw-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.cpw-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.cpw-thumb {
  width: 32px;
  height: 32px;
  object-fit: cover;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.cpw-thumb--empty {
  background: var(--md-surface-container-highest);
}

.cpw-main {
  width: 130px;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.cpw-name {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cpw-sub {
  font-size: var(--md-label-small-size);
  color: var(--md-on-surface-variant);
}

.cpw-bar {
  flex: 1;
  min-width: 0;
}

.cpw-value {
  font-size: var(--md-label-large-size);
  font-weight: var(--md-label-large-weight);
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
