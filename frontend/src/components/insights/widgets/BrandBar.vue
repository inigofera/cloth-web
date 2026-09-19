<template>
  <InsightChart v-if="rows.length" type="bar" :data="chartData" :options="chartOptions" :height="Math.max(160, rows.length * 34)" />
  <div v-else class="widget-empty">No items yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ChartData, ChartOptions } from 'chart.js';
import InsightChart from '../InsightChart.vue';
import { brandBreakdown, type BreakdownMetric } from '../../../lib/insights/metrics';
import { axisOptions, chartTheme } from '../../../lib/insights/charts';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const n = computed(() => {
  const v = Number(props.config.topN);
  return Number.isFinite(v) && v > 0 ? Math.min(20, Math.round(v)) : 8;
});

const metric = computed<BreakdownMetric>(() => {
  const v = props.config.metric;
  return v === 'spend' || v === 'avg' ? v : 'items';
});

const includeNoBrand = computed(() => props.config.includeNoBrand !== false);

const rows = computed(() =>
  brandBreakdown(props.data, n.value, { metric: metric.value, includeNoBrand: includeNoBrand.value }),
);

const METRIC_LABELS: Record<BreakdownMetric, string> = {
  items: 'Items',
  spend: 'Spend',
  avg: 'Avg price',
};

const chartData = computed<ChartData<'bar'>>(() => {
  const t = chartTheme();
  return {
    labels: rows.value.map(r => r.label),
    datasets: [
      {
        label: METRIC_LABELS[metric.value],
        data: rows.value.map(r => r.value),
        backgroundColor: t.primary,
        borderRadius: 6,
        maxBarThickness: 22,
      },
    ],
  };
});

const axis = axisOptions();

const chartOptions = computed<ChartOptions<'bar'>>(() => ({
  indexAxis: 'y',
  plugins: { legend: { display: false } },
  scales: {
    x: { ...axis, beginAtZero: true, ticks: { ...axis.ticks, precision: metric.value === 'avg' ? 1 : 0 } },
    y: { ...axis, grid: { display: false } },
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
