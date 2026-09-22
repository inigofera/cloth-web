<template>
  <form class="item-form" @submit.prevent="emit('submit')">
    <h4>{{ title }}</h4>

    <label class="field">
      <span class="field-label">Name *</span>
      <input v-model="form.name" type="text" required placeholder="e.g. Blue Denim Jacket" />
    </label>

    <label class="field">
      <span class="field-label">Image</span>
      <input
        :value="imageFile?.name ?? ''"
        type="file"
        accept="image/jpeg,image/png,image/webp,image/gif"
        class="file-input"
        @change="onImageChange"
      />
      <span v-if="imageError" class="field-error">{{ imageError }}</span>
      <span v-else-if="imageFile" class="field-hint">
        {{ imageFile.name }} ({{ formatSize(imageFile.size) }})
      </span>
    </label>

    <div class="field-row">
      <div class="field">
        <span class="field-label">Category</span>
        <select v-model="form.category_id">
          <option :value="null">—</option>
          <option v-for="c in categories" :key="c.id" :value="c.id">
            {{ c.name }}
          </option>
          <option :value="NEW_OPTION">+ Add new…</option>
        </select>
        <div v-if="form.category_id === NEW_OPTION" class="inline-add">
          <input
            v-model="newCategory.name"
            type="text"
            placeholder="e.g. Outerwear"
            @keyup.enter="addCategory"
          />
          <button type="button" class="add-btn" :disabled="addingCategory" @click="addCategory">
            Add
          </button>
        </div>
      </div>

      <div class="field">
        <span class="field-label">Subcategory</span>
        <select
          v-model="form.subcategory_id"
          :disabled="typeof form.category_id !== 'number'"
        >
          <option :value="null">—</option>
          <option v-for="s in subcategoriesForSelection" :key="s.id" :value="s.id">
            {{ s.name }}
          </option>
          <option :value="NEW_OPTION">+ Add new…</option>
        </select>
        <div v-if="form.subcategory_id === NEW_OPTION" class="inline-add">
          <template v-if="typeof form.category_id === 'number'">
            <input
              v-model="newSubcategory.name"
              type="text"
              placeholder="e.g. Hoodies"
              @keyup.enter="addSubcategory"
            />
            <button
              type="button"
              class="add-btn"
              :disabled="addingSubcategory"
              @click="addSubcategory"
            >
              Add
            </button>
          </template>
          <span v-else class="inline-hint">Select a category first</span>
        </div>
      </div>
    </div>

    <div class="field-row">
      <div class="field">
        <span class="field-label">Color</span>
        <select v-model="form.color_id">
          <option :value="null">—</option>
          <option v-for="c in colors" :key="c.id" :value="c.id">
            {{ c.id }}
          </option>
          <option :value="NEW_OPTION">+ Add new…</option>
        </select>
        <div v-if="form.color_id === NEW_OPTION" class="inline-add">
          <input
            v-model="newColor.name"
            type="text"
            placeholder="e.g. blue"
            @keyup.enter="addColor"
          />
          <input
            v-model="newColor.hex"
            type="text"
            placeholder="#RRGGBB"
            class="hex-input"
            @keyup.enter="addColor"
          />
          <button type="button" class="add-btn" :disabled="addingColor" @click="addColor">
            Add
          </button>
        </div>
      </div>

      <div class="field">
        <span class="field-label">Brand</span>
        <div class="brand-select">
          <button
            type="button"
            class="brand-select-trigger"
            :aria-expanded="brandDropdownOpen"
            aria-haspopup="listbox"
            @click="brandDropdownOpen = !brandDropdownOpen"
          >
            <span>{{ selectedBrandName || '—' }}</span>
            <span aria-hidden="true">▾</span>
          </button>
          <div v-if="brandDropdownOpen" class="brand-dropdown" role="listbox">
            <input
              v-model="brandSearch"
              type="search"
              class="brand-search"
              placeholder="Search brands…"
              aria-label="Search brands"
              autofocus
              @keydown.esc="brandDropdownOpen = false"
            />
            <button type="button" class="brand-option" @click="selectBrand(null)">—</button>
            <button
              v-for="b in filteredBrands"
              :key="b.id"
              type="button"
              class="brand-option"
              :class="{ selected: form.brand_id === b.id }"
              @click="selectBrand(b.id)"
            >
              {{ b.name }}
            </button>
            <button type="button" class="brand-option add-brand-option" @click="selectBrand(NEW_OPTION)">
              + Add new…
            </button>
            <span v-if="filteredBrands.length === 0" class="brand-empty">No brands found</span>
          </div>
        </div>
        <div v-if="form.brand_id === NEW_OPTION" class="inline-add">
          <input
            v-model="newBrand.name"
            type="text"
            placeholder="e.g. Acme"
            @keyup.enter="addBrand"
          />
          <button type="button" class="add-btn" :disabled="addingBrand" @click="addBrand">
            Add
          </button>
        </div>
      </div>
    </div>

    <div class="field-row">
      <label class="field">
        <span class="field-label">Purchase price</span>
        <input v-model.number="form.purchase_price" type="number" min="0" step="0.01" />
      </label>

      <label class="field">
        <span class="field-label">Owned since</span>
        <input v-model="form.owned_since" type="date" />
      </label>
    </div>

    <label class="field">
      <span class="field-label">Origin</span>
      <select v-model="form.origin">
        <option value="">—</option>
        <option v-for="origin in ORIGIN_OPTIONS" :key="origin" :value="origin">
          {{ origin }}
        </option>
      </select>
    </label>

    <div class="field-row">
      <label class="field">
        <span class="field-label">Laundry impact</span>
        <select v-model="form.laundry_impact">
          <option value="">—</option>
          <option v-for="impact in LAUNDRY_IMPACT_OPTIONS" :key="impact" :value="impact">
            {{ impact }}
          </option>
        </select>
      </label>

      <label class="field">
        <span class="field-label">Wear count</span>
        <input v-model.number="form.wear_count" type="number" min="0" step="1" />
      </label>
    </div>

    <div class="field-row">
      <label class="field checkbox-field">
        <input v-model="form.repairable" type="checkbox" />
        <span class="field-label checkbox-label">Repairable</span>
      </label>

      <label class="field checkbox-field">
        <input v-model="form.is_active" type="checkbox" />
        <span class="field-label checkbox-label">Active</span>
      </label>
    </div>

    <label class="field">
      <span class="field-label">Notes</span>
      <textarea
        v-model="form.notes"
        rows="3"
        placeholder="optional"
      ></textarea>
    </label>

    <div v-if="error || addError" class="error">{{ error || addError }}</div>

    <div class="form-actions">
      <button type="submit" class="primary" :disabled="saving">
        {{ saving ? 'Saving…' : submitLabel }}
      </button>
      <button type="button" class="secondary" @click="emit('secondary')">
        {{ secondaryLabel }}
      </button>
    </div>
  </form>
</template>

<script setup lang="ts">
import { ref, reactive, computed } from 'vue';
import { api } from '../api/client';
import {
  validateImageFile,
  sanitizeText,
  isValidHexColor,
  NAME_MAX,
  COLOR_NAME_MAX,
} from '../lib/sanitize';
import {
  NEW_OPTION,
  ORIGIN_OPTIONS,
  LAUNDRY_IMPACT_OPTIONS,
  type Brand,
  type ClothingCategory,
  type ClothingItemFormState,
  type ClothingSubcategory,
  type Color
} from '../api/types';

interface Props {
  form: ClothingItemFormState;
  colors: Color[];
  categories: ClothingCategory[];
  subcategories: ClothingSubcategory[];
  brands: Brand[];
  saving?: boolean;
  title?: string;
  submitLabel?: string;
  secondaryLabel?: string;
  error?: string | null;
}

const props = withDefaults(defineProps<Props>(), {
  saving: false,
  title: 'Clothing item',
  submitLabel: 'Save',
  secondaryLabel: 'Cancel',
  error: null
});

const emit = defineEmits<{
  submit: [];
  secondary: [];
  'color-added': [color: Color];
  'category-added': [category: ClothingCategory];
  'subcategory-added': [subcategory: ClothingSubcategory];
  'brand-added': [brand: Brand];
}>();

const imageFile = ref<File | null>(null);
const imageError = ref<string | null>(null);
const addError = ref<string | null>(null);

const newColor = reactive({ name: '', hex: '' });
const newCategory = reactive({ name: '' });
const newSubcategory = reactive({ name: '' });
const newBrand = reactive({ name: '' });
const addingColor = ref(false);
const addingCategory = ref(false);
const addingSubcategory = ref(false);
const addingBrand = ref(false);
const brandSearch = ref('');
const brandDropdownOpen = ref(false);

async function onImageChange(event: Event) {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0] ?? null;
  imageError.value = null;
  if (!file) {
    imageFile.value = null;
    return;
  }
  const result = await validateImageFile(file);
  if (result.ok) {
    imageFile.value = result.file;
  } else {
    imageFile.value = null;
    imageError.value = result.error ?? 'Invalid image.';
    target.value = '';
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

const subcategoriesForSelection = computed(() => {
  if (typeof props.form.category_id !== 'number') return [];
  return props.subcategories.filter(s => s.category_id === props.form.category_id);
});

const filteredBrands = computed(() => {
  const query = brandSearch.value.trim().toLowerCase();
  if (!query) return props.brands;

  return props.brands.filter(brand => brand.name.toLowerCase().includes(query));
});

const selectedBrandName = computed(() => {
  if (!props.form.brand_id || props.form.brand_id === NEW_OPTION) return '';
  return props.brands.find(brand => brand.id === props.form.brand_id)?.name ?? '';
});

function selectBrand(brandId: string | null | typeof NEW_OPTION) {
  props.form.brand_id = brandId;
  brandSearch.value = '';
  brandDropdownOpen.value = false;
}

async function addColor() {
  const name = sanitizeText(newColor.name, { max: COLOR_NAME_MAX }).toLowerCase();
  if (!name) return;
  const hex = newColor.hex.trim();
  if (hex && !isValidHexColor(hex)) {
    addError.value = 'Color hex must be in #RRGGBB format.';
    return;
  }
  addingColor.value = true;
  addError.value = null;
  try {
    const existing = props.colors.find(c => c.id === name);
    if (existing) {
      props.form.color_id = existing.id;
    } else {
      const created = await api.createColor({ id: name, hex_value: hex || null });
      emit('color-added', created);
      props.form.color_id = created.id;
    }
    newColor.name = '';
    newColor.hex = '';
  } catch (err) {
    addError.value = String(err);
  } finally {
    addingColor.value = false;
  }
}

async function addCategory() {
  const name = sanitizeText(newCategory.name, { max: NAME_MAX });
  if (!name) return;
  addingCategory.value = true;
  addError.value = null;
  try {
    const existing = props.categories.find(c => c.name.toLowerCase() === name.toLowerCase());
    if (existing) {
      props.form.category_id = existing.id;
    } else {
      const created = await api.createCategory({ name });
      emit('category-added', created);
      props.form.category_id = created.id;
    }
    newCategory.name = '';
  } catch (err) {
    addError.value = String(err);
  } finally {
    addingCategory.value = false;
  }
}

async function addSubcategory() {
  if (typeof props.form.category_id !== 'number') return;
  const name = sanitizeText(newSubcategory.name, { max: NAME_MAX });
  if (!name) return;
  addingSubcategory.value = true;
  addError.value = null;
  try {
    const existing = props.subcategories.find(
      s => s.category_id === props.form.category_id && s.name.toLowerCase() === name.toLowerCase()
    );
    if (existing) {
      props.form.subcategory_id = existing.id;
    } else {
      const created = await api.createSubcategory({ name, category_id: props.form.category_id });
      emit('subcategory-added', created);
      props.form.subcategory_id = created.id;
    }
    newSubcategory.name = '';
  } catch (err) {
    addError.value = String(err);
  } finally {
    addingSubcategory.value = false;
  }
}

async function addBrand() {
  const name = sanitizeText(newBrand.name, { max: NAME_MAX });
  if (!name) return;
  addingBrand.value = true;
  addError.value = null;
  try {
    const existing = props.brands.find(b => b.name.toLowerCase() === name.toLowerCase());
    if (existing) {
      props.form.brand_id = existing.id;
    } else {
      const created = (await api.insertTableRow('brands', { name })) as Brand | null;
      if (created) emit('brand-added', created);
      props.form.brand_id = created?.id ?? null;
    }
    newBrand.name = '';
  } catch (err) {
    addError.value = String(err);
  } finally {
    addingBrand.value = false;
  }
}

function clearTransient() {
  newColor.name = '';
  newColor.hex = '';
  newCategory.name = '';
  newSubcategory.name = '';
  newBrand.name = '';
  brandSearch.value = '';
  imageFile.value = null;
  imageError.value = null;
}

defineExpose({ imageFile, clearTransient });
</script>

<style scoped>
.item-form {
  border-top: 2px solid #007bff;
  padding-top: 1rem;
}

.item-form h4 {
  margin: 0 0 0.75rem;
  color: #333;
}

.field {
  display: block;
  margin-bottom: 0.9rem;
  flex: 1;
  min-width: 0;
}

.field-row {
  display: flex;
  gap: 0.75rem;
}

.field-row .field {
  margin-bottom: 0.9rem;
}

.field-label {
  display: block;
  font-size: 0.85rem;
  font-weight: 600;
  color: #555;
  margin-bottom: 0.3rem;
}

.field input[type='text'],
.field input[type='search'],
.field input[type='number'],
.field input[type='date'],
.field select,
.field textarea {
  width: 100%;
  box-sizing: border-box;
  padding: 0.45rem 0.6rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  font-size: 0.9rem;
  background: #fff;
  font-family: inherit;
}

.field textarea {
  resize: vertical;
}

.field select:disabled {
  background: #f0f0f0;
  color: #999;
}

.brand-search {
  margin-bottom: 0.4rem;
}

.brand-select {
  position: relative;
}

.brand-select-trigger {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  box-sizing: border-box;
  padding: 0.45rem 0.6rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fff;
  color: #333;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.brand-select-trigger:hover,
.brand-select-trigger:focus {
  border-color: #007bff;
}

.brand-dropdown {
  position: absolute;
  z-index: 10;
  top: calc(100% + 0.2rem);
  left: 0;
  right: 0;
  max-height: 16rem;
  overflow-y: auto;
  padding: 0.4rem;
  border: 1px solid #bbb;
  border-radius: 4px;
  background: #fff;
  box-shadow: 0 3px 8px rgb(0 0 0 / 15%);
}

.brand-option {
  display: block;
  width: 100%;
  padding: 0.4rem 0.5rem;
  border: 0;
  background: #fff;
  color: #333;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.brand-option:hover,
.brand-option.selected {
  background: #e3f2fd;
}

.add-brand-option {
  border-top: 1px solid #eee;
  margin-top: 0.25rem;
  color: #007bff;
}

.brand-empty {
  display: block;
  padding: 0.4rem 0.5rem;
  color: #777;
  font-size: 0.85rem;
}

.file-input {
  font-size: 0.85rem;
  width: 100%;
}

.field-error {
  display: block;
  margin-top: 0.25rem;
  font-size: 0.8rem;
  color: #c0392b;
}

.field-hint {
  display: block;
  margin-top: 0.25rem;
  font-size: 0.8rem;
  color: #666;
}

.inline-add {
  display: flex;
  gap: 0.4rem;
  margin-top: 0.4rem;
}

.inline-add input[type='text'] {
  flex: 1;
  min-width: 0;
  padding: 0.35rem 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  font-size: 0.85rem;
}

.inline-add .hex-input {
  flex: 0 0 82px;
}

.add-btn {
  padding: 0.35rem 0.7rem;
  border: 1px solid #007bff;
  border-radius: 4px;
  background: #fff;
  color: #007bff;
  font-size: 0.85rem;
  cursor: pointer;
  white-space: nowrap;
}

.add-btn:hover {
  background: #e3f2fd;
}

.add-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.inline-hint {
  font-size: 0.8rem;
  color: #999;
  font-style: italic;
  align-self: center;
}

.checkbox-field {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.9rem;
}

.checkbox-field input {
  accent-color: #007bff;
  width: 16px;
  height: 16px;
}

.checkbox-label {
  margin-bottom: 0;
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

.error {
  color: red;
  padding: 0.75rem 1rem;
  background: #ffe0e0;
  border: 1px solid #ff6b6b;
  border-radius: 4px;
  margin: 0.75rem 0;
}
</style>
