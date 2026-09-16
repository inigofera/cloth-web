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
        accept="image/*"
        class="file-input"
        @change="onImageChange"
      />
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
        <select v-model="form.brand_id">
          <option :value="null">—</option>
          <option v-for="b in brands" :key="b.id" :value="b.id">
            {{ b.name }}
          </option>
          <option :value="NEW_OPTION">+ Add new…</option>
        </select>
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
      <input v-model="form.origin" type="text" placeholder="e.g. Spain" />
    </label>

    <div class="field-row">
      <label class="field">
        <span class="field-label">Laundry impact</span>
        <input v-model="form.laundry_impact" type="text" placeholder="e.g. Low" />
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
  NEW_OPTION,
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
const addError = ref<string | null>(null);

const newColor = reactive({ name: '', hex: '' });
const newCategory = reactive({ name: '' });
const newSubcategory = reactive({ name: '' });
const newBrand = reactive({ name: '' });
const addingColor = ref(false);
const addingCategory = ref(false);
const addingSubcategory = ref(false);
const addingBrand = ref(false);

function onImageChange(event: Event) {
  const target = event.target as HTMLInputElement;
  imageFile.value = target.files?.[0] ?? null;
}

const subcategoriesForSelection = computed(() => {
  if (typeof props.form.category_id !== 'number') return [];
  return props.subcategories.filter(s => s.category_id === props.form.category_id);
});

async function addColor() {
  const name = newColor.name.trim().toLowerCase();
  if (!name) return;
  addingColor.value = true;
  addError.value = null;
  try {
    const existing = props.colors.find(c => c.id === name);
    if (existing) {
      props.form.color_id = existing.id;
    } else {
      const created = await api.createColor({ id: name, hex_value: newColor.hex.trim() || null });
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
  const name = newCategory.name.trim();
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
  const name = newSubcategory.name.trim();
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
  const name = newBrand.name.trim();
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
  imageFile.value = null;
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

.file-input {
  font-size: 0.85rem;
  width: 100%;
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
