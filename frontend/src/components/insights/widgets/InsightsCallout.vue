<template>
  <ul class="callout">
    <li v-for="(line, i) in lines" :key="i" class="callout__item">
      <span class="callout__dot"></span>
      <span class="callout__text">{{ line }}</span>
    </li>
  </ul>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { textInsights, type InsightCategory } from '../../../lib/insights/metrics';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const focus = computed<InsightCategory | 'all'>(() => {
  const v = props.config.focus;
  return v === 'usage' || v === 'value' || v === 'sustainability' ? v : 'all';
});

const maxInsights = computed(() => {
  const v = Number(props.config.maxInsights);
  return Number.isFinite(v) && v > 0 ? Math.min(12, Math.round(v)) : 6;
});

const lines = computed(() =>
  textInsights(props.data)
    .filter(i => focus.value === 'all' || i.category === focus.value)
    .slice(0, maxInsights.value)
    .map(i => i.text),
);
</script>

<style scoped>
.callout {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}

.callout__item {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
}

.callout__dot {
  width: 8px;
  height: 8px;
  border-radius: var(--md-shape-full);
  background: var(--md-primary);
  flex-shrink: 0;
  margin-top: 6px;
}

.callout__text {
  font-size: var(--md-body-medium-size);
  line-height: var(--md-body-medium-line);
  color: var(--md-on-surface);
}
</style>
