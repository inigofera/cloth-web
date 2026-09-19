<template>
  <InsightChart v-if="hasData" type="bar" :data="chartData" :options="chartOptions" height="200" />
  <div v-else class="widget-empty">No outfits logged yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ChartData, ChartOptions } from 'chart.js';
import InsightChart from '../InsightChart.vue';
import { dayOfWeek, rangeData, widgetRange, type TrendMetric } from '../../../lib/insights/metrics';
import { axisOptions, chartTheme } from '../../../lib/insights/charts';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const metric = computed<TrendMetric>(() => (props.config.metric === 'items' ? 'items' : 'outfits'));

const data = computed(() => rangeData(props.data, widgetRange(props.config)));

const points = computed(() => dayOfWeek(data.value, metric.value));
const hasData = computed(() => points.value.some(p => p.value > 0));

const chartData = computed<ChartData<'bar'>>(() => {
  const t = chartTheme();
  return {
    labels: points.value.map(p => p.label),
    datasets: [
      {
        label: metric.value === 'items' ? 'Unique items' : 'Outfits',
        data: points.value.map(p => p.value),
        backgroundColor: points.value.map(p => (p.value === max.value ? t.primary : `${t.primary}66`)),
        borderRadius: 6,
        maxBarThickness: 36,
      },
    ],
  };
});

const max = computed(() => Math.max(...points.value.map(p => p.value), 0));

const axis = axisOptions();

const chartOptions = computed<ChartOptions<'bar'>>(() => ({
  plugins: { legend: { display: false } },
  scales: {
    x: { ...axis, grid: { display: false } },
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
