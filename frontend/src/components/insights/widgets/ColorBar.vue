<template>
  <div v-if="slices.length" class="color-list">
    <div v-for="s in slices.slice(0, 10)" :key="s.label" class="color-row">
      <span class="color-swatch" :style="{ background: s.hex || 'var(--md-surface-container-highest)' }"></span>
      <span class="color-name">{{ s.label }}</span>
      <div class="color-bar">
        <MiniBar :value="s.value" :max="slices[0].value" :color="s.hex || 'var(--md-primary)'"></MiniBar>
      </div>
      <span class="color-count">{{ s.value }}</span>
    </div>
    <div v-if="slices.length > 10" class="color-more">+{{ slices.length - 10 }} more</div>
  </div>
  <div v-else class="widget-empty">No items yet.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import MiniBar from '../MiniBar.vue';
import { colorBreakdown } from '../../../lib/insights/metrics';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const slices = computed(() => colorBreakdown(props.data));
</script>

<style scoped>
.color-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.color-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.color-swatch {
  width: 14px;
  height: 14px;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.color-name {
  width: 90px;
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  text-transform: capitalize;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 0;
}

.color-bar {
  flex: 1;
  min-width: 0;
}

.color-count {
  font-size: var(--md-label-medium-size);
  color: var(--md-on-surface-variant);
  font-variant-numeric: tabular-nums;
  width: 24px;
  text-align: right;
  flex-shrink: 0;
}

.color-more {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface-variant);
  padding-left: 1.6rem;
}

.widget-empty {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-small-size);
  font-style: italic;
  padding: 1rem 0;
  text-align: center;
}
</style>
