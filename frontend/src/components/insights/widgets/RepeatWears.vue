<template>
  <div v-if="rows.length" class="repeat-list">
    <div v-for="r in rows.slice(0, 8)" :key="r.id" class="repeat-row">
      <img v-if="thumb(r.image_path)" :src="thumb(r.image_path)!" class="repeat-thumb" alt="" />
      <div v-else class="repeat-thumb repeat-thumb--empty"></div>
      <div class="repeat-main">
        <span class="repeat-name" :title="r.name">{{ r.name }}</span>
        <MiniBar :value="r.repeats" :max="rows[0].repeats" color="var(--md-tertiary)" />
      </div>
      <span class="repeat-count">{{ r.repeats }}×</span>
    </div>
  </div>
  <div v-else class="widget-empty">No quick re-wears in this period.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import MiniBar from '../MiniBar.vue';
import { repeatWears } from '../../../lib/insights/metrics';
import { imageUrl } from '../../../api/client';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const windowDays = computed(() => {
  const v = Number(props.config.windowDays);
  return Number.isFinite(v) && v > 0 ? Math.min(30, Math.round(v)) : 7;
});

const rows = computed(() => repeatWears(props.data, windowDays.value));

function thumb(path: string | null): string | null {
  return imageUrl(path, { width: 56 });
}
</script>

<style scoped>
.repeat-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.repeat-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.repeat-thumb {
  width: 32px;
  height: 32px;
  object-fit: cover;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.repeat-thumb--empty {
  background: var(--md-surface-container-highest);
}

.repeat-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.repeat-name {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.repeat-count {
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
