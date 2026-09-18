<template>
  <div class="stat">
    <div class="stat__value">{{ display }}</div>
    <div class="stat__label">{{ METRIC_LABELS[metric] }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { computeStat, type StatMetric } from '../../../lib/insights/metrics';
import { formatNumber, formatNumber1, formatPrice } from '../../../lib/insights/format';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const METRIC_LABELS: Record<StatMetric, string> = {
  total_items: 'Items',
  active_items: 'Active items',
  total_outfits: 'Outfits logged',
  total_spend: 'Total spend',
  avg_price: 'Average price',
  cost_per_wear: 'Cost per wear',
  items_per_outfit: 'Items per outfit',
  current_streak: 'Day streak',
};

const metric = computed<StatMetric>(() =>
  typeof props.config.metric === 'string' ? (props.config.metric as StatMetric) : 'total_items',
);

const stat = computed(() => computeStat(props.data, metric.value));

const display = computed(() => {
  if (stat.value.value == null) return '—';
  if (stat.value.format === 'price') return formatPrice(stat.value.value);
  if (stat.value.format === 'decimal') return formatNumber1(stat.value.value);
  return formatNumber(stat.value.value);
});
</script>

<style scoped>
.stat {
  display: flex;
  flex-direction: column;
  gap: 4px;
  height: 100%;
  justify-content: center;
  padding: 0.5rem 0;
}

.stat__value {
  font-size: var(--md-headline-small-size);
  line-height: var(--md-headline-small-line);
  font-weight: var(--md-headline-small-weight);
  color: var(--md-on-surface);
  font-variant-numeric: tabular-nums;
}

.stat__label {
  font-size: var(--md-label-large-size);
  color: var(--md-on-surface-variant);
}
</style>
