<template>
  <div v-if="rows.length" class="dusty-list">
    <div v-for="r in rows.slice(0, 8)" :key="r.id" class="dusty-row">
      <img v-if="thumb(r.image_path)" :src="thumb(r.image_path)!" class="dusty-thumb" alt="" />
      <div v-else class="dusty-thumb dusty-thumb--empty"></div>
      <div class="dusty-main">
        <span class="dusty-name" :title="r.name">{{ r.name }}</span>
        <MiniBar :value="r.daysSince ?? 0" :max="barMax" color="var(--md-tertiary)" />
      </div>
      <span class="dusty-days" :class="{ 'dusty-days--never': r.daysSince == null }">
        {{ r.daysSince == null ? 'Never worn' : `${r.daysSince}d` }}
      </span>
    </div>
    <div v-if="rows.length > 8" class="dusty-more">+{{ rows.length - 8 }} more</div>
  </div>
  <div v-else class="widget-empty">Nothing is gathering dust.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import MiniBar from '../MiniBar.vue';
import { dustyItems } from '../../../lib/insights/metrics';
import { imageUrl } from '../../../api/client';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const threshold = computed(() => {
  const v = Number(props.config.thresholdDays);
  return Number.isFinite(v) && v > 0 ? Math.min(365, Math.round(v)) : 30;
});

const rows = computed(() => dustyItems(props.data, threshold.value));

const barMax = computed(() => Math.max(90, threshold.value * 2));

function thumb(path: string | null): string | null {
  return imageUrl(path, { width: 56 });
}
</script>

<style scoped>
.dusty-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.dusty-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.dusty-thumb {
  width: 32px;
  height: 32px;
  object-fit: cover;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.dusty-thumb--empty {
  background: var(--md-surface-container-highest);
}

.dusty-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.dusty-name {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dusty-days {
  font-size: var(--md-label-medium-size);
  color: var(--md-on-surface-variant);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.dusty-days--never {
  color: var(--md-error);
  font-weight: var(--md-label-medium-weight);
}

.dusty-more {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface-variant);
  padding-left: 2.6rem;
}

.widget-empty {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-small-size);
  font-style: italic;
  padding: 1rem 0;
  text-align: center;
}
</style>
