<template>
  <div class="outfits-view">
    <div v-if="loading" class="loading">Loading outfits...</div>
    <div v-else-if="loadError" class="error-wrapper">
      <ErrorDisplay :message="loadError" type="network" can-retry @retry="loadData" />
    </div>

    <div v-else class="layout">
      <section class="calendar-section">
        <div class="calendar-header">
          <button class="nav-btn" @click="prevMonth" aria-label="Previous month">‹</button>
          <h2 class="month-label">{{ monthLabel }}</h2>
          <button class="nav-btn" @click="nextMonth" aria-label="Next month">›</button>
          <button class="today-btn" @click="goToday">Today</button>
        </div>

        <div class="calendar-grid">
          <div v-for="day in WEEKDAYS" :key="day" class="weekday">{{ day }}</div>
          <div
            v-for="cell in cells"
            :key="cell.key"
            class="day-cell"
            :class="{
              other: cell.otherMonth,
              today: cell.isToday,
              selected: cell.key === selectedDate
            }"
            @click="selectDay(cell.key)"
          >
            <span class="day-number">{{ cell.day }}</span>
            <div class="outfit-chips">
              <div
                v-for="outfit in cell.outfits.slice(0, 3)"
                :key="outfit.id"
                class="outfit-chip"
                :title="chipTitle(outfit)"
              >
                <img v-if="firstItemImage(outfit, 28)" :src="firstItemImage(outfit, 28)!" class="chip-thumb" />
                <span class="chip-label">{{ chipLabel(outfit) }}</span>
              </div>
              <div v-if="cell.outfits.length > 3" class="more-chip">
                +{{ cell.outfits.length - 3 }} more
              </div>
            </div>
          </div>
        </div>
      </section>

      <aside class="day-panel">
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

                <span v-for="item in outfit.items" :key="item.id" class="item-tag">
                  {{ item.name }}
                </span>
                <span v-if="outfit.items.length === 0" class="item-tag empty">no items</span>
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
                  <span v-if="!item.is_active" class="picker-inactive-tag">Inactive</span>
                </label>
                <div v-if="filteredPickerItems.length === 0" class="picker-empty">
                  {{ items.length === 0 ? 'No clothing items yet' : 'No items match your search' }}
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
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { api, imageUrl } from '../api/client';
import { sanitizeText, OUTFIT_NOTES_MAX } from '../lib/sanitize';
import { ErrorDisplay, Toast, MdButton, MdIconButton, MdIcon } from '../ui';
import { classifyError } from '../lib/errors';
import { formatLocalDateKey, parseOutfitDateKey, formatSelectedDateLabel } from '../lib/dates';
import { useRouter } from '../lib/router';
import type { ClothingCategory, ClothingItem, Outfit } from '../api/types';

const router = useRouter();

const isGeneratingPreview = ref(false);
const generatedImagePath = ref<string | null>(null);
const generationError = ref<unknown | null>(null);
const expandedImagePath = ref<string | null>(null);
const expandedImageTitle = ref<string>('Outfit Preview');

function openExpandedPreview(path: string | null, title = 'Outfit Preview') {
  if (!path) return;
  expandedImagePath.value = path;
  expandedImageTitle.value = title;
}

function closeExpandedPreview() {
  expandedImagePath.value = null;
}

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

async function handleGeneratePreview() {
  if (selectedItems.value.length === 0 || isGeneratingPreview.value) return;
  isGeneratingPreview.value = true;
  generationError.value = null;
  try {
    const res = await api.generateOutfitPreview(selectedItems.value);
    generatedImagePath.value = res.image_path;
  } catch (err) {
    generationError.value = err;
  } finally {
    isGeneratingPreview.value = false;
  }
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

const loading = ref(true);
const loadError = ref<string | null>(null);
const actionError = ref<string | null>(null);
const outfits = ref<Outfit[]>([]);
const items = ref<ClothingItem[]>([]);
const categories = ref<ClothingCategory[]>([]);

const pickerSearch = ref('');
const pickerCategory = ref<number | null>(null);

function getItemName(id: string): string {
  return items.value.find(i => i.id === id)?.name ?? 'Item';
}

function removeItemFromSelection(id: string) {
  selectedItems.value = selectedItems.value.filter(itemId => itemId !== id);
}

const filteredPickerItems = computed(() => {
  const q = pickerSearch.value.trim().toLowerCase();
  const cat = pickerCategory.value;

  return items.value.filter(item => {
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
});

const confirmingDeleteId = ref<string | null>(null);
const deletingId = ref<string | null>(null);
const showToast = ref(false);
const toastMessage = ref('');

const now = new Date();
const viewYear = ref(now.getFullYear());
const viewMonth = ref(now.getMonth());

const selectedDate = ref<string | null>(null);
const notes = ref('');
const selectedItems = ref<string[]>([]);
const saving = ref(false);
const formError = ref<string | null>(null);

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

const dayOutfits = computed<Outfit[]>(() => {
  if (!selectedDate.value) return [];
  return outfitsByDate.value.get(selectedDate.value) ?? [];
});

const selectedDateLabel = computed(() => {
  return formatSelectedDateLabel(selectedDate.value);
});

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

function goToday() {
  const today = new Date();
  viewYear.value = today.getFullYear();
  viewMonth.value = today.getMonth();
  selectedDate.value = dateKey(today);
}

function selectDay(key: string) {
  selectedDate.value = key;
  formError.value = null;
  generatedImagePath.value = null;
  generationError.value = null;
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
  for (const item of outfit.items) {
    const url = imageUrl(item.image_path, { width });
    if (url) return url;
  }
  return null;
}

function chipLabel(outfit: Outfit): string {
  if (outfit.notes) return outfit.notes;
  if (outfit.items.length > 0) return outfit.items[0].name;
  return 'Outfit';
}

function chipTitle(outfit: Outfit): string {
  const itemNames = outfit.items.map(i => i.name).join(', ');
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
    outfits.value.push(created);
    outfits.value.sort((a, b) => outfitKey(a).localeCompare(outfitKey(b)));
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
    outfits.value = outfits.value.filter(o => o.id !== outfit.id);
    confirmingDeleteId.value = null;
    toastMessage.value = 'Outfit deleted.';
    showToast.value = true;
  } catch (err) {
    actionError.value = classifyError(err).message;
  } finally {
    deletingId.value = null;
  }
}

async function loadData() {
  try {
    loading.value = true;
    loadError.value = null;
    const [outfitData, itemData, categoryData] = await Promise.all([
      api.listOutfits(),
      api.getTableRows('clothing-items'),
      api.listCategories()
    ]);
    outfits.value = outfitData;
    items.value = Array.isArray(itemData) ? (itemData as ClothingItem[]) : [];
    categories.value = Array.isArray(categoryData) ? categoryData : [];
  } catch (err) {
    loadError.value = classifyError(err).message;
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown);
  loadData();
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
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

</style>
