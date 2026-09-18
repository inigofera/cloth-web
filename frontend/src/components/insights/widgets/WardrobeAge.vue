<template>
  <InsightChart v-if="rows.length" type="bar" :data="chartData" :options="chartOptions" height="200" />
  <div v-else class="widget-empty">No items yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ChartData, ChartOptions } from 'chart.js';
import InsightChart from '../InsightChart.vue';
import { wardrobeAge } from '../../../lib/insights/metrics';
import { axisOptions, chartTheme } from '../../../lib/insights/charts';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const rows = computed(() => wardrobeAge(props.data));

const chartData = computed<ChartData<'bar'>>(() => {
  const t = chartTheme();
  return {
    labels: rows.value.map(r => r.label),
    datasets: [
      {
        label: 'Items',
        data: rows.value.map(r => r.value),
        backgroundColor: t.tertiary,
        borderRadius: 6,
      },
    ],
  };
});

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
