<template>
  <!-- Focus mode: a single item with wardrobe context -->
  <div v-if="detail" class="cpw-detail">
    <div class="cpw-detail__head">
      <img v-if="thumb(detail.image_path)" :src="thumb(detail.image_path)!" class="cpw-thumb" alt="" />
      <div v-else class="cpw-thumb cpw-thumb--empty"></div>
      <div class="cpw-detail__name">
        <span class="cpw-name" :title="detail.name">{{ detail.name }}</span>
        <span class="cpw-sub">{{ formatPrice(detail.price) }} · {{ detail.wears }} wear{{ detail.wears === 1 ? '' : 's' }}</span>
      </div>
    </div>
    <div class="cpw-detail__stats">
      <div class="cpw-detail__stat">
        <div class="cpw-detail__value">{{ detail.cpw == null ? '—' : formatPrice(detail.cpw) }}</div>
        <div class="cpw-detail__label">Cost per wear</div>
      </div>
      <div v-if="detail.rank != null" class="cpw-detail__stat">
        <div class="cpw-detail__value">#{{ detail.rank }}</div>
        <div class="cpw-detail__label">of {{ detail.total }} ranked</div>
      </div>
    </div>
    <div
      v-if="detail.cpw != null && detail.average != null"
      class="cpw-detail__compare"
      :class="compareGood ? 'cpw-detail__compare--good' : 'cpw-detail__compare--bad'"
    >
      {{ compareText }}
    </div>
  </div>

  <!-- Leaderboard mode -->
  <div v-else-if="rows.length" class="cpw-list">
    <div v-for="r in rows" :key="r.id" class="cpw-row">
      <img v-if="thumb(r.image_path)" :src="thumb(r.image_path)!" class="cpw-thumb" alt="" />
      <div v-else class="cpw-thumb cpw-thumb--empty"></div>
      <div class="cpw-main">
        <span class="cpw-name" :title="r.name">{{ r.name }}</span>
        <span class="cpw-sub">{{ formatPrice(r.price) }} · {{ r.wears }} wears</span>
      </div>
      <div class="cpw-bar">
        <MiniBar :value="r.cpw" :max="rows[0].cpw" :color="barColor" />
      </div>
      <span class="cpw-value">{{ formatPrice(r.cpw) }}</span>
    </div>
    <div v-if="showAverage && average != null" class="cpw-avg">
      <span>Wardrobe average</span>
      <span class="cpw-avg__value">{{ formatPrice(average) }}</span>
    </div>
  </div>
  <div v-else class="widget-empty">Need prices and logged wears to compute this.</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import MiniBar from '../MiniBar.vue';
import {
  costPerWear,
  cpwDetail,
  rangeData,
  resolveItem,
  wardrobeCpwAverage,
  widgetRange,
  type CpwSource,
} from '../../../lib/insights/metrics';
import { formatNumber1, formatPrice } from '../../../lib/insights/format';
import { imageUrl } from '../../../api/client';
import type { FilteredData } from '../../../lib/insights/types';

const props = defineProps<{ data: FilteredData; config: Record<string, unknown> }>();

const data = computed(() => rangeData(props.data, widgetRange(props.config)));

const direction = computed<'best' | 'worst'>(() =>
  props.config.direction === 'worst' ? 'worst' : 'best',
);

const source = computed<CpwSource>(() => {
  const v = props.config.source;
  return v === 'outfits' || v === 'wear_count' ? v : 'best';
});

const minWears = computed(() => {
  const v = Number(props.config.minWears);
  return Number.isFinite(v) && v > 0 ? Math.min(20, Math.round(v)) : 0;
});

const n = computed(() => {
  const v = Number(props.config.topN);
  return Number.isFinite(v) && v > 0 ? Math.min(15, Math.round(v)) : 5;
});

const showAverage = computed(() => props.config.showAverage !== false);

const cpwOpts = computed(() => ({ minWears: minWears.value, source: source.value }));

const focusItem = computed(() => resolveItem(data.value, props.config, 'focusItem'));

const detail = computed(() =>
  focusItem.value ? cpwDetail(data.value, focusItem.value.id, cpwOpts.value) : null,
);

const rows = computed(() => costPerWear(data.value, direction.value, n.value, cpwOpts.value));

const average = computed(() => wardrobeCpwAverage(data.value, source.value));

const compareGood = computed(() => {
  if (!detail.value || detail.value.cpw == null || detail.value.average == null) return false;
  return detail.value.cpw <= detail.value.average;
});

const compareText = computed(() => {
  if (!detail.value || detail.value.cpw == null || detail.value.average == null) return '';
  const ratio = detail.value.cpw / detail.value.average;
  if (ratio <= 1) {
    const pct = Math.round((1 - ratio) * 100);
    return pct > 0 ? `${pct}% below your wardrobe average` : 'At your wardrobe average';
  }
  return `${formatNumber1(ratio)}× your wardrobe average`;
});

const barColor = computed(() =>
  direction.value === 'best' ? 'var(--md-primary)' : 'var(--md-error)',
);

function thumb(path: string | null): string | null {
  return imageUrl(path, { width: 56 });
}
</script>

<style scoped>
.cpw-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.cpw-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.cpw-thumb {
  width: 32px;
  height: 32px;
  object-fit: cover;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.cpw-thumb--empty {
  background: var(--md-surface-container-highest);
}

.cpw-main {
  width: 130px;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.cpw-name {
  font-size: var(--md-body-small-size);
  color: var(--md-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cpw-sub {
  font-size: var(--md-label-small-size);
  color: var(--md-on-surface-variant);
}

.cpw-bar {
  flex: 1;
  min-width: 0;
}

.cpw-value {
  font-size: var(--md-label-large-size);
  font-weight: var(--md-label-large-weight);
  color: var(--md-on-surface-variant);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.cpw-avg {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.6rem;
  margin-top: 0.25rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--md-outline-variant);
  font-size: var(--md-label-medium-size);
  color: var(--md-on-surface-variant);
}

.cpw-avg__value {
  font-size: var(--md-label-large-size);
  font-weight: var(--md-label-large-weight);
  font-variant-numeric: tabular-nums;
}

/* Focus mode */
.cpw-detail {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0.25rem 0;
}

.cpw-detail__head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.cpw-detail__name {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.cpw-detail__stats {
  display: flex;
  gap: 2rem;
}

.cpw-detail__stat {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.cpw-detail__value {
  font-size: var(--md-headline-small-size);
  line-height: var(--md-headline-small-line);
  font-weight: var(--md-headline-small-weight);
  color: var(--md-on-surface);
  font-variant-numeric: tabular-nums;
}

.cpw-detail__label {
  font-size: var(--md-label-medium-size);
  color: var(--md-on-surface-variant);
}

.cpw-detail__compare {
  font-size: var(--md-body-small-size);
  padding: 0.35rem 0.6rem;
  border-radius: var(--md-shape-small);
  width: fit-content;
}

.cpw-detail__compare--good {
  color: var(--md-on-secondary-container);
  background: var(--md-secondary-container);
}

.cpw-detail__compare--bad {
  color: var(--md-on-error-container);
  background: var(--md-error-container);
}

.widget-empty {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-small-size);
  font-style: italic;
  padding: 1rem 0;
  text-align: center;
}
</style>
