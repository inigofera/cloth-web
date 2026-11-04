<template>
  <div class="fselect" ref="root">
    <!--
      Combobox trigger (APG "combobox with listbox popup" pattern for
      single-select; plain disclosure button for multi-select checkbox groups).
    -->
    <button
      type="button"
      class="fselect__btn"
      :class="{ 'fselect__btn--active': active }"
      ref="btnRef"
      :aria-haspopup="multi ? 'true' : 'listbox'"
      :aria-expanded="open"
      @click="toggleOpen($event)"
      @keydown="onBtnKeydown"
    >
      <span class="fselect__label">{{ label }}</span>
      <span v-if="badgeText != null" class="fselect__badge">{{ badgeText }}</span>
      <MdIcon name="arrow_drop_down" :size="16" />
    </button>

    <div
      v-if="open"
      class="fselect__panel"
      :role="multi ? 'group' : 'listbox'"
      :aria-label="label + (multi ? ': select one or more options' : ' list')"
      @keydown="onPanelKeydown"
    >
      <template v-if="multi">
        <!-- Native checkboxes are already fully keyboard-operable; the group
             just needs a meaningful name for screen readers. -->
        <label v-for="opt in options" :key="'m-' + opt.value" class="fselect__option" @click="selectOption(opt.value)">
          <input
            type="checkbox"
            :checked="selected.includes(opt.value)"
            @change="toggle(opt.value)"
          />
          <span v-if="opt.swatch" class="fselect__swatch" :style="{ background: opt.swatch }"></span>
          <span class="fselect__option-label">{{ opt.label }}</span>
        </label>
      </template>
      <template v-else>
        <!-- Single-select listbox with roving tabindex: one tab stop (the
             trigger), arrow keys move focus, Enter/Space selects. -->
        <button
          v-for="(opt, i) in options"
          :key="'o-' + opt.value"
          type="button"
          role="option"
          ref="optionEls"
          class="fselect__option"
          :tabindex="i === activeIndex ? 0 : -1"
          :aria-selected="selected.includes(opt.value)"
          @click="selectOption(opt.value)"
        >
          <span class="fselect__radio" :class="{ 'fselect__radio--on': selected.includes(opt.value) }"></span>
          <span v-if="opt.swatch" class="fselect__swatch" :style="{ background: opt.swatch }"></span>
          <span class="fselect__option-label">{{ opt.label }}</span>
        </button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref } from 'vue';
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
const btnRef = ref<HTMLButtonElement | null>(null);
// Collected by the `ref="optionEls"` v-for (single-select mode only); Vue
// hands this a plain array of elements in render order.
const optionEls = ref<Array<HTMLElement | null>>([]);
// Roving-tabindex position inside the listbox. Re-derived on every open so it
// always comes from the current selection rather than stale state.
const activeIndex = ref(-1);

function initialActiveIndex(): number {
  const idx = props.options.findIndex(o => props.selected.includes(o.value));
  return idx >= 0 ? idx : Math.min(0, props.options.length - 1);
}

function openPanel(source: 'pointer' | 'keyboard'): void {
  if (open.value) return;
  activeIndex.value = initialActiveIndex();
  open.value = true;
  // APG combobox listbox pattern: opening from the keyboard moves focus into
  // the listbox so the user can immediately arrow through options.
  if (source === 'keyboard' && !props.multi) {
    void nextTick().then(() => optionEls.value[activeIndex.value]?.focus());
  }
}

function closePanel(restoreFocus: boolean): void {
  const wasOpen = open.value;
  open.value = false;
  if (wasOpen && restoreFocus) btnRef.value?.focus();
}

function toggleOpen(e: MouseEvent): void {
  // Keyboard activation of a button arrives as a click with `detail` 0.
  const source = e.detail === 0 ? 'keyboard' : 'pointer';
  if (open.value) closePanel(true);
  else openPanel(source);
}

function toggle(value: string) {
  if (!props.multi) {
    emit('update', [value]);
    closePanel(true);
    return;
  }
  const next = props.selected.includes(value)
    ? props.selected.filter(v => v !== value)
    : [...props.selected, value];
  emit('update', next);
}

// Single-select options have no checkbox input, so the button click must drive
// the selection. Multi-select is handled by the checkbox's @change instead.
function selectOption(value: string) {
  if (!props.multi) toggle(value);
}

/**
 * Keyboard handling inside the open panel:
 * - single-select: arrow keys / Home / End move the roving focus (listbox pattern)
 * - both modes: Escape closes and returns focus to the trigger button.
 */
function onPanelKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    closePanel(true);
    return;
  }

  if (props.multi) return; // native checkboxes are already fully keyboard-operable

  const n = props.options.length;
  if (n === 0) return;
  let idx: number | null = null;
  switch (e.key) {
    case 'ArrowDown':
      idx = activeIndex.value + 1;
      break;
    case 'ArrowUp':
      idx = activeIndex.value - 1;
      break;
    case 'Home':
      idx = 0;
      break;
    case 'End':
      idx = n - 1;
      break;
    default:
      return;
  }
  e.preventDefault();
  const wrapped = ((idx % n) + n) % n;
  activeIndex.value = wrapped;
  void nextTick().then(() => optionEls.value[wrapped]?.focus());
}

// Opening with ArrowDown from the trigger is the keyboard entry point to the listbox.
function onBtnKeydown(e: KeyboardEvent): void {
  if (!open.value && e.key === 'ArrowDown') {
    e.preventDefault();
    openPanel('keyboard');
  }
}

function onDocMouseDown(e: MouseEvent) {
  if (root.value && !root.value.contains(e.target as Node)) {
    // If focus had wandered into the panel, hand it back to the trigger.
    closePanel(root.value.contains(document.activeElement));
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

/* Single-select options are real buttons; strip their native chrome so they
   line up visually with the multi-select label rows. */
button.fselect__option {
  appearance: none;
  -webkit-appearance: none;
  background: none;
  border: none;
  font: inherit;
  width: 100%;
  text-align: left;
}
</style>
