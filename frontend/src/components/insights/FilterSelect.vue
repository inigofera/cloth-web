<template>
  <div class="fselect" ref="root">
    <button
      type="button"
      class="fselect__btn"
      :class="{ 'fselect__btn--active': active }"
      @click="open = !open"
    >
      <span class="fselect__label">{{ label }}</span>
      <span v-if="badgeText != null" class="fselect__badge">{{ badgeText }}</span>
      <MdIcon name="arrow_drop_down" :size="16" />
    </button>

    <div v-if="open" class="fselect__panel">
      <label v-for="opt in options" :key="opt.value" class="fselect__option" @click="selectOption(opt.value)">
        <input
          v-if="multi"
          type="checkbox"
          :checked="selected.includes(opt.value)"
          @change="toggle(opt.value)"
        />
        <span v-else class="fselect__radio" :class="{ 'fselect__radio--on': selected.includes(opt.value) }"></span>
        <span v-if="opt.swatch" class="fselect__swatch" :style="{ background: opt.swatch }"></span>
        <span class="fselect__option-label">{{ opt.label }}</span>
      </label>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue';
import { MdIcon } from '../../ui';

export interface FilterOption {
  value: string;
  label: string;
  swatch?: string | null;
}

const props = withDefaults(
  defineProps<{
    label: string;
    options: FilterOption[];
    selected: string[];
    multi?: boolean;
    active?: boolean;
    badgeText?: string | number | null;
  }>(),
  {
    multi: false,
    active: false,
    badgeText: null,
  },
);

const emit = defineEmits<{
  update: [selected: string[]];
}>();

const open = ref(false);
const root = ref<HTMLElement | null>(null);

function toggle(value: string) {
  if (!props.multi) {
    emit('update', [value]);
    open.value = false;
    return;
  }
  const next = props.selected.includes(value)
    ? props.selected.filter(v => v !== value)
    : [...props.selected, value];
  emit('update', next);
}

// Single-select options have no checkbox input, so the label click must drive
// the selection. Multi-select is handled by the checkbox's @change instead.
function selectOption(value: string) {
  if (!props.multi) toggle(value);
}

function onDocMouseDown(e: MouseEvent) {
  if (root.value && !root.value.contains(e.target as Node)) {
    open.value = false;
  }
}

if (typeof document !== 'undefined') {
  document.addEventListener('mousedown', onDocMouseDown);
}

onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocMouseDown);
});
</script>

<style scoped>
.fselect {
  position: relative;
}

.fselect__btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 34px;
  padding: 0 10px;
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-full);
  background: var(--md-surface-container-low);
  color: var(--md-on-surface-variant);
  font-size: var(--md-label-large-size);
  cursor: pointer;
  transition:
    background var(--md-motion-duration-short) var(--md-motion-easing-standard),
    border-color var(--md-motion-duration-short) var(--md-motion-easing-standard);
}

.fselect__btn:hover {
  background: var(--md-surface-container);
}

.fselect__btn--active {
  background: var(--md-secondary-container);
  border-color: var(--md-secondary);
  color: var(--md-on-secondary-container);
}

.fselect__badge {
  font-size: var(--md-label-small-size);
  font-weight: var(--md-label-small-weight);
  background: var(--md-primary);
  color: var(--md-on-primary);
  border-radius: var(--md-shape-full);
  padding: 1px 7px;
}

.fselect__panel {
  position: absolute;
  top: 40px;
  left: 0;
  z-index: 30;
  min-width: 190px;
  max-height: 300px;
  overflow-y: auto;
  background: var(--md-surface-container-high);
  border: 1px solid var(--md-outline-variant);
  border-radius: var(--md-shape-small);
  box-shadow: var(--md-elevation-3);
  padding: 4px;
}

.fselect__option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: var(--md-shape-extra-small);
  font-size: var(--md-body-medium-size);
  color: var(--md-on-surface);
  cursor: pointer;
  white-space: nowrap;
}

.fselect__option:hover {
  background: var(--md-surface-container-highest);
}

.fselect__radio {
  width: 14px;
  height: 14px;
  border-radius: var(--md-shape-full);
  border: 2px solid var(--md-outline);
  flex-shrink: 0;
}

.fselect__radio--on {
  border: 4px solid var(--md-primary);
}

.fselect__swatch {
  width: 12px;
  height: 12px;
  border-radius: var(--md-shape-extra-small);
  border: 1px solid var(--md-outline-variant);
  flex-shrink: 0;
}

.fselect__option input[type='checkbox'] {
  accent-color: var(--md-primary);
}
</style>
