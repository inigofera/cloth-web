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
   * Replace an item everywhere it is rendered. Outfit cards keep their own
   * joined snapshot of the clothing row, so updating only `items` would leave
   * stale names/photos in the outfits view.
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

    outfits.value = outfits.value.map(outfit => {
      if (!outfit.items.some(outfitItem => outfitItem.id === item.id)) return outfit;
      return {
        ...outfit,
        items: outfit.items.map(outfitItem =>
          outfitItem.id === item.id ? { ...outfitItem, ...item } : outfitItem,
        ),
      };
    });
  }

  function patchItem(id: string, patch: Partial<ClothingItem>): void {
    touch();
    items.value = items.value.map(item => (item.id === id ? { ...item, ...patch } : item));

    outfits.value = outfits.value.map(outfit => {
      if (!outfit.items.some(outfitItem => outfitItem.id === id)) return outfit;
      return {
        ...outfit,
        items: outfit.items.map(outfitItem =>
          outfitItem.id === id ? { ...outfitItem, ...patch } : outfitItem,
        ),
      };
    });
  }

  function removeItem(id: string): void {
    touch();
    items.value = items.value.filter(item => item.id !== id);

    // The backend removes the item from every outfit it belonged to. Mirror
    // that locally so the outfits view stops showing a deleted item.
    outfits.value = outfits.value.map(outfit => {
      if (!outfit.items.some(outfitItem => outfitItem.id === id)) return outfit;
      return {
        ...outfit,
        items: outfit.items.filter(outfitItem => outfitItem.id !== id),
      };
    });
  }

  // ---------------------------------------------------------------------------
  // Outfits
  // ---------------------------------------------------------------------------

  function adjustWearCounts(outfitItems: Array<{ id: string }>, delta: number): void {
    const ids = new Set(outfitItems.map(item => item.id));
    if (ids.size === 0) return;

    items.value = items.value.map(item =>
      ids.has(item.id)
        ? { ...item, wear_count: Math.max(0, (item.wear_count ?? 0) + delta) }
        : item,
    );
  }

  function addOutfit(outfit: Outfit): void {
    touch();
    outfits.value = [...outfits.value.filter(existing => existing.id !== outfit.id), outfit].sort(
      (a, b) => a.date.localeCompare(b.date),
    );
    // The server increments wear counts when an outfit is created.
    adjustWearCounts(outfit.items, 1);
  }

  function removeOutfit(outfit: Outfit): void {
    touch();
    outfits.value = outfits.value.filter(existing => existing.id !== outfit.id);
    // The server decrements wear counts when an outfit is deleted.
    adjustWearCounts(outfit.items, -1);
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
    removeOutfit,
    addColor,
    addCategory,
    addSubcategory,
    addBrand,
  };
}

export type WardrobeStore = ReturnType<typeof createWardrobeStore>;

const wardrobeStore = createWardrobeStore();

/** The single wardrobe snapshot shared by every tab. */
export function useWardrobe(): WardrobeStore {
  return wardrobeStore;
}
