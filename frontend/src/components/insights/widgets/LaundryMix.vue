<template>
  <InsightChart v-if="slices.length" type="doughnut" :data="chartData" :options="chartOptions" height="220" />
  <div v-else class="widget-empty">No items yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ChartData, ChartOptions } from 'chart.js';
import InsightChart from '../InsightChart.vue';
import { laundryMix, type BreakdownMetric } from '../../../lib/insights/metrics';
import { paletteColor } from '../../../lib/insights/charts';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const metric = computed<BreakdownMetric>(() =>
  props.config.metric === 'spend' ? 'spend' : 'items',
);

const includeNotSet = computed(() => props.config.includeNotSet !== false);

const slices = computed(() =>
  laundryMix(props.data, { metric: metric.value, includeNotSet: includeNotSet.value }),
);

const IMPACT_COLORS: Record<string, string> = {
  Low: '#6750a4',
  Medium: '#7c5872',
  High: '#b3261e',
};

const chartData = computed<ChartData<'doughnut'>>(() => ({
  labels: slices.value.map(s => s.label),
  datasets: [
    {
      data: slices.value.map(s => s.value),
      backgroundColor: slices.value.map((s, i) => IMPACT_COLORS[s.label] ?? paletteColor(i + 2)),
      borderColor: 'transparent',
      borderWidth: 0,
      hoverOffset: 6,
    },
  ],
}));

const chartOptions = computed<ChartOptions<'doughnut'>>(() => ({
  cutout: '62%',
  plugins: {
    legend: {
      position: 'right',
      labels: { padding: 10 },
    },
  },
}));
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
