<template>
  <InsightChart v-if="buckets.length" type="bar" :data="chartData" :options="chartOptions" height="200" />
  <div v-else class="widget-empty">No prices set yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ChartData, ChartOptions } from 'chart.js';
import InsightChart from '../InsightChart.vue';
import { priceHistogram } from '../../../lib/insights/metrics';
import { axisOptions, chartTheme } from '../../../lib/insights/charts';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const bucketCount = computed(() => {
  const v = Number(props.config.buckets);
  return Number.isFinite(v) && v > 0 ? Math.min(12, Math.round(v)) : 8;
});

const buckets = computed(() => priceHistogram(props.data, bucketCount.value));

const chartData = computed<ChartData<'bar'>>(() => {
  const t = chartTheme();
  return {
    labels: buckets.value.map(b => b.label),
    datasets: [
      {
        label: 'Items',
        data: buckets.value.map(b => b.value),
        backgroundColor: t.secondary,
        borderRadius: 6,
        borderSkipped: false,
      },
    ],
  };
});

const axis = axisOptions();

const chartOptions = computed<ChartOptions<'bar'>>(() => ({
  plugins: { legend: { display: false } },
  scales: {
    x: { ...axis, grid: { display: false }, ticks: { ...axis.ticks, maxRotation: 45, minRotation: 0 } },
    y: { ...axis, beginAtZero: true, ticks: { ...axis.ticks, precision: 0 } },
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
