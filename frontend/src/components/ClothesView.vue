<template>
  <div class="clothes-view">
    <div v-if="loading" class="loading">Loading clothes...</div>
    <div v-else-if="loadError" class="error">{{ loadError }}</div>

    <div v-else-if="selectedItem" class="detail-page">
      <button class="back-btn" @click="selectedItemId = null">← Back to clothes</button>

      <div class="detail-layout">
        <div class="detail-media">
          <img
            v-if="imageUrl(selectedItem.image_path)"
            :src="imageUrl(selectedItem.image_path)"
            :alt="selectedItem.name"
          />
          <div v-else class="detail-media-placeholder">No image</div>
        </div>

        <div class="detail-body">
          <div class="detail-header">
            <h2 class="detail-title">{{ selectedItem.name }}</h2>
            <span v-if="!selectedItem.is_active" class="inactive-badge">Inactive</span>
            <div class="detail-header-actions">
              <MdButton
                v-if="!editMode && !confirmingDelete"
                variant="outlined"
                @click="startEdit"
              >
                Edit
              </MdButton>
              <MdButton
                v-if="!editMode"
                variant="error"
                @click="confirmingDelete = true"
              >
                Delete item
              </MdButton>
            </div>
          </div>

          <template v-if="editMode">
            <ClothingItemForm
              ref="editFormRef"
              :form="editForm"
              :colors="colors"
              :categories="categories"
              :subcategories="subcategories"
              :brands="brands"
              :saving="editSaving"
              title="Edit item"
              submit-label="Save changes"
              secondary-label="Cancel"
              :error="formError"
              @submit="saveEdit"
              @secondary="cancelEdit"
              @color-added="pushColor"
              @category-added="pushCategory"
              @subcategory-added="pushSubcategory"
              @brand-added="pushBrand"
            />
          </template>

          <template v-else>
          <dl class="detail-rows">
            <div class="row">
              <dt>Category</dt>
              <dd>{{ categoryName(selectedItem.category_id) ?? '—' }}</dd>
            </div>
            <div class="row">
              <dt>Subcategory</dt>
              <dd>{{ subcategoryName(selectedItem) ?? '—' }}</dd>
            </div>
            <div class="row">
              <dt>Color</dt>
              <dd>
                <span v-if="colorOf(selectedItem)" class="color-value">
                  <span
                    class="color-swatch"
                    :style="{ background: colorOf(selectedItem)!.hex_value || '#ccc' }"
                  ></span>
                  {{ selectedItem.color_id }}
                </span>
                <template v-else>—</template>
              </dd>
            </div>
            <div class="row">
              <dt>Brand</dt>
              <dd>{{ brandName(selectedItem.brand_id) ?? '—' }}</dd>
            </div>
            <div class="row">
              <dt>Purchase price</dt>
              <dd>{{ selectedItem.purchase_price != null ? selectedItem.purchase_price.toFixed(2) : '—' }}</dd>
            </div>
            <div class="row">
              <dt>Owned since</dt>
              <dd>{{ selectedItem.owned_since ?? '—' }}</dd>
            </div>
            <div class="row">
              <dt>Origin</dt>
              <dd>{{ selectedItem.origin ?? '—' }}</dd>
            </div>
            <div class="row">
              <dt>Laundry impact</dt>
              <dd>{{ selectedItem.laundry_impact ?? '—' }}</dd>
            </div>
            <div class="row">
              <dt>Repairable</dt>
              <dd>{{ selectedItem.repairable == null ? '—' : (selectedItem.repairable ? 'Yes' : 'No') }}</dd>
            </div>
            <div class="row">
              <dt>Active</dt>
              <dd>{{ selectedItem.is_active ? 'Yes' : 'No' }}</dd>
            </div>
            <div class="row">
              <dt>Wear count</dt>
              <dd>{{ selectedItem.wear_count ?? 0 }}×</dd>
            </div>
          </dl>

          <p v-if="selectedItem.notes" class="detail-notes">{{ selectedItem.notes }}</p>

          <div class="detail-timestamps">
            <span>Created {{ formatDate(selectedItem.created_at) }}</span>
            <span>Updated {{ formatDate(selectedItem.updated_at) }}</span>
            <span class="detail-id">{{ selectedItem.id }}</span>
          </div>
          </template>

          <div v-if="confirmingDelete" class="confirm-panel">
            <p class="confirm-text">
              Delete <strong>{{ selectedItem.name }}</strong>? This permanently removes the
              item and takes it out of all outfits that use it. If you might need it again,
              you can deactivate it instead — it stays in your wardrobe, just marked as
              inactive.
            </p>
            <div class="confirm-actions">
              <MdButton variant="error" :disabled="deleting" @click="removeItemAndBack">
                {{ deleting ? 'Deleting…' : 'Delete item' }}
              </MdButton>
              <MdButton variant="outlined" :disabled="deactivating" @click="deactivateSelected">
                {{ deactivating ? 'Deactivating…' : 'Deactivate instead' }}
              </MdButton>
              <MdButton
                variant="text"
                :disabled="deleting || deactivating"
                @click="confirmingDelete = false"
              >
                Cancel
              </MdButton>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-else class="layout">
      <section class="list-section">
        <h2 class="section-title">
          My clothes <span class="count">({{ items.length }})</span>
        </h2>

        <div v-if="items.length === 0" class="empty">
          No clothing items yet. Add your first one with the form.
        </div>

        <div v-else class="items-grid">
          <div
            v-for="item in items"
            :key="item.id"
            class="item-card clickable"
            @click="openItem(item)"
          >
            <div class="item-card__media">
              <img
                v-if="imageUrl(item.image_path)"
                :src="imageUrl(item.image_path)"
                class="item-image"
              />
              <div v-else class="item-image placeholder">
                <span>No image</span>
              </div>
              <span v-if="!item.is_active" class="inactive-badge">Inactive</span>
            </div>

            <div class="item-card__body">
              <h3 class="item-name">{{ item.name }}</h3>

              <div class="item-meta">
                <span v-if="categoryName(item.category_id)" class="meta-tag">
                  {{ categoryName(item.category_id) }}
                  <template v-if="subcategoryName(item)">
                    › {{ subcategoryName(item) }}
                  </template>
                </span>
                <span v-if="colorOf(item)" class="meta-tag">
                  <span
                    class="color-swatch"
                    :style="{ background: colorOf(item)!.hex_value || '#ccc' }"
                  ></span>
                  {{ colorOf(item)!.id }}
                </span>
                <span v-if="brandName(item.brand_id)" class="meta-tag">
                  {{ brandName(item.brand_id) }}
                </span>
              </div>

              <dl class="item-details">
                <div v-if="item.purchase_price != null" class="detail-row">
                  <dt>Price</dt>
                  <dd>{{ item.purchase_price.toFixed(2) }}</dd>
                </div>
                <div v-if="item.owned_since" class="detail-row">
                  <dt>Owned since</dt>
                  <dd>{{ item.owned_since }}</dd>
                </div>
                <div v-if="item.origin" class="detail-row">
                  <dt>Origin</dt>
                  <dd>{{ item.origin }}</dd>
                </div>
                <div v-if="item.laundry_impact" class="detail-row">
                  <dt>Laundry</dt>
                  <dd>{{ item.laundry_impact }}</dd>
                </div>
                <div v-if="item.repairable != null" class="detail-row">
                  <dt>Repairable</dt>
                  <dd>{{ item.repairable ? 'Yes' : 'No' }}</dd>
                </div>
                <div class="detail-row">
                  <dt>Worn</dt>
                  <dd>{{ item.wear_count ?? 0 }}×</dd>
                </div>
              </dl>

              <p v-if="item.notes" class="item-notes">{{ item.notes }}</p>
            </div>
          </div>
        </div>
      </section>

      <aside class="form-panel">
        <ClothingItemForm
          ref="addFormRef"
          :form="form"
          :colors="colors"
          :categories="categories"
          :subcategories="subcategories"
          :brands="brands"
          :saving="saving"
          title="Add clothing item"
          submit-label="Add item"
          secondary-label="Reset"
          :error="formError"
          @submit="submitItem"
          @secondary="resetForm"
          @color-added="pushColor"
          @category-added="pushCategory"
          @subcategory-added="pushSubcategory"
          @brand-added="pushBrand"
        />
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue';
import { api, imageUrl } from '../api/client';
import { MdButton } from '../ui';
import ClothingItemForm from './ClothingItemForm.vue';
import {
  NEW_OPTION,
  type Brand,
  type ClothingCategory,
  type ClothingItem,
  type ClothingItemFormState,
  type ClothingSubcategory,
  type Color
} from '../api/types';

const loading = ref(true);
const loadError = ref<string | null>(null);
const items = ref<ClothingItem[]>([]);
const selectedItemId = ref<string | null>(null);

const selectedItem = computed(
  () => items.value.find(i => i.id === selectedItemId.value) ?? null
);

const colors = ref<Color[]>([]);
const categories = ref<ClothingCategory[]>([]);
const subcategories = ref<ClothingSubcategory[]>([]);
const brands = ref<Brand[]>([]);

const form = reactive<ClothingItemFormState>({
  name: '',
  category_id: null,
  subcategory_id: null,
  color_id: null,
  brand_id: null,
  purchase_price: null,
  owned_since: '',
  origin: '',
  laundry_impact: '',
  wear_count: null,
  repairable: false,
  is_active: true,
  notes: ''
});
const addFormRef = ref<InstanceType<typeof ClothingItemForm> | null>(null);
const editFormRef = ref<InstanceType<typeof ClothingItemForm> | null>(null);
const saving = ref(false);
const formError = ref<string | null>(null);

const editMode = ref(false);
const editSaving = ref(false);
const editForm = reactive<ClothingItemFormState>({
  name: '',
  category_id: null,
  subcategory_id: null,
  color_id: null,
  brand_id: null,
  purchase_price: null,
  owned_since: '',
  origin: '',
  laundry_impact: '',
  wear_count: null,
  repairable: false,
  is_active: true,
  notes: ''
});

function categoryName(id: number | null): string | null {
  if (id == null) return null;
  return categories.value.find(c => c.id === id)?.name ?? null;
}

function subcategoryName(item: ClothingItem): string | null {
  if (item.subcategory_id == null) return null;
  return subcategories.value.find(s => s.id === item.subcategory_id)?.name ?? null;
}

function colorOf(item: ClothingItem): Color | null {
  if (!item.color_id) return null;
  return colors.value.find(c => c.id === item.color_id) ?? null;
}

function brandName(id: string | null): string | null {
  if (!id) return null;
  return brands.value.find(b => b.id === id)?.name ?? null;
}

function pushColor(color: Color) {
  if (!colors.value.some(c => c.id === color.id)) colors.value.push(color);
}

function pushCategory(category: ClothingCategory) {
  if (!categories.value.some(c => c.id === category.id)) categories.value.push(category);
}

function pushSubcategory(subcategory: ClothingSubcategory) {
  if (!subcategories.value.some(s => s.id === subcategory.id)) subcategories.value.push(subcategory);
}

function pushBrand(brand: Brand) {
  if (!brands.value.some(b => b.id === brand.id)) brands.value.push(brand);
}

function resetForm() {
  form.name = '';
  form.category_id = null;
  form.subcategory_id = null;
  form.color_id = null;
  form.brand_id = null;
  form.purchase_price = null;
  form.owned_since = '';
  form.origin = '';
  form.laundry_impact = '';
  form.wear_count = null;
  form.repairable = false;
  form.is_active = true;
  form.notes = '';
  addFormRef.value?.clearTransient();
  formError.value = null;
}

async function submitItem() {
  saving.value = true;
  formError.value = null;
  try {
    let imagePath: string | null = null;
    const file = addFormRef.value?.imageFile ?? null;
    if (file) {
      imagePath = await api.uploadFile(file);
    }

    const created = await api.createClothingItem({
      name: form.name.trim(),
      category_id: typeof form.category_id === 'number' ? form.category_id : null,
      subcategory_id: typeof form.subcategory_id === 'number' ? form.subcategory_id : null,
      color_id: form.color_id === NEW_OPTION ? null : form.color_id,
      brand_id: form.brand_id === NEW_OPTION ? null : form.brand_id,
      purchase_price: form.purchase_price,
      owned_since: form.owned_since || null,
      origin: form.origin.trim() || null,
      laundry_impact: form.laundry_impact.trim() || null,
      wear_count: form.wear_count,
      repairable: form.repairable,
      is_active: form.is_active,
      notes: form.notes.trim() || null,
      image_path: imagePath
    });

    items.value.unshift(created);
    resetForm();
  } catch (err) {
    formError.value = String(err);
  } finally {
    saving.value = false;
  }
}

function openItem(item: ClothingItem) {
  selectedItemId.value = item.id;
  confirmingDelete.value = false;
  editMode.value = false;
}

function startEdit() {
  const item = selectedItem.value;
  if (!item) return;
  editForm.name = item.name;
  editForm.category_id = item.category_id;
  editForm.subcategory_id = item.subcategory_id;
  editForm.color_id = item.color_id;
  editForm.brand_id = item.brand_id;
  editForm.purchase_price = item.purchase_price;
  editForm.owned_since = item.owned_since ?? '';
  editForm.origin = item.origin ?? '';
  editForm.laundry_impact = item.laundry_impact ?? '';
  editForm.wear_count = item.wear_count;
  editForm.repairable = item.repairable ?? false;
  editForm.is_active = item.is_active;
  editForm.notes = item.notes ?? '';
  formError.value = null;
  editMode.value = true;
}

function cancelEdit() {
  editMode.value = false;
  formError.value = null;
}

async function saveEdit() {
  const item = selectedItem.value;
  if (!item) return;
  editSaving.value = true;
  formError.value = null;
  try {
    let imagePath: string | null = null;
    const file = editFormRef.value?.imageFile ?? null;
    if (file) {
      imagePath = await api.uploadFile(file);
    }

    const updated = (await api.updateTableRow('clothing-items', item.id, {
      name: editForm.name.trim(),
      category_id: typeof editForm.category_id === 'number' ? editForm.category_id : null,
      subcategory_id: typeof editForm.subcategory_id === 'number' ? editForm.subcategory_id : null,
      color_id: editForm.color_id === NEW_OPTION ? null : editForm.color_id,
      brand_id: editForm.brand_id === NEW_OPTION ? null : editForm.brand_id,
      purchase_price: editForm.purchase_price,
      owned_since: editForm.owned_since || null,
      origin: editForm.origin.trim() || null,
      laundry_impact: editForm.laundry_impact.trim() || null,
      wear_count: editForm.wear_count,
      repairable: editForm.repairable,
      is_active: editForm.is_active,
      notes: editForm.notes.trim() || null,
      ...(imagePath ? { image_path: imagePath } : {})
    })) as ClothingItem | null;

    if (updated) {
      Object.assign(item, updated);
    }
    editMode.value = false;
  } catch (err) {
    formError.value = String(err);
  } finally {
    editSaving.value = false;
  }
}

function formatDate(value: string | null | undefined): string {
  if (!value) return '—';
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? value : d.toLocaleString();
}

const confirmingDelete = ref(false);
const deleting = ref(false);
const deactivating = ref(false);

async function removeItemAndBack() {
  const item = selectedItem.value;
  if (!item) return;
  deleting.value = true;
  try {
    await api.deleteTableRow('clothing-items', item.id);
    items.value = items.value.filter(i => i.id !== item.id);
    selectedItemId.value = null;
    confirmingDelete.value = false;
  } catch (err) {
    loadError.value = String(err);
  } finally {
    deleting.value = false;
  }
}

async function deactivateSelected() {
  const item = selectedItem.value;
  if (!item) return;
  deactivating.value = true;
  try {
    await api.updateTableRow('clothing-items', item.id, { is_active: false });
    item.is_active = false;
    confirmingDelete.value = false;
  } catch (err) {
    loadError.value = String(err);
  } finally {
    deactivating.value = false;
  }
}

onMounted(async () => {
  try {
    loading.value = true;
    const [itemData, colorData, categoryData, subcategoryData, brandData] =
      await Promise.all([
        api.listClothingItems(),
        api.listColors(),
        api.listCategories(),
        api.listSubcategories(),
        api.listBrands()
      ]);
    items.value = Array.isArray(itemData) ? itemData : [];
    colors.value = Array.isArray(colorData) ? colorData : [];
    categories.value = Array.isArray(categoryData) ? categoryData : [];
    subcategories.value = Array.isArray(subcategoryData) ? subcategoryData : [];
    brands.value = Array.isArray(brandData) ? brandData : [];
  } catch (err) {
    loadError.value = String(err);
  } finally {
    loading.value = false;
  }
});
</script>

<style scoped>
.clothes-view {
  margin-top: 1.5rem;
}

.layout {
  display: flex;
  gap: 1.5rem;
  align-items: flex-start;
}

.list-section {
  flex: 1;
  min-width: 0;
}

.section-title {
  margin: 0 0 1rem;
  font-size: 1.25rem;
  color: #333;
}

.count {
  font-size: 1rem;
  color: #666;
  font-weight: 400;
}

.empty {
  color: #666;
  font-style: italic;
  padding: 1rem 0;
}

.items-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 0.75rem;
}

.item-card {
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.item-card__media {
  position: relative;
  background: #f7f7f7;
  border-bottom: 1px solid #e0e0e0;
}

.item-image {
  width: 100%;
  height: 160px;
  object-fit: cover;
  display: block;
}

.item-image.placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  color: #999;
  font-size: 0.85rem;
  font-style: italic;
}

.inactive-badge {
  position: absolute;
  top: 6px;
  right: 6px;
  background: #fdecea;
  color: #c0392b;
  border: 1px solid #e0b4ae;
  border-radius: 3px;
  font-size: 0.7rem;
  padding: 0.1rem 0.4rem;
}

.item-card__body {
  padding: 0.6rem 0.75rem 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  flex: 1;
}

.item-name {
  margin: 0;
  font-size: 0.95rem;
  color: #333;
}

.item-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.meta-tag {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  background: #eef;
  border: 1px solid #ccd;
  border-radius: 3px;
  padding: 0.1rem 0.45rem;
}

.color-swatch {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  border: 1px solid rgba(0, 0, 0, 0.2);
  display: inline-block;
}

.item-details {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.detail-row {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.8rem;
}

.detail-row dt {
  color: #888;
}

.detail-row dd {
  margin: 0;
  color: #444;
  text-align: right;
}

.item-notes {
  margin: 0;
  font-size: 0.85rem;
  color: #555;
  font-style: italic;
}

.item-card.clickable {
  cursor: pointer;
}

.item-card.clickable:hover {
  border-color: #007bff;
}

.back-btn {
  margin-bottom: 1rem;
  padding: 0.35rem 0.75rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fff;
  cursor: pointer;
  font-size: 0.9rem;
}

.back-btn:hover {
  background: #f0f0f0;
}

.detail-layout {
  display: flex;
  gap: 1.5rem;
  align-items: flex-start;
}

.detail-media {
  width: 360px;
  flex-shrink: 0;
  border: 1px solid #ddd;
  border-radius: 8px;
  overflow: hidden;
  background: #f7f7f7;
}

.detail-media img {
  width: 100%;
  max-height: 480px;
  object-fit: cover;
  display: block;
}

.detail-media-placeholder {
  height: 300px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #999;
  font-size: 0.9rem;
  font-style: italic;
}

.detail-body {
  flex: 1;
  min-width: 0;
  border: 1px solid #ddd;
  border-radius: 8px;
  background: #fff;
  padding: 1.25rem;
}

.detail-header {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  margin-bottom: 0.75rem;
}

.detail-header .inactive-badge {
  position: static;
}

.detail-title {
  margin: 0;
  font-size: 1.4rem;
  color: #333;
}

.detail-rows {
  margin: 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.5rem 1.5rem;
  border-top: 1px solid #eee;
  padding-top: 1rem;
}

.detail-rows .row {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.9rem;
  border-bottom: 1px dashed #eee;
  padding-bottom: 0.3rem;
}

.detail-rows dt {
  color: #888;
}

.detail-rows dd {
  margin: 0;
  color: #333;
  font-weight: 500;
  text-align: right;
}

.color-value {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  text-transform: capitalize;
}

.detail-notes {
  margin: 1rem 0 0;
  font-size: 0.9rem;
  color: #555;
  background: #f9f9f9;
  border: 1px solid #eee;
  border-radius: 6px;
  padding: 0.6rem 0.75rem;
  white-space: pre-wrap;
}

.detail-timestamps {
  margin-top: 1rem;
  font-size: 0.78rem;
  color: #999;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.detail-id {
  font-family: monospace;
  font-size: 0.72rem;
}

.detail-header-actions {
  margin-left: auto;
  display: flex;
  gap: 0.5rem;
}

.confirm-panel {
  margin-top: 1.25rem;
  border: 1px solid #e0b4ae;
  background: #fdecea;
  border-radius: 6px;
  padding: 0.9rem 1rem;
}

.confirm-text {
  margin: 0 0 0.75rem;
  font-size: 0.88rem;
  color: #7b241c;
  line-height: 1.45;
}

.confirm-actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.form-panel {
  width: 360px;
  flex-shrink: 0;
  border: 1px solid #ddd;
  border-radius: 8px;
  background: #f9f9f9;
  padding: 1.25rem;
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

  .form-panel {
    width: 100%;
  }

  .detail-layout {
    flex-direction: column;
  }

  .detail-media {
    width: 100%;
  }
}
</style>
