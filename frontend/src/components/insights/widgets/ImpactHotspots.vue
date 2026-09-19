<template>
  <div v-if="rows.length" class="impact-list">
    <div v-for="r in rows" :key="r.id" class="impact-row">
      <img v-if="thumb(r.image_path)" :src="thumb(r.image_path)!" class="impact-thumb" alt="" />
      <div v-else class="impact-thumb impact-thumb--empty"></div>
      <div class="impact-main">
        <span class="impact-name" :title="r.name">{{ r.name }}</span>
        <span class="impact-badge">{{ r.impact }}</span>
      </div>
      <span class="impact-wears">{{ r.wears }}× worn</span>
    </div>
  </div>
  <div v-else class="widget-empty">No {{ includeMedium ? 'high or medium' : 'high' }}-impact items in your wardrobe.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { impactHotspots } from '../../../lib/insights/metrics';
import { imageUrl } from '../../../api/client';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const n = computed(() => {
  const v = Number(props.config.topN);
  return Number.isFinite(v) && v > 0 ? Math.min(15, Math.round(v)) : 5;
});

const includeMedium = computed(() => props.config.includeMedium === true);

const minWears = computed(() => {
  const v = Number(props.config.minWears);
  return Number.isFinite(v) && v > 0 ? Math.min(20, Math.round(v)) : 0;
});

const rows = computed(() =>
  impactHotspots(props.data, n.value, {
    includeMedium: includeMedium.value,
    minWears: minWears.value,
  }),
);

function thumb(path: string | null): string | null {
  return imageUrl(path, { width: 56 });
}
</script>

<style scoped>
.impact-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.impact-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.impact-thumb {
  width: 32px;
  height: 32px;
  object-fit: cover;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.impact-thumb--empty {
  background: var(--md-surface-container-highest);
}

.impact-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.impact-name {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.impact-badge {
  font-size: var(--md-label-small-size);
  font-weight: var(--md-label-small-weight);
  color: var(--md-on-error-container);
  background: var(--md-error-container);
  border-radius: var(--md-shape-full);
  padding: 1px 8px;
  flex-shrink: 0;
}

.impact-wears {
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
