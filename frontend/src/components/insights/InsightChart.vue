<template>
  <div class="insight-chart">
    <Chart :type="type" :data="data" :options="mergedOptions" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { Chart } from 'vue-chartjs';
import type { ChartData, ChartOptions } from 'chart.js';
import { baseOptions } from '../../lib/insights/charts';

const props = withDefaults(
  defineProps<{
    type: 'doughnut' | 'bar' | 'line';
    data: ChartData;
    options?: ChartOptions;
    height?: number;
  }>(),
  {
    options: undefined,
    height: 220,
  },
);

function mergeOptions(base: ChartOptions, override?: ChartOptions): ChartOptions {
  if (!override) return base;
  return {
    ...base,
    ...override,
    plugins: {
      ...base.plugins,
      ...override.plugins,
      legend: { ...base.plugins?.legend, ...override.plugins?.legend },
      tooltip: { ...base.plugins?.tooltip, ...override.plugins?.tooltip },
    },
    scales: { ...base.scales, ...override.scales },
  };
}

const mergedOptions = computed<ChartOptions>(() => mergeOptions(baseOptions(), props.options));
</script>

<style scoped>
.insight-chart {
  position: relative;
  width: 100%;
  height: v-bind('height + "px"');
}
</style>
