<template>
  <div class="outfits-view">
    <div v-if="loading" class="loading">Loading outfits...</div>
    <div v-else-if="loadError" class="error-wrapper">
      <ErrorDisplay :message="loadError" type="network" can-retry @retry="loadData" />
    </div>

    <div v-else class="layout">
      <div v-if="showOnboarding" class="onboard-card">
        <h3 class="onboard-title">Welcome to Cloth Web</h3>
        <p class="onboard-copy">
          Your insights are powered by logged outfits. Three quick steps to get started:
        </p>
        <ol class="onboard-steps">
          <li>
            <button type="button" class="link-btn" @click="router.navigate('clothes')">
              Add your clothes in the Clothes tab
            </button>
          </li>
          <li>Log an outfit for a day using the calendar and form on this screen.</li>
          <li>
            <button type="button" class="link-btn" @click="router.navigate('insights')">
              Explore Insights to see what your wardrobe tells you
            </button>
          </li>
        </ol>
        <div class="onboard-actions">
          <MdIconButton icon="close" label="Dismiss welcome guide" variant="standard" @click="dismissOnboarding()" />
        </div>
      </div>

      <div v-if="truncationNotice" class="cap-notice">{{ truncationNotice }}</div>

      <section class="calendar-section">
        <div class="calendar-header">
          <button class="nav-btn" @click="prevMonth" aria-label="Previous month">‹</button>
          <h2 class="month-label">{{ monthLabel }}</h2>
          <button class="nav-btn" @click="nextMonth" aria-label="Next month">›</button>
          <MdButton variant="filled" @click="logToday()">
            Log today's outfit
          </MdButton>
          <button
            class="today-btn sync-btn"
            :disabled="isSyncingDiscord"
            title="Check and import new outfits from Discord"
            @click="syncDiscord(true)"
          >
            {{ isSyncingDiscord ? 'Syncing...' : 'Sync Discord' }}
          </button>
        </div>

        <div class="calendar-grid" role="grid" aria-label="Outfit calendar">
          <div role="row" class="weekday-row">
            <span v-for="day in WEEKDAYS" :key="day" role="columnheader" class="weekday">{{ day }}</span>
          </div>
          <template v-for="(week, wi) in weeks" :key="'week-' + wi">
            <div role="row" class="calendar-week">
              <button
                v-for="cell in week"
                :key="cell.key"
                type="button"
                role="gridcell"
                :ref="(el: unknown) => setDayCellRef(cell, el)"
                class="day-cell"
                :class="{
                  other: cell.otherMonth,
                  today: cell.isToday,
                  selected: cell.key === selectedDate
                }"
                :tabindex="cell.key === focusedKey ? 0 : -1"
                :aria-selected="cell.key === selectedDate"
                :aria-label="cellLabel(cell)"
                @click="selectDay(cell.key)"
                @keydown="onCalendarKeydown($event, cell)"
              >
                <span class="day-number">{{ cell.day }}</span>
                <div class="outfit-chips">
                  <div
                    v-for="outfit in cell.outfits.slice(0, 3)"
                    :key="outfit.id"
                    class="outfit-chip"
                    :title="chipTitle(outfit)"
                  >
                    <img v-if="firstItemImage(outfit, 28)" :src="firstItemImage(outfit, 28)!" class="chip-thumb" alt="" />
                    <span class="chip-label">{{ chipLabel(outfit) }}</span>
                  </div>
                  <div v-if="cell.outfits.length > 3" class="more-chip">
                    +{{ cell.outfits.length - 3 }} more
                  </div>
                </div>
              </button>
            </div>
          </template>
        </div>
      </section>

      <aside class="day-panel" ref="dayPanelRef" tabindex="-1">
        <ErrorDisplay
          v-if="actionError"
          :message="actionError"
          dismissible
          class="panel-error"
          @dismiss="actionError = null"
        />

        <template v-if="selectedDate">
          <h3 class="day-title">{{ selectedDateLabel }}</h3>

          <div v-if="dayOutfits.length > 0" class="day-outfits">
            <div v-for="outfit in dayOutfits" :key="outfit.id" class="outfit-card">
              <div v-if="outfit.image_path" class="outfit-image-wrapper">
                <button
                  type="button"
                  class="outfit-image-button"
                  title="Click to view full size"
                  aria-label="Click to expand outfit photo"
                  @click="openExpandedPreview(outfit.image_path, outfit.notes || 'Outfit Photo')"
                >
                  <img
                    :src="imageUrl(outfit.image_path, { width: 720 })!"
                    alt="Try-on outfit photo"
                    class="outfit-card-img"
                    loading="lazy"
                  />
                  <div class="expand-badge">
                    <MdIcon name="fullscreen" :size="16" />
                    <span>Expand</span>
                  </div>
                </button>
              </div>
              <div class="outfit-items">

                <span v-for="itemId in outfit.item_ids" :key="itemId" class="item-tag">
                  {{ getItemName(itemId) }}
                </span>
                <span v-if="outfit.item_ids.length === 0" class="item-tag empty">no items</span>
              </div>
              <p v-if="outfit.notes" class="outfit-notes">{{ outfit.notes }}</p>

              <div v-if="confirmingDeleteId === outfit.id" class="confirm-delete">
                <span class="confirm-prompt">Delete this outfit?</span>
                <div class="confirm-buttons">
                  <button
                    type="button"
                    class="confirm-btn danger"
                    :disabled="deletingId === outfit.id"
                    @click="removeOutfit(outfit)"
                  >
                    {{ deletingId === outfit.id ? 'Deleting…' : 'Yes, delete' }}
                  </button>
                  <button
                    type="button"
                    class="confirm-btn secondary"
                    :disabled="deletingId === outfit.id"
                    @click="confirmingDeleteId = null"
                  >
                    Cancel
                  </button>
                </div>
              </div>
              <button
                v-else
                type="button"
                class="delete-btn"
                @click="confirmingDeleteId = outfit.id"
              >
                Delete
              </button>
            </div>
          </div>

          <form class="add-form" @submit.prevent="submitOutfit">
            <h4>Add outfit</h4>

            <label class="field">
              <span class="field-label">Notes</span>
              <input v-model="notes" type="text" placeholder="optional" />
            </label>

            <div class="field">
              <span class="field-label">Items ({{ selectedItems.length }} selected)</span>

              <div v-if="selectedItems.length > 0" class="selected-chips">
                <span
                  v-for="id in selectedItems"
                  :key="id"
                  class="selected-chip"
                >
                  {{ getItemName(id) }}
                  <button
                    type="button"
                    class="remove-chip-btn"
                    aria-label="Remove item"
                    @click="removeItemFromSelection(id)"
                  >✕</button>
                </span>
              </div>

              <div class="picker-controls">
                <input
                  v-model="pickerSearch"
                  type="search"
                  class="picker-search-input"
                  aria-label="Search clothing items"
                  placeholder="Search items…"
                />
                <select v-if="categories.length > 0" v-model="pickerCategory" class="picker-cat-select">
                  <option :value="null">All categories</option>
                  <option v-for="cat in categories" :key="cat.id" :value="cat.id">{{ cat.name }}</option>
                </select>
              </div>

              <div class="item-picker">
                <label
                  v-for="item in filteredPickerItems"
                  :key="item.id"
                  class="item-option"
                  :class="{ checked: selectedItems.includes(item.id), inactive: !item.is_active }"
                >
                  <input
                    v-model="selectedItems"
                    type="checkbox"
                    :value="item.id"
                  />
                  <img v-if="imageUrl(item.image_path, { width: 56 })" :src="imageUrl(item.image_path, { width: 56 })" class="thumb" loading="lazy" />
                  <span class="item-name">{{ item.name }}</span>
                  <span v-if="lastWornLabel(item.id)" class="picker-last-worn">last worn {{ lastWornLabel(item.id) }}</span>
                  <span v-if="!item.is_active" class="picker-inactive-tag">Inactive</span>
                </label>
                <div v-if="filteredPickerItems.length === 0" class="picker-empty">
                  <template v-if="items.length === 0">
                    No clothing items yet — add some in the
                    <button type="button" class="link-btn" @click="router.navigate('clothes')">Clothes</button>
                    tab.
                  </template>
                  <template v-else>No items match your search</template>
                </div>
              </div>
            </div>

            <!-- Try-On Preview Generation -->
            <div class="tryon-section">
              <div class="tryon-controls">
                <MdButton
                  variant="tonal"
                  :disabled="selectedItems.length === 0 || isGeneratingPreview"
                  @click="handleGeneratePreview"
                >
                  <template #icon>
                    <MdIcon name="sparkles" :size="18" />
                  </template>
                  {{ isGeneratingPreview ? 'Generating outfit preview (10–20s)...' : 'Generate Try-On Preview' }}
                </MdButton>
                <MdButton
                  v-if="isGeneratingPreview"
                  variant="text"
                  @click="cancelGeneration()"
                >
                  Cancel
                </MdButton>
              </div>

              <div v-if="isGeneratingPreview" class="tryon-progress" role="status">
                <span class="tryon-progress__bar"><span class="tryon-progress__fill"></span></span>
                <span class="tryon-progress__label">Elapsed {{ genSeconds }}s — usually 10–20s</span>
              </div>

              <div v-if="generationError" class="tryon-error-block">
                <ErrorDisplay
                  :message="generationErrorMessage"
                  type="unknown"
                  dismissible
                  @dismiss="generationError = null"
                />
                <div v-if="isMissingSettings" class="missing-settings-cta">
                  <MdButton variant="outlined" @click="router.navigate('settings')">
                    Go to Settings
                  </MdButton>
                </div>
              </div>

              <div v-if="generatedImagePath" class="tryon-preview-box">
                <div
                  class="tryon-preview-wrapper"
                  role="button"
                  tabindex="0"
                  title="Click to view full size"
                  aria-label="Click to expand outfit preview"
                  @click="openExpandedPreview(generatedImagePath, 'Generated Outfit Preview')"
                  @keydown.enter.prevent="openExpandedPreview(generatedImagePath, 'Generated Outfit Preview')"
                  @keydown.space.prevent="openExpandedPreview(generatedImagePath, 'Generated Outfit Preview')"
                >
                  <img
                    :src="imageUrl(generatedImagePath, { width: 1200 })!"
                    alt="Generated outfit preview"
                    class="tryon-preview-thumb"
                  />
                  <div class="expand-badge">
                    <MdIcon name="fullscreen" :size="16" />
                    <span>Expand</span>
                  </div>
                </div>
                <div class="tryon-preview-actions">
                  <MdButton
                    variant="outlined"
                    @click="saveImageToDisk(generatedImagePath)"
                  >
                    <template #icon><MdIcon name="download" :size="16" /></template>
                    Save Image
                  </MdButton>
                  <MdButton variant="text" @click="discardPreview">
                    Discard
                  </MdButton>
                  <MdButton
                    variant="tonal"
                    :disabled="isGeneratingPreview"
                    @click="handleGeneratePreview"
                  >
                    <template #icon><MdIcon name="sparkles" :size="16" /></template>
                    Regenerate
                  </MdButton>
                </div>
              </div>
            </div>

            <div v-if="formError" class="error">{{ formError }}</div>


            <div class="form-actions">
              <button
                type="submit"
                class="primary"
                :disabled="saving || (selectedItems.length === 0 && !notes.trim())"
              >
                {{ saving ? 'Saving…' : 'Add outfit' }}
              </button>
              <button type="button" class="secondary" @click="closeForm">Cancel</button>
            </div>
          </form>
        </template>

        <div v-else class="hint">Select a day on the calendar to add an outfit.</div>
      </aside>
    </div>

    <!-- Lightbox Expanded Image Modal -->
    <Teleport to="body">
      <Transition name="lightbox-fade">
        <div
          v-if="expandedImagePath"
          class="lightbox-overlay"
          role="dialog"
          aria-modal="true"
          :aria-label="expandedImageTitle"
          tabindex="-1"
          ref="lightboxRef"
          @click.self="closeExpandedPreview"
        >
          <div class="lightbox-dialog" role="document">
            <header class="lightbox-header">
              <span class="lightbox-title">{{ expandedImageTitle }}</span>
              <div class="lightbox-header-actions">
                <MdButton
                  variant="tonal"
                  @click="saveImageToDisk(expandedImagePath)"
                >
                  <template #icon><MdIcon name="download" :size="18" /></template>
                  Save Image
                </MdButton>
                <MdIconButton
                  icon="close"
                  label="Close preview"
                  variant="standard"
                  @click="closeExpandedPreview"
                />
              </div>
            </header>
            <div class="lightbox-body" @click.self="closeExpandedPreview">
              <img
                :src="imageUrl(expandedImagePath, { width: 1600 })!"
                :alt="expandedImageTitle"
                class="lightbox-img"
              />
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <Toast v-model:show="showToast" :message="toastMessage" />
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, computed, onActivated, onMounted, onUnmounted, watch } from 'vue';
import { api, imageUrl } from '../api/client';
import { sanitizeText, OUTFIT_NOTES_MAX } from '../lib/sanitize';
import { ErrorDisplay, Toast, MdButton, MdIconButton, MdIcon } from '../ui';
import { classifyError } from '../lib/errors';
import { formatLocalDateKey, parseOutfitDateKey, formatSelectedDateLabel } from '../lib/dates';
import { useRouter } from '../lib/router';
import { formatTruncationNotice, useWardrobe, WARDROBE_STALE_MS } from '../lib/wardrobe';
import { buildLastWornMap, sortByRecency, formatLastWorn } from '../lib/picker';
import type { Outfit } from '../api/types';

const router = useRouter();

const isGeneratingPreview = ref(false);
const generatedImagePath = ref<string | null>(null);
const generationError = ref<unknown | null>(null);
const expandedImagePath = ref<string | null>(null);
const expandedImageTitle = ref<string>('Outfit Preview');

// Dialog focus management: remember where focus came from, move focus into the
// dialog when it opens, and hand focus back to the trigger on close.
const lightboxRef = ref<HTMLElement | null>(null);
let previewTriggerEl: HTMLElement | null = null;

function openExpandedPreview(path: string | null, title = 'Outfit Preview') {
  if (!path) return;
  previewTriggerEl = (document.activeElement as HTMLElement | null) ?? null;
  expandedImagePath.value = path;
  expandedImageTitle.value = title;
}

async function closeExpandedPreview() {
  const trigger = previewTriggerEl;
  previewTriggerEl = null;
  expandedImagePath.value = null;
  await nextTick();
  if (trigger && document.contains(trigger)) {
    trigger.focus();
  }
}

watch(expandedImagePath, async (value) => {
  if (!value) return;
  await nextTick();
  lightboxRef.value?.focus();
});

async function saveImageToDisk(path: string | null, customName?: string) {
  if (!path) return;
  const src = imageUrl(path, { width: 1600 }) || imageUrl(path);
  if (!src) return;
  try {
    const resp = await fetch(src);
    const blob = await resp.blob();
    const objectUrl = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = objectUrl;
    const datePart = selectedDate.value || new Date().toISOString().slice(0, 10);
    link.download = customName || `outfit-${datePart}.jpg`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(objectUrl);
    toastMessage.value = 'Image saved.';
    showToast.value = true;
  } catch (e) {
    console.error('Failed to download image directly, falling back to window.open', e);
    window.open(src, '_blank');
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && expandedImagePath.value) {
    closeExpandedPreview();
  }
}

const generationErrorMessage = computed(() => {
  if (!generationError.value) return '';
  return classifyError(generationError.value).message;
});

const isMissingSettings = computed(() => {
  const msg = generationErrorMessage.value.toLowerCase();
  return msg.includes('settings') || msg.includes('api key') || msg.includes('reference photo') || msg.includes('configured');
});

// A long-running generation must be legible: show elapsed time, give the user
// a cancel path, and never leave them staring at an unresponsive button.
const genSeconds = ref(0);
let genTimer: number | null = null;
let previewAbort: AbortController | null = null;

function stopGenTimer(): void {
  if (genTimer !== null) {
    window.clearInterval(genTimer);
    genTimer = null;
  }
}

async function handleGeneratePreview() {
  if (selectedItems.value.length === 0 || isGeneratingPreview.value) return;
  isGeneratingPreview.value = true;
  generationError.value = null;
  generatedImagePath.value = null;
  previewAbort.value?.abort(); // defensive: never two overlapping generations
  const controller = new AbortController();
  previewAbort.value = controller;
  genSeconds.value = 0;
  stopGenTimer();
  genTimer = window.setInterval(() => {
    genSeconds.value += 1;
  }, 1000);
  try {
    const res = await api.generateOutfitPreview(selectedItems.value, controller.signal);
    generatedImagePath.value = res.image_path;
  } catch (err) {
    if (err instanceof DOMException && err.name === 'AbortError') {
      // User cancelled the generation — that is not an error state.
      return;
    }
    generationError.value = err;
  } finally {
    stopGenTimer();
    isGeneratingPreview.value = false;
    previewAbort.value = null;
  }
}

function cancelGeneration(): void {
  previewAbort.value?.abort();
}

function discardPreview() {
  generatedImagePath.value = null;
}


const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

interface CalendarCell {
  key: string;
  day: number;
  otherMonth: boolean;
  isToday: boolean;
  outfits: Outfit[];
}

const wardrobe = useWardrobe();
const {
  outfits,
  items,
  categories,
  loaded,
  loading,
  error: loadError
} = wardrobe;

const actionError = ref<string | null>(null);

// First-run guidance: only shown while the wardrobe is genuinely empty, and it
// can be dismissed permanently (per browser) via localStorage.
const ONBOARDING_STORAGE_KEY = 'cw.onboarding.v1.dismissed';
function initialOnboardingDismissed(): boolean {
  try {
    return typeof window !== 'undefined' && window.localStorage.getItem(ONBOARDING_STORAGE_KEY) === '1';
  } catch {
    return true; // storage unavailable: never nag
  }
}
const onboardingDismissed = ref(initialOnboardingDismissed());

function dismissOnboarding(): void {
  onboardingDismissed.value = true;
  try {
    window.localStorage.setItem(ONBOARDING_STORAGE_KEY, '1');
  } catch {
    // Non-fatal: the dismissal simply will not persist.
  }
}

const pickerSearch = ref('');
const pickerCategory = ref<number | null>(null);

function getItemName(id: string): string {
  return items.value.find(i => i.id === id)?.name ?? 'Item';
}

function removeItemFromSelection(id: string) {
  selectedItems.value = selectedItems.value.filter(itemId => itemId !== id);
}

// Items the user most recently logged first: logging is a daily habit, so the
// picker should surface familiar garments before the long tail. Items never
// logged sink to the bottom (stable name order as tiebreak).
const lastWorn = computed(() => buildLastWornMap(outfits.value));

function lastWornLabel(id: string): string | null {
  const key = lastWorn.value.get(id);
  if (!key) return null;
  return formatLastWorn(key, todayDateKey);
}

const filteredPickerItems = computed(() => {
  const q = pickerSearch.value.trim().toLowerCase();
  const cat = pickerCategory.value;

  const matches = items.value.filter(item => {
    if (cat !== null && item.category_id !== cat) {
      return false;
    }
    if (q) {
      const matchName = item.name.toLowerCase().includes(q);
      const matchNotes = item.notes?.toLowerCase().includes(q);
      const matchColor = item.color_id?.toLowerCase().includes(q);
      return matchName || matchNotes || matchColor;
    }
    return true;
  });

  return sortByRecency(matches, lastWorn.value);
});

const confirmingDeleteId = ref<string | null>(null);
const deletingId = ref<string | null>(null);
const showToast = ref(false);
const toastMessage = ref('');

const now = new Date();
const viewYear = ref(now.getFullYear());
const viewMonth = ref(now.getMonth());
const todayDateKey = dateKey(now);

// The core action of this app is logging the outfit worn *today*. Defaulting
// to that date keeps the landing screen immediately actionable; any explicit
// selection made by the user always takes precedence.
const selectedDate = ref<string | null>(todayDateKey);
const focusedKey = ref<string>(todayDateKey);
const notes = ref('');
const selectedItems = ref<string[]>([]);
const saving = ref(false);
const formError = ref<string | null>(null);

// A draft selection may survive in this kept-alive view after the item is
// deleted in the Clothes tab. Drop ids that no longer exist so the form cannot
// try to link a deleted item.
watch(items, currentItems => {
  const validIds = new Set(currentItems.map(item => item.id));
  if (selectedItems.value.some(id => !validIds.has(id))) {
    selectedItems.value = selectedItems.value.filter(id => validIds.has(id));
  }
});

function dateKey(d: Date): string {
  return formatLocalDateKey(d);
}

function outfitKey(o: Outfit): string {
  return parseOutfitDateKey(o.date);
}

const outfitsByDate = computed<Map<string, Outfit[]>>(() => {
  const map = new Map<string, Outfit[]>();
  outfits.value.forEach(o => {
    const key = outfitKey(o);
    const list = map.get(key);
    if (list) {
      list.push(o);
    } else {
      map.set(key, [o]);
    }
  });
  return map;
});

const cells = computed<CalendarCell[]>(() => {
  const first = new Date(viewYear.value, viewMonth.value, 1);
  const startOffset = (first.getDay() + 6) % 7;
  const start = new Date(viewYear.value, viewMonth.value, 1 - startOffset);
  const todayKey = dateKey(new Date());
  const byDate = outfitsByDate.value;

  const result: CalendarCell[] = [];
  for (let i = 0; i < 42; i++) {
    const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
    const key = dateKey(d);
    result.push({
      key,
      day: d.getDate(),
      otherMonth: d.getMonth() !== viewMonth.value,
      isToday: key === todayKey,
      outfits: byDate.get(key) ?? []
    });
  }
  return result;
});

const monthLabel = computed(() => {
  return new Date(viewYear.value, viewMonth.value, 1)
    .toLocaleString(undefined, { month: 'long', year: 'numeric' });
});

// The 42 calendar cells grouped into the six visible weeks (ARIA grid rows).
const weeks = computed<CalendarCell[][]>(() => {
  const result: CalendarCell[][] = [];
  for (let i = 0; i < cells.value.length; i += 7) {
    result.push(cells.value.slice(i, i + 7));
  }
  return result;
});

const dayOutfits = computed<Outfit[]>(() => {
  if (!selectedDate.value) return [];
  return outfitsByDate.value.get(selectedDate.value) ?? [];
});

const selectedDateLabel = computed(() => {
  return formatSelectedDateLabel(selectedDate.value);
});

const showOnboarding = computed(
  () => loaded.value && items.value.length === 0 && outfits.value.length === 0 && !onboardingDismissed.value,
);

// Data-completeness notice (UX_ANALYSIS.md P1-B3): rendered only when the
// server reports that more rows exist than were loaded.
const truncationNotice = computed(() =>
  formatTruncationNotice(
    items.value.length,
    wardrobe.itemTotal.value,
    outfits.value.length,
    wardrobe.outfitTotal.value
  ),
);

function prevMonth() {
  if (viewMonth.value === 0) {
    viewMonth.value = 11;
    viewYear.value -= 1;
  } else {
    viewMonth.value -= 1;
  }
}

function nextMonth() {
  if (viewMonth.value === 11) {
    viewMonth.value = 0;
    viewYear.value += 1;
  } else {
    viewMonth.value += 1;
  }
}

const dayPanelRef = ref<HTMLElement | null>(null);

/**
 * On the stacked (narrow) layout the day panel sits below the calendar and is
 * off-screen; selecting a day must have an observable consequence, so scroll
 * the panel into view. With `focus`, also move focus there for keyboard and
 * screen-reader users.
 */
function revealDayPanel(focus = false): void {
  const panel = dayPanelRef.value;
  if (!panel) return;
  if (window.innerWidth <= 900) {
    panel.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
  if (focus) {
    panel.focus();
  }
}

function logToday(): void {
  const today = new Date();
  viewYear.value = today.getFullYear();
  viewMonth.value = today.getMonth();
  selectedDate.value = dateKey(today);
  focusedKey.value = selectedDate.value;
  formError.value = null;
  generationError.value = null;
  revealDayPanel(true);
}

function cellLabel(cell: CalendarCell): string {
  const parts: string[] = [formatSelectedDateLabel(cell.key)];
  if (cell.outfits.length > 0) {
    parts.push(`${cell.outfits.length} ${cell.outfits.length === 1 ? 'outfit' : 'outfits'} logged`);
  }
  if (cell.isToday) parts.push('today');
  return parts.join(', ');
}

const dayCellEls = new Map<string, HTMLElement>();

function setDayCellRef(cell: CalendarCell, el: unknown): void {
  const node = el as HTMLElement | null;
  if (node) {
    dayCellEls.set(cell.key, node);
  } else {
    dayCellEls.delete(cell.key);
  }
}

function keyToDate(key: string): Date {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(key);
  if (!m) return new Date();
  return new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
}

function addMonthsClamped(d: Date, delta: number): Date {
  const monthIndex = d.getMonth() + delta;
  // Day 0 of month `monthIndex + 1` is the last day of `monthIndex`.
  const daysInMonth = new Date(d.getFullYear(), monthIndex + 1, 0).getDate();
  return new Date(d.getFullYear(), monthIndex, Math.min(d.getDate(), daysInMonth));
}

/** Navigate the visible grid so `date` falls inside it (PageUp/PageDown). */
function navigateTo(date: Date): void {
  viewYear.value = date.getFullYear();
  viewMonth.value = date.getMonth();
}

function focusCell(key: string): void {
  focusedKey.value = key;
  dayCellEls.get(key)?.focus();
}

/**
 * Keyboard navigation for the calendar grid (single tab stop, roving tabindex):
 * arrows move by day/week, Home/End jump within the week, PageUp/PageDown
 * switch months. Selection and focus advance together because selecting a day
 * is the only action this grid performs.
 */
function onCalendarKeydown(e: KeyboardEvent, cell: CalendarCell): void {
  const all = cells.value;
  const idx = all.findIndex(c => c.key === cell.key);
  if (idx === -1) return;

  let targetIdx: number | null = null;
  switch (e.key) {
    case 'ArrowLeft':
      targetIdx = Math.max(idx - 1, 0);
      break;
    case 'ArrowRight':
      targetIdx = Math.min(idx + 1, all.length - 1);
      break;
    case 'ArrowUp':
      targetIdx = Math.max(idx - 7, 0);
      break;
    case 'ArrowDown':
      targetIdx = Math.min(idx + 7, all.length - 1);
      break;
    case 'Home': {
      const offset = idx % 7;
      targetIdx = idx - offset;
      break;
    }
    case 'End': {
      const offset = idx % 7;
      targetIdx = Math.min(idx + (6 - offset), all.length - 1);
      break;
    }
    case 'PageUp':
    case 'PageDown': {
      const delta = e.key === 'PageUp' ? -1 : 1;
      const target = addMonthsClamped(keyToDate(cell.key), delta);
      navigateTo(target);
      // The grid re-renders with the new month; focus after Vue updates.
      void nextTick().then(() => {
        selectDay(formatLocalDateKey(target));
        dayCellEls.get(formatLocalDateKey(target))?.focus();
      });
      e.preventDefault();
      return;
    }
    default:
      return;
  }

  e.preventDefault();
  const target = all[targetIdx];
  if (!target) return;
  selectDay(target.key);
  focusCell(target.key);
}

function selectDay(key: string) {
  selectedDate.value = key;
  focusedKey.value = key;
  formError.value = null;
  generatedImagePath.value = null;
  generationError.value = null;
  revealDayPanel();
}

function closeForm() {
  selectedDate.value = null;
  notes.value = '';
  selectedItems.value = [];
  formError.value = null;
  generatedImagePath.value = null;
  generationError.value = null;
  isGeneratingPreview.value = false;
}

function firstItemImage(outfit: Outfit, width: number): string | null {
  if (outfit.image_path) {
    const url = imageUrl(outfit.image_path, { width });
    if (url) return url;
  }
  for (const item of wardrobe.itemsOf(outfit)) {
    const url = imageUrl(item.image_path, { width });
    if (url) return url;
  }
  return null;
}

function chipLabel(outfit: Outfit): string {
  if (outfit.notes) return outfit.notes;
  // The server orders `item_ids` by item name, so the first id is the same
  // garment the joined payload used to lead with.
  const firstId = outfit.item_ids[0];
  if (firstId) {
    const name = items.value.find(i => i.id === firstId)?.name;
    if (name) return name;
  }
  return 'Outfit';
}

function chipTitle(outfit: Outfit): string {
  const itemNames = wardrobe.itemsOf(outfit).map(i => i.name).join(', ');
  return [outfit.notes, itemNames].filter(Boolean).join(' — ');
}

async function submitOutfit() {
  if (!selectedDate.value) return;
  saving.value = true;
  formError.value = null;
  try {
    const created = await api.createOutfit({
      date: selectedDate.value,
      notes: sanitizeText(notes.value, { max: OUTFIT_NOTES_MAX }) || null,
      item_ids: selectedItems.value,
      image_path: generatedImagePath.value || null
    });
    wardrobe.addOutfit(created);
    notes.value = '';
    selectedItems.value = [];
    generatedImagePath.value = null;
    generationError.value = null;
  } catch (err) {
    formError.value = classifyError(err).message;
  } finally {
    saving.value = false;
  }
}


async function removeOutfit(outfit: Outfit) {
  actionError.value = null;
  deletingId.value = outfit.id;
  try {
    await api.deleteOutfit(outfit.id);
    wardrobe.removeOutfit(outfit);
    confirmingDeleteId.value = null;
    toastMessage.value = 'Outfit deleted.';
    showToast.value = true;
  } catch (err) {
    actionError.value = classifyError(err).message;
  } finally {
    deletingId.value = null;
  }
}

function loadData() {
  void wardrobe.refresh();
}

const isSyncingDiscord = ref(false);

async function syncDiscord(manual = false) {
  if (isSyncingDiscord.value) return;
  isSyncingDiscord.value = true;
  try {
    const res = await api.syncDiscordOutfits();
    if (res.imported_count > 0) {
      let msg = `Imported ${res.imported_count} outfit${res.imported_count > 1 ? 's' : ''}`;
      if (res.new_items_created > 0) {
        msg += ` (${res.new_items_created} new item${res.new_items_created > 1 ? 's' : ''} added)`;
      }
      msg += ' successfully from Discord.';
      toastMessage.value = msg;
      showToast.value = true;
      await wardrobe.refresh();
    } else if (manual) {
      toastMessage.value = 'Discord is up to date (no new outfits found).';
      showToast.value = true;
    }
  } catch (err) {
    if (manual) {
      actionError.value = classifyError(err).message;
    }
  } finally {
    isSyncingDiscord.value = false;
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown);
  void wardrobe.load();
  void syncDiscord(false);
});

onActivated(() => {
  // Local mutations already update the shared store; this catches changes made
  // in another tab/device after the snapshot has gone stale.
  void wardrobe.load({ maxAgeMs: WARDROBE_STALE_MS });
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  stopGenTimer();
  previewAbort.value?.abort();
});
</script>

<style scoped>
.outfits-view {
  margin-top: 1.5rem;
}

.layout {
  display: flex;
  gap: 1.5rem;
  align-items: flex-start;
}

.calendar-section {
  flex: 1;
  min-width: 0;
}

.calendar-header {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1rem;
}

.month-label {
  margin: 0;
  font-size: 1.25rem;
  color: #333;
  flex: 1;
}

.nav-btn,
.sync-btn {
  margin-left: 0.35rem;
}

.today-btn {
  padding: 0.35rem 0.75rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fff;
  cursor: pointer;
  font-size: 1rem;
}

.nav-btn:hover,
.today-btn:hover {
  background: #f0f0f0;
}

.calendar-grid {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.weekday-row,
.calendar-week {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
}

.weekday {
  text-align: center;
  font-size: 0.8rem;
  font-weight: 600;
  color: #666;
  padding: 0.25rem 0;
}

.day-cell {
  appearance: none;
  -webkit-appearance: none;
  text-align: left;
  min-height: 84px;
  border: 1px solid #e0e0e0;
  border-radius: 6px;
  background: #fff;
  padding: 0.35rem;
  cursor: pointer;
  overflow: hidden;
}

.day-cell:hover {
  border-color: #007bff;
}

.day-cell.other {
  background: #f7f7f7;
  color: #aaa;
}

.day-cell.today {
  border-color: #007bff;
  box-shadow: inset 0 0 0 1px #007bff;
}

.day-cell.selected {
  background: #e3f2fd;
  border-color: #007bff;
}

.day-number {
  display: block;
  font-size: 0.85rem;
  font-weight: 600;
  margin-bottom: 0.25rem;
}

.outfit-chips {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.outfit-chip {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 0.7rem;
  background: #e3f2fd;
  border: 1px solid #90caf9;
  border-radius: 3px;
  padding: 1px 4px;
  white-space: nowrap;
  overflow: hidden;
}

.chip-thumb {
  width: 14px;
  height: 14px;
  object-fit: cover;
  border-radius: 2px;
  flex-shrink: 0;
}

.chip-label {
  overflow: hidden;
  text-overflow: ellipsis;
}

.more-chip {
  font-size: 0.7rem;
  color: #666;
  padding-left: 4px;
}

.day-panel {
  width: 360px;
  flex-shrink: 0;
  border: 1px solid #ddd;
  border-radius: 8px;
  background: #f9f9f9;
  padding: 1.25rem;
}

.day-title {
  margin: 0 0 1rem;
  font-size: 1.05rem;
  color: #333;
}

.hint {
  color: #666;
  font-style: italic;
  padding: 1rem 0;
}

.day-outfits {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-bottom: 1.25rem;
}

.outfit-card {
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 6px;
  padding: 0.6rem 0.75rem;
}

.outfit-items {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.item-tag {
  font-size: 0.78rem;
  background: #eef;
  border: 1px solid #ccd;
  border-radius: 3px;
  padding: 0.1rem 0.45rem;
}

.item-tag.empty {
  color: #999;
  background: #f5f5f5;
  border-color: #e0e0e0;
}

.outfit-notes {
  margin: 0.4rem 0 0;
  font-size: 0.85rem;
  color: #555;
}

.delete-btn {
  margin-top: 0.5rem;
  font-size: 0.75rem;
  color: #c0392b;
  background: none;
  border: 1px solid #e0b4ae;
  border-radius: 4px;
  padding: 0.2rem 0.5rem;
  cursor: pointer;
}

.delete-btn:hover {
  background: #fdecea;
}

.confirm-delete {
  margin-top: 0.5rem;
  padding: 0.4rem 0.6rem;
  background: #fdecea;
  border: 1px solid #e0b4ae;
  border-radius: 4px;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.confirm-prompt {
  font-size: 0.8rem;
  font-weight: 600;
  color: #c0392b;
}

.confirm-buttons {
  display: flex;
  gap: 0.4rem;
}

.confirm-btn {
  padding: 0.2rem 0.6rem;
  font-size: 0.75rem;
  border-radius: 3px;
  border: 1px solid #ccc;
  background: #fff;
  cursor: pointer;
}

.confirm-btn.danger {
  background: #c0392b;
  color: #fff;
  border-color: #c0392b;
}

.confirm-btn.danger:hover {
  background: #a93226;
}

.confirm-btn.secondary:hover {
  background: #f5f5f5;
}

.add-form {
  border-top: 2px solid #007bff;
  padding-top: 1rem;
}

.add-form h4 {
  margin: 0 0 0.75rem;
  color: #333;
}

.field {
  display: block;
  margin-bottom: 0.9rem;
}

.field-label {
  display: block;
  font-size: 0.85rem;
  font-weight: 600;
  color: #555;
  margin-bottom: 0.3rem;
}

.field input[type='text'] {
  width: 100%;
  box-sizing: border-box;
  padding: 0.45rem 0.6rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  font-size: 0.9rem;
}

.item-picker {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  max-height: 240px;
  overflow-y: auto;
  border: 1px solid #ddd;
  border-radius: 4px;
  background: #fff;
  padding: 0.5rem;
}

.picker-empty {
  color: #999;
  font-size: 0.85rem;
  font-style: italic;
  text-align: center;
  padding: 0.5rem 0;
}

.selected-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  margin-bottom: 0.5rem;
}

.selected-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.75rem;
  background: #e3f2fd;
  color: #0b57d0;
  border: 1px solid #90caf9;
  border-radius: 4px;
  padding: 0.15rem 0.45rem;
}

.remove-chip-btn {
  background: transparent;
  border: none;
  color: inherit;
  font-size: 0.75rem;
  cursor: pointer;
  padding: 0;
  line-height: 1;
}

.remove-chip-btn:hover {
  color: #c0392b;
}

.picker-controls {
  display: flex;
  gap: 0.4rem;
  margin-bottom: 0.5rem;
}

.picker-search-input {
  flex: 1;
  min-width: 0;
  padding: 0.35rem 0.5rem;
  font-size: 0.82rem;
  border: 1px solid #ccc;
  border-radius: 4px;
}

.picker-cat-select {
  padding: 0.35rem 0.4rem;
  font-size: 0.82rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  max-width: 120px;
}

.picker-inactive-tag {
  font-size: 0.68rem;
  color: #888;
  background: #eee;
  padding: 0.05rem 0.3rem;
  border-radius: 3px;
  margin-left: auto;
}

.item-option.inactive {
  opacity: 0.7;
}

.item-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.88rem;
  padding: 0.3rem 0.4rem;
  border-radius: 4px;
  cursor: pointer;
}

.item-option:hover {
  background: #f0f6ff;
}

.item-option.checked {
  background: #e3f2fd;
}

.item-option input {
  accent-color: #007bff;
}

.thumb {
  width: 28px;
  height: 28px;
  object-fit: cover;
  border-radius: 4px;
  border: 1px solid #ddd;
  flex-shrink: 0;
}

.item-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.form-actions {
  display: flex;
  gap: 0.5rem;
}

.primary {
  padding: 0.45rem 1rem;
  border: none;
  border-radius: 4px;
  background: #007bff;
  color: #fff;
  font-size: 0.9rem;
  cursor: pointer;
}

.primary:hover {
  background: #0069d9;
}

.primary:disabled {
  background: #99c5f4;
  cursor: default;
}

.secondary {
  padding: 0.45rem 1rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fff;
  font-size: 0.9rem;
  cursor: pointer;
}

.secondary:hover {
  background: #f0f0f0;
}

.loading {
  color: #666;
  padding: 1rem;
  font-style: italic;
}

.error {
  color: red;
  padding: 0.75rem 1rem;
  background: #ffe0e0;
  border: 1px solid #ff6b6b;
  border-radius: 4px;
  margin: 0.75rem 0;
}

@media (max-width: 900px) {
  .layout {
    flex-direction: column;
  }

  .day-panel {
    width: 100%;
  }
}

.outfit-image-wrapper {
  margin-bottom: 0.75rem;
  border-radius: var(--md-shape-medium, 8px);
  overflow: hidden;
  background: var(--md-surface-container-high, #e9ecef);
}

.outfit-image-button {
  position: relative;
  display: flex;
  justify-content: center;
  align-items: center;
  width: 100%;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
  min-height: 180px;
  max-height: 360px;
}

.outfit-card-img {
  width: auto;
  height: auto;
  max-width: 100%;
  max-height: 360px;
  object-fit: contain;
  display: block;
  transition: transform 0.2s ease;
}

.outfit-image-button:hover .outfit-card-img {
  transform: scale(1.02);
}

.outfit-image-button:hover .expand-badge {
  opacity: 1;
  background: rgba(0, 0, 0, 0.9);
}

.tryon-section {
  margin: 1rem 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.tryon-controls {
  display: flex;
  align-items: center;
}

.tryon-error-block {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.missing-settings-cta {
  display: flex;
  justify-content: flex-start;
}

.tryon-preview-box {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0.75rem;
  border: 1px solid var(--md-outline-variant, #e0e0e0);
  border-radius: var(--md-shape-medium, 8px);
  background: var(--md-surface-container, #f8f9fa);
}

.tryon-preview-wrapper {
  position: relative;
  display: flex;
  justify-content: center;
  align-items: center;
  background: var(--md-surface-container-high, #e9ecef);
  border-radius: var(--md-shape-medium, 8px);
  overflow: hidden;
  cursor: pointer;
  min-height: 240px;
  max-height: 480px;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
}

.tryon-preview-thumb {
  width: auto;
  height: auto;
  max-width: 100%;
  max-height: 480px;
  object-fit: contain;
  display: block;
  transition: transform 0.2s ease;
}

.tryon-preview-wrapper:hover .tryon-preview-thumb {
  transform: scale(1.02);
}

.expand-badge {
  position: absolute;
  bottom: 8px;
  right: 8px;
  background: rgba(0, 0, 0, 0.7);
  color: #fff;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 0.75rem;
  display: flex;
  align-items: center;
  gap: 4px;
  opacity: 0.85;
  transition: opacity 0.2s ease, background 0.2s ease;
  pointer-events: none;
}

.tryon-preview-wrapper:hover .expand-badge {
  opacity: 1;
  background: rgba(0, 0, 0, 0.9);
}

.tryon-preview-actions {
  display: flex;
  gap: 0.5rem;
  justify-content: flex-end;
  align-items: center;
  flex-wrap: wrap;
}

/* Lightbox Modal */
.lightbox-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: rgba(0, 0, 0, 0.85);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
}

.lightbox-dialog {
  display: flex;
  flex-direction: column;
  max-width: 95vw;
  max-height: 95vh;
  background: var(--md-surface, #ffffff);
  border-radius: var(--md-shape-large, 16px);
  overflow: hidden;
  box-shadow: 0 16px 32px rgba(0, 0, 0, 0.4);
}

.lightbox-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.75rem 1rem;
  background: var(--md-surface, #ffffff);
  border-bottom: 1px solid var(--md-outline-variant, #e0e0e0);
}

.lightbox-title {
  font-weight: 600;
  font-size: 1.1rem;
  color: var(--md-on-surface, #212121);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lightbox-header-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.lightbox-body {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #121212;
  padding: 0.75rem;
}

.lightbox-img {
  max-width: 90vw;
  max-height: 80vh;
  width: auto;
  height: auto;
  object-fit: contain;
  border-radius: 4px;
  display: block;
}

.lightbox-fade-enter-active,
.lightbox-fade-leave-active {
  transition: opacity 0.2s ease;
}

.lightbox-fade-enter-from,
.lightbox-fade-leave-to {
  opacity: 0;
}

/* First-run onboarding card */
.onboard-card {
  border: 1px solid #ddd;
  border-radius: 8px;
  background: var(--md-surface-container-lowest, #fff);
  padding: 1rem 1.25rem;
  margin-bottom: 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.onboard-title {
  font-size: 1.1rem;
}

.onboard-copy {
  color: #666;
  font-size: 0.9rem;
}

.onboard-steps {
  margin: 0;
  padding-left: 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  font-size: 0.9rem;
  color: #444;
}

.onboard-actions {
  align-self: flex-end;
}

/* Inline text button (cross-links inside sentences) */
.link-btn {
  appearance: none;
  border: none;
  background: none;
  padding: 0;
  cursor: pointer;
  font: inherit;
  color: var(--md-primary, #6750a4);
  text-decoration: underline;
}

/* Picker recency hint */
.picker-last-worn {
  font-size: 0.68rem;
  color: #999;
  white-space: nowrap;
}

/* Try-on generation progress (indeterminate) */
.tryon-progress {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.tryon-progress__bar {
  position: relative;
  flex: 1;
  max-width: 260px;
  height: 4px;
  border-radius: 9999px;
  background: #e3f2fd;
  overflow: hidden;
}

.tryon-progress__fill {
  position: absolute;
  top: 0;
  bottom: 0;
  left: -40%;
  width: 40%;
  border-radius: inherit;
  background: #007bff;
  animation: tryon-progress-slide 1.2s ease-in-out infinite;
}

.tryon-progress__label {
  font-size: 0.8rem;
  color: #666;
}

@keyframes tryon-progress-slide {
  from {
    left: -40%;
  }
  to {
    left: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .tryon-progress__fill {
    animation: none;
    left: 0;
    width: 100%;
  }
}

.cap-notice {
  margin-bottom: 1rem;
  padding: 0.5rem 0.9rem;
  font-size: var(--md-body-small-size, 0.8rem);
  color: var(--md-on-secondary-container, #444);
  background: var(--md-secondary-container, #eee);
  border-radius: var(--md-shape-small, 6px);
}

</style>
