<template>
  <div v-if="rows.length" class="rank-list">
    <div v-for="r in rows" :key="r.id" class="rank-row">
      <img v-if="thumb(r.image_path)" :src="thumb(r.image_path)!" class="rank-thumb" alt="" />
      <div v-else class="rank-thumb rank-thumb--empty"></div>
      <div class="rank-main">
        <span class="rank-name" :title="r.name">{{ r.name }}</span>
        <MiniBar :value="r.count" :max="rows[0].count" />
      </div>
      <span class="rank-count">{{ r.count }}×</span>
    </div>
  </div>
  <div v-else class="widget-empty">No wear data yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import MiniBar from '../MiniBar.vue';
import { topWorn } from '../../../lib/insights/metrics';
import { imageUrl } from '../../../api/client';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const n = computed(() => {
  const v = Number(props.config.topN);
  return Number.isFinite(v) && v > 0 ? Math.min(20, Math.round(v)) : 5;
});

const source = computed<'outfits' | 'wear_count'>(() =>
  props.config.source === 'wear_count' ? 'wear_count' : 'outfits',
);

const rows = computed(() => topWorn(props.data, n.value, source.value));

function thumb(path: string | null): string | null {
  return imageUrl(path, { width: 56 });
}
</script>

<style scoped>
.rank-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.rank-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.rank-thumb {
  width: 32px;
  height: 32px;
  object-fit: cover;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.rank-thumb--empty {
  background: var(--md-surface-container-highest);
}

.rank-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.rank-name {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rank-count {
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
