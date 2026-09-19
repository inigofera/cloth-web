<template>
  <HeatmapGrid v-if="hasData" :values="pattern.values" :max="pattern.max" />
  <div v-else class="widget-empty">No outfits logged yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import HeatmapGrid from '../HeatmapGrid.vue';
import { rangeData, seasonPattern, widgetRange, type TrendMetric } from '../../../lib/insights/metrics';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const metric = computed<TrendMetric>(() => (props.config.metric === 'items' ? 'items' : 'outfits'));

const data = computed(() => rangeData(props.data, widgetRange(props.config)));

const pattern = computed(() => seasonPattern(data.value, metric.value));
const hasData = computed(() => pattern.value.max > 0);
</script>

<style scoped>
.widget-empty {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-small-size);
  font-style: italic;
  padding: 1rem 0;
  text-align: center;
}
</style>
