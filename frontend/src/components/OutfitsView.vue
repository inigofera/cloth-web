<template>
  <div class="outfits-view">
    <div v-if="loading" class="loading">Loading outfits...</div>
    <div v-else-if="loadError" class="error">{{ loadError }}</div>

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
                <img v-if="firstItemImage(outfit)" :src="firstItemImage(outfit)!" class="chip-thumb" />
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
        <template v-if="selectedDate">
          <h3 class="day-title">{{ selectedDateLabel }}</h3>

          <div v-if="dayOutfits.length > 0" class="day-outfits">
            <div v-for="outfit in dayOutfits" :key="outfit.id" class="outfit-card">
              <div class="outfit-items">
                <span v-for="item in outfit.items" :key="item.id" class="item-tag">
                  {{ item.name }}
                </span>
                <span v-if="outfit.items.length === 0" class="item-tag empty">no items</span>
              </div>
              <p v-if="outfit.notes" class="outfit-notes">{{ outfit.notes }}</p>
              <button class="delete-btn" @click="removeOutfit(outfit)">Delete</button>
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
              <div class="item-picker">
                <label
                  v-for="item in items"
                  :key="item.id"
                  class="item-option"
                  :class="{ checked: selectedItems.includes(item.id) }"
                >
                  <input
                    v-model="selectedItems"
                    type="checkbox"
                    :value="item.id"
                  />
                  <img v-if="imageUrl(item.image_path)" :src="imageUrl(item.image_path)" class="thumb" />
                  <span class="item-name">{{ item.name }}</span>
                </label>
                <div v-if="items.length === 0" class="picker-empty">
                  No clothing items yet
                </div>
              </div>
            </div>

            <div v-if="formError" class="error">{{ formError }}</div>

            <div class="form-actions">
              <button type="submit" class="primary" :disabled="saving">
                {{ saving ? 'Saving…' : 'Add outfit' }}
              </button>
              <button type="button" class="secondary" @click="closeForm">Cancel</button>
            </div>
          </form>
        </template>

        <div v-else class="hint">Select a day on the calendar to add an outfit.</div>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { api, imageUrl } from '../api/client';
import type { ClothingItem, Outfit } from '../api/types';

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
const outfits = ref<Outfit[]>([]);
const items = ref<ClothingItem[]>([]);

const now = new Date();
const viewYear = ref(now.getFullYear());
const viewMonth = ref(now.getMonth());

const selectedDate = ref<string | null>(null);
const notes = ref('');
const selectedItems = ref<string[]>([]);
const saving = ref(false);
const formError = ref<string | null>(null);

function pad(n: number): string {
  return n.toString().padStart(2, '0');
}

function dateKey(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function outfitKey(o: Outfit): string {
  return new Date(o.date).toISOString().slice(0, 10);
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
  if (!selectedDate.value) return '';
  const [y, m, d] = selectedDate.value.split('-').map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
    year: 'numeric'
  });
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
}

function closeForm() {
  selectedDate.value = null;
  notes.value = '';
  selectedItems.value = [];
  formError.value = null;
}

function firstItemImage(outfit: Outfit): string | null {
  for (const item of outfit.items) {
    const url = imageUrl(item.image_path);
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
      notes: notes.value.trim() ? notes.value.trim() : null,
      item_ids: selectedItems.value
    });
    outfits.value.push(created);
    outfits.value.sort((a, b) => a.date.localeCompare(b.date));
    notes.value = '';
    selectedItems.value = [];
  } catch (err) {
    formError.value = String(err);
  } finally {
    saving.value = false;
  }
}

async function removeOutfit(outfit: Outfit) {
  try {
    await api.deleteTableRow('outfits', outfit.id);
    outfits.value = outfits.value.filter(o => o.id !== outfit.id);
  } catch (err) {
    loadError.value = String(err);
  }
}

onMounted(async () => {
  try {
    loading.value = true;
    const [outfitData, itemData] = await Promise.all([
      api.listOutfits(),
      api.getTableRows('clothing-items')
    ]);
    outfits.value = outfitData;
    items.value = Array.isArray(itemData) ? (itemData as ClothingItem[]) : [];
  } catch (err) {
    loadError.value = String(err);
  } finally {
    loading.value = false;
  }
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
</style>
