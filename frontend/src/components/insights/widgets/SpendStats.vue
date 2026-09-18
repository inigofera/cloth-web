<template>
  <div v-if="stats.count > 0" class="spend">
    <div class="spend__item">
      <div class="spend__value">{{ formatPrice(stats.total) }}</div>
      <div class="spend__label">Total</div>
    </div>
    <div class="spend__item">
      <div class="spend__value">{{ stats.avg == null ? '—' : formatPrice(stats.avg) }}</div>
      <div class="spend__label">Average</div>
    </div>
    <div class="spend__item">
      <div class="spend__value">{{ stats.median == null ? '—' : formatPrice(stats.median) }}</div>
      <div class="spend__label">Median</div>
    </div>
    <div class="spend__footnote">{{ stats.count }} item{{ stats.count === 1 ? '' : 's' }} with a price</div>
  </div>
  <div v-else class="widget-empty">No prices set yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { spendStats } from '../../../lib/insights/metrics';
import { formatPrice } from '../../../lib/insights/format';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const stats = computed(() => spendStats(props.data));
</script>

<style scoped>
.spend {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  height: 100%;
  justify-content: center;
}

.spend__item {
  display: flex;
  align-items: baseline;
  gap: 0.6rem;
}

.spend__value {
  font-size: var(--md-title-large-size);
  line-height: var(--md-title-large-line);
  color: var(--md-on-surface);
  font-variant-numeric: tabular-nums;
}

.spend__label {
  font-size: var(--md-label-large-size);
  color: var(--md-on-surface-variant);
}

.spend__footnote {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface-variant);
}

.widget-empty {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-small-size);
  font-style: italic;
  padding: 1rem 0;
  text-align: center;
}
</style>
