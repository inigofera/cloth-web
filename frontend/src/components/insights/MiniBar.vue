<template>
  <div class="mini-bar">
    <div class="mini-bar__fill" :style="{ width: pct + '%', background: color }"></div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    value: number;
    max: number;
    color?: string;
  }>(),
  {
    color: 'var(--md-primary)',
  },
);

const pct = computed(() => (props.max > 0 ? Math.min(100, (props.value / props.max) * 100) : 0));
</script>

<style scoped>
.mini-bar {
  height: 6px;
  border-radius: var(--md-shape-full);
  background: var(--md-surface-container-highest);
  overflow: hidden;
}

.mini-bar__fill {
  height: 100%;
  border-radius: var(--md-shape-full);
  transition: width var(--md-motion-duration-medium) var(--md-motion-easing-standard);
}
</style>
