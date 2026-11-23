/**
 * Shared wardrobe store.
 *
 * Every tab used to fetch its own copy of the wardrobe inside `onMounted`.
 * Because the app keeps tabs alive, those copies drifted apart: a newly added
 * item was missing from the outfit picker, logged outfits did not update wear
 * counts in the clothes view, and Insights kept showing an old snapshot.
 *
 * This module owns one reactive copy of the data for the whole session. Views
 * read from it, mutation helpers keep it in sync, and `onActivated` callers can
 * ask for a background refresh when the data has gone stale.
 */
import { readonly, ref } from 'vue';
import { api } from '../api/client';
import { classifyError } from './errors';
import type {
  Brand,
  ClothingCategory,
  ClothingItem,
  ClothingSubcategory,
  Color,
  Outfit,
} from '../api/types';

/** How long a loaded snapshot is considered fresh when a tab is re-activated. */
export const WARDROBE_STALE_MS = 30_000;

/**
 * Human-readable notice for a capped snapshot (UX_ANALYSIS.md P1-B3). Returns
 * null when every collection is complete, so views can render the notice only
 * when it is actually true.
 */
export function formatTruncationNotice(
  itemsLoaded: number,
  itemsTotal: number | null,
  outfitsLoaded: number,
  outfitsTotal: number | null
): string | null {
  const parts: string[] = [];
  if (itemsTotal != null && itemsTotal > itemsLoaded) {
    parts.push(`showing ${itemsLoaded} of ${itemsTotal} clothing items`);
  }
  if (outfitsTotal != null && outfitsTotal > outfitsLoaded) {
    parts.push(`${outfitsLoaded} of ${outfitsTotal} outfits`);
  }
  if (parts.length === 0) return null;
  return `${parts.join(' and ')} — some data is not loaded, so results cover only the loaded subset.`;
}

export type WardrobeClient = Pick<
  typeof api,
  | 'listClothingItemsWithTotal'
  | 'listOutfitsWithTotal'
  | 'listColors'
  | 'listCategories'
  | 'listSubcategories'
  | 'listBrands'
>;

export interface LoadOptions {
  /** Always fetch, even when a fresh snapshot exists. */
  force?: boolean;
  /** Refetch only when the snapshot is older than this many milliseconds. */
  maxAgeMs?: number;
}

function asArray<T>(value: unknown): T[] {
  return Array.isArray(value) ? (value as T[]) : [];
}

export function createWardrobeStore(client: WardrobeClient = api) {
  const items = ref<ClothingItem[]>([]);
  /**
   * Server-reported totals (X-Total-Count). Null when the endpoint predates
   * the signal; `null` means "unknown", never "complete".
   */
  const itemTotal = ref<number | null>(null);
  const outfitTotal = ref<number | null>(null);
  const outfits = ref<Outfit[]>([]);
  const colors = ref<Color[]>([]);
  const categories = ref<ClothingCategory[]>([]);
  const subcategories = ref<ClothingSubcategory[]>([]);
  const brands = ref<Brand[]>([]);

  const loaded = ref(false);
  const loading = ref(true);
  const error = ref<string | null>(null);
  const lastLoadedAt = ref(0);

  let inflight: Promise<void> | null = null;

  /**
   * Incremented on every local mutation. A refresh that started before a
   * mutation must not overwrite the newer local state when it lands.
   */
  let revision = 0;

  async function load(options: LoadOptions = {}): Promise<void> {
    if (inflight) return inflight;

    if (!options.force && loaded.value) {
      if (options.maxAgeMs == null || Date.now() - lastLoadedAt.value < options.maxAgeMs) {
        return;
      }
    }

    const showSpinner = !loaded.value;
    if (showSpinner) {
      loading.value = true;
      error.value = null;
    }

    const startedAtRevision = revision;

    inflight = (async () => {
      try {
        const [itemsRes, outfitsRes, colorData, categoryData, subcategoryData, brandData] =
          await Promise.all([
            client.listClothingItemsWithTotal(),
            client.listOutfitsWithTotal(),
            client.listColors(),
            client.listCategories(),
            client.listSubcategories(),
            client.listBrands(),
          ]);

        // A mutation happened while this request was in flight: keep the
        // newer local state and let the next activation refresh instead.
        if (revision !== startedAtRevision) return;

        items.value = asArray<ClothingItem>(itemsRes.items);
        itemTotal.value = itemsRes.total;
        outfits.value = asArray<Outfit>(outfitsRes.items);
        outfitTotal.value = outfitsRes.total;
        colors.value = asArray<Color>(colorData);
        categories.value = asArray<ClothingCategory>(categoryData);
        subcategories.value = asArray<ClothingSubcategory>(subcategoryData);
        brands.value = asArray<Brand>(brandData);

        loaded.value = true;
        lastLoadedAt.value = Date.now();
        error.value = null;
      } catch (err) {
        // Once data has loaded, a failed background refresh should not replace
        // the screen with an error. Keep the cached snapshot instead.
        if (!loaded.value) {
          error.value = classifyError(err).message;
        }
      } finally {
        loading.value = false;
        inflight = null;
      }
    })();

    return inflight;
  }

  function refresh(): Promise<void> {
    return load({ force: true });
  }

  /** Mark the snapshot stale so the next `load({ maxAgeMs })` refetches. */
  function invalidate(): void {
    lastLoadedAt.value = 0;
  }

  function touch(): void {
    revision += 1;
  }

  // ---------------------------------------------------------------------------
  // Clothing items
  // ---------------------------------------------------------------------------

  function addItem(item: ClothingItem): void {
    touch();
    items.value = [item, ...items.value.filter(existing => existing.id !== item.id)];
  }

  /**
   * Replace an item everywhere it is rendered. Outfits now hold only clothing-
   * item ids, so there is no joined copy of the row to keep in step: every
   * consumer resolves the id against `items`, which we update here.
   */
  function replaceItem(item: ClothingItem): void {
    touch();
    const index = items.value.findIndex(existing => existing.id === item.id);
    if (index === -1) {
      items.value = [item, ...items.value];
    } else {
      const next = items.value.slice();
      next[index] = item;
      items.value = next;
    }
  }

  function patchItem(id: string, patch: Partial<ClothingItem>): void {
    touch();
    items.value = items.value.map(item => (item.id === id ? { ...item, ...patch } : item));
  }

  function removeItem(id: string): void {
    touch();
    items.value = items.value.filter(item => item.id !== id);

    // The backend removes the item from every outfit it belonged to. Mirror
    // that locally so the outfits view stops showing a deleted item.
    outfits.value = outfits.value.map(outfit => {
      if (!outfit.item_ids.some(outfitItemId => outfitItemId === id)) return outfit;
      return {
        ...outfit,
        item_ids: outfit.item_ids.filter(outfitItemId => outfitItemId !== id),
      };
    });
  }

  // ---------------------------------------------------------------------------
  // Outfits
  // ---------------------------------------------------------------------------

  function adjustWearCounts(itemIds: string[], delta: number): void {
    const ids = new Set(itemIds);
    if (ids.size === 0) return;

    items.value = items.value.map(item =>
      ids.has(item.id)
        ? { ...item, wear_count: Math.max(0, (item.wear_count ?? 0) + delta) }
        : item,
    );
  }

  /**
   * Resolve an outfit's item ids against the current clothing-item snapshot.
   * One lookup pass per outfit, sharing the reactive item objects rather than
   * copying rows, so renaming or re-photoing an item needs no outfit update.
   * Ids whose item no longer exists are dropped (the backend prunes them too).
   */
  function itemsOf(outfit: Outfit): ClothingItem[] {
    if (!outfit.item_ids || outfit.item_ids.length === 0) return [];
    if (outfit.item_ids.length === 1) {
      const single = items.value.find(item => item.id === outfit.item_ids[0]);
      return single ? [single] : [];
    }

    const byId = new Map<string, ClothingItem>(items.value.map(item => [item.id, item]));
    return outfit.item_ids.map(id => byId.get(id)).filter((item) => item != null);
  }

  function addOutfit(outfit: Outfit): void {
    touch();
    outfits.value = [...outfits.value.filter(existing => existing.id !== outfit.id), outfit].sort(
      (a, b) => a.date.localeCompare(b.date),
    );
    // The server increments wear counts when an outfit is created.
    adjustWearCounts(outfit.item_ids, 1);
  }

  function removeOutfit(outfit: Outfit): void {
    touch();
    outfits.value = outfits.value.filter(existing => existing.id !== outfit.id);
    // The server decrements wear counts when an outfit is deleted.
    adjustWearCounts(outfit.item_ids, -1);
  }

  /**
   * Mirror of `PATCH /outfits/:id`: the caller passes the server's updated
   * row, the snapshot swaps it in (re-sorting by date), and wear counts move
   * exactly as the server moved them in its transaction: +1 for items the
   * outfit gained, -1 (floored at 0) for items it lost.
   */
  function replaceOutfit(outfit: Outfit): void {
    const index = outfits.value.findIndex(existing => existing.id === outfit.id);
    const previous = index === -1 ? null : outfits.value[index];

    const next = outfits.value.slice();
    if (index === -1) {
      next.push(outfit);
    } else {
      next[index] = outfit;
    }
    touch();
    outfits.value = next.sort((a, b) => a.date.localeCompare(b.date));

    if (!previous) return;
    const before = new Set(previous.item_ids);
    const after = new Set(outfit.item_ids);
    const added = outfit.item_ids.filter(id => !before.has(id));
    const removed = previous.item_ids.filter(id => !after.has(id));
    if (added.length > 0) adjustWearCounts(added, 1);
    if (removed.length > 0) adjustWearCounts(removed, -1);
  }

  // ---------------------------------------------------------------------------
  // Shared reference data
  // ---------------------------------------------------------------------------

  function addColor(color: Color): void {
    touch();
    if (!colors.value.some(existing => existing.id === color.id)) {
      colors.value = [...colors.value, color];
    }
  }

  function addCategory(category: ClothingCategory): void {
    touch();
    if (!categories.value.some(existing => existing.id === category.id)) {
      categories.value = [...categories.value, category];
    }
  }

  function addSubcategory(subcategory: ClothingSubcategory): void {
    touch();
    if (!subcategories.value.some(existing => existing.id === subcategory.id)) {
      subcategories.value = [...subcategories.value, subcategory];
    }
  }

  function addBrand(brand: Brand): void {
    touch();
    if (!brands.value.some(existing => existing.id === brand.id)) {
      brands.value = [...brands.value, brand];
    }
  }

  // ---------------------------------------------------------------------------
  // Reference-data management (M5): local mirrors of the dedicated
  // rename/delete endpoints, which re-point or clear item references in the
  // same server transaction.
  // ---------------------------------------------------------------------------

  /** Rename a color: the row keeps its identity slot, every item that used
   * the old slug now carries the new one. */
  function renameColor(oldId: string, newId: string): void {
    if (oldId === newId) return;
    touch();
    colors.value = colors.value.map(color => (color.id === oldId ? { ...color, id: newId } : color));
    items.value = items.value.map(item =>
      item.color_id === oldId ? { ...item, color_id: newId } : item,
    );
  }

  /** Delete a color: only ever succeeds when nothing references it, so no
   * item repair is needed. */
  function removeColor(id: string): void {
    touch();
    colors.value = colors.value.filter(color => color.id !== id);
    items.value = items.value.map(item =>
      item.color_id === id ? { ...item, color_id: null } : item,
    );
  }

  /** Rename a category: the integer id is stable, so item references are
   * untouched - only the row's name changes. */
  function renameCategory(id: number, name: string): void {
    touch();
    categories.value = categories.value.map(category =>
      category.id === id ? { ...category, name } : category,
    );
  }

  /** Delete a category: the server removes its owned subcategories and
   * clears every reference from the caller's items; both are mirrored. */
  function removeCategory(id: number): void {
    const removedSubs = new Set(
      subcategories.value.filter(sub => sub.category_id === id).map(sub => sub.id),
    );
    touch();
    categories.value = categories.value.filter(category => category.id !== id);
    subcategories.value = subcategories.value.filter(sub => !removedSubs.has(sub.id));
    items.value = items.value.map(item => {
      if (item.category_id !== id && !removedSubs.has(item.subcategory_id ?? -1)) return item;
      return {
        ...item,
        category_id: item.category_id === id ? null : item.category_id,
        subcategory_id: removedSubs.has(item.subcategory_id ?? -1) ? null : item.subcategory_id,
      };
    });
  }

  /** Rename (and optionally move) a subcategory: id is stable, items keep
   * pointing at it. */
  function renameSubcategory(id: number, name: string, categoryId?: number | null): void {
    touch();
    subcategories.value = subcategories.value.map(sub =>
      sub.id === id
        ? { ...sub, name, category_id: categoryId ?? sub.category_id }
        : sub,
    );
  }

  function removeSubcategory(id: number): void {
    touch();
    subcategories.value = subcategories.value.filter(sub => sub.id !== id);
    items.value = items.value.map(item =>
      item.subcategory_id === id ? { ...item, subcategory_id: null } : item,
    );
  }

  function renameBrand(id: string, name: string): void {
    touch();
    brands.value = brands.value.map(brand => (brand.id === id ? { ...brand, name } : brand));
  }

  /** Delete a brand: item references are cleared by the server. */
  function removeBrand(id: string): void {
    touch();
    brands.value = brands.value.filter(brand => brand.id !== id);
    items.value = items.value.map(item =>
      item.brand_id === id ? { ...item, brand_id: null } : item,
    );
  }

  return {
    items,
    itemTotal: readonly(itemTotal),
    outfits,
    outfitTotal: readonly(outfitTotal),
    colors,
    categories,
    subcategories,
    brands,
    loaded: readonly(loaded),
    loading: readonly(loading),
    error: readonly(error),
    lastLoadedAt: readonly(lastLoadedAt),
    load,
    refresh,
    invalidate,
    addItem,
    replaceItem,
    patchItem,
    removeItem,
    addOutfit,
    replaceOutfit,
    removeOutfit,
    itemsOf,
    addColor,
    addCategory,
    addSubcategory,
    addBrand,
    renameColor,
    removeColor,
    renameCategory,
    removeCategory,
    renameSubcategory,
    removeSubcategory,
    renameBrand,
    removeBrand,
  };
}

export type WardrobeStore = ReturnType<typeof createWardrobeStore>;

const wardrobeStore = createWardrobeStore();

/** The single wardrobe snapshot shared by every tab. */
export function useWardrobe(): WardrobeStore {
  return wardrobeStore;
}
