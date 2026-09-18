<template>
  <div class="heatmap">
    <div class="heatmap__grid">
      <div class="heatmap__corner"></div>
      <div v-for="d in weekdayLabels" :key="d" class="heatmap__col-label">
        {{ d }}
      </div>

      <template v-for="(row, mi) in values" :key="mi">
        <div class="heatmap__row-label">{{ monthLabels[mi] }}</div>
        <div
          v-for="(v, di) in row"
          :key="di"
          class="heatmap__cell"
          :style="{ opacity: max > 0 ? 0.12 + 0.88 * (v / max) : 0 }"
          :title="`${monthLabels[mi]} ${weekdayLabels[di]}: ${v} outfit${v === 1 ? '' : 's'}`"
        ></div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  values: number[][];
  max: number;
}>();

const monthLabels = Array.from({ length: 12 }, (_, i) =>
  new Date(Date.UTC(2024, i, 1)).toLocaleDateString(undefined, { month: 'short', timeZone: 'UTC' }),
);

// 2024-01-01 is a Monday
const weekdayLabels = Array.from({ length: 7 }, (_, i) =>
  new Date(Date.UTC(2024, 0, 1 + i)).toLocaleDateString(undefined, { weekday: 'narrow', timeZone: 'UTC' }),
);
</script>

<style scoped>
.heatmap {
  width: 100%;
}

.heatmap__grid {
  display: grid;
  grid-template-columns: auto repeat(7, 1fr);
  gap: 3px;
  align-items: center;
}

.heatmap__corner {
  width: 28px;
}

.heatmap__col-label,
.heatmap__row-label {
  font-size: var(--md-label-small-size);
  color: var(--md-on-surface-variant);
  text-align: center;
}

.heatmap__row-label {
  text-align: right;
  padding-right: 4px;
  width: 28px;
}

.heatmap__cell {
  aspect-ratio: 1;
  border-radius: var(--md-shape-extra-small);
  background: var(--md-primary);
  min-height: 12px;
}
</style>
