<template>
  <InsightChart v-if="trend.length" type="line" :data="chartData" :options="chartOptions" height="240" />
  <div v-else class="widget-empty">No outfits in this period.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ChartData, ChartOptions } from 'chart.js';
import InsightChart from '../InsightChart.vue';
import { wearTrend } from '../../../lib/insights/metrics';
import { axisOptions, chartTheme } from '../../../lib/insights/charts';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const granularity = computed<'week' | 'month'>(() =>
  props.config.granularity === 'month' ? 'month' : 'week',
);

const trend = computed(() => wearTrend(props.data, granularity.value));

const chartData = computed<ChartData<'line'>>(() => {
  const t = chartTheme();
  return {
    labels: trend.value.map(p => p.label),
    datasets: [
      {
        label: 'Outfits',
        data: trend.value.map(p => p.value),
        borderColor: t.primary,
        backgroundColor: `${t.primary}26`,
        fill: true,
        tension: 0.35,
        borderWidth: 2,
        pointRadius: trend.value.length > 40 ? 0 : 2,
        pointBackgroundColor: t.primary,
      },
    ],
  };
});

const axis = axisOptions();

const chartOptions = computed<ChartOptions<'line'>>(() => ({
  plugins: { legend: { display: false } },
  scales: {
    x: { ...axis, grid: { display: false }, ticks: { ...axis.ticks, maxTicksLimit: 10 } },
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
