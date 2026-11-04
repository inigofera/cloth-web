import { describe, expect, it, vi } from 'vitest';
import { createWardrobeStore, type WardrobeClient } from './wardrobe';
import type { ClothingItem, Outfit } from '../api/types';

function clothingItem(id: string, overrides: Partial<ClothingItem> = {}): ClothingItem {
  return {
    id,
    name: `Item ${id}`,
    category_id: 1,
    subcategory_id: null,
    color_id: null,
    brand_id: null,
    purchase_price: null,
    owned_since: null,
    origin: null,
    laundry_impact: null,
    repairable: null,
    notes: null,
    image_path: null,
    is_active: true,
    wear_count: 0,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
    ...overrides,
  };
}

function outfit(id: string, date: string, itemIds: string[]): Outfit {
  return {
    id,
    date,
    notes: null,
    image_path: null,
    is_active: true,
    created_at: '2024-01-01T00:00:00Z',
    items: itemIds.map(itemId => ({
      id: itemId,
      name: `Item ${itemId}`,
      image_path: null,
      color_id: null,
    })),
  };
}

function makeClient(overrides: Partial<WardrobeClient> = {}): WardrobeClient {
  return {
    // `total` is null here: simulates a server without the X-Total-Count signal.
    listClothingItemsWithTotal: vi.fn().mockResolvedValue({
      items: [
        clothingItem('i1', { name: 'Blue Shirt', wear_count: 3 }),
        clothingItem('i2', { name: 'Dark Jeans', wear_count: 0 }),
      ],
      total: null,
    }),
    listOutfitsWithTotal: vi.fn().mockResolvedValue({
      items: [outfit('o1', '2024-01-01', ['i1', 'i2'])],
      total: null,
    }),
    listColors: vi.fn().mockResolvedValue([{ id: 'blue', hex_value: '#0000ff' }]),
    listCategories: vi.fn().mockResolvedValue([{ id: 1, name: 'Tops' }]),
    listSubcategories: vi.fn().mockResolvedValue([{ id: 10, name: 'Shirts', category_id: 1 }]),
    listBrands: vi.fn().mockResolvedValue([{ id: 'b1', name: 'Acme' }]),
    ...overrides,
  };
}

describe('wardrobe store', () => {
  it('loads all resources once and shares the snapshot', async () => {
    const client = makeClient();
    const store = createWardrobeStore(client);

    await Promise.all([store.load(), store.load()]);

    expect(client.listClothingItemsWithTotal).toHaveBeenCalledTimes(1);
    expect(client.listOutfitsWithTotal).toHaveBeenCalledTimes(1);
    expect(store.loaded.value).toBe(true);
    expect(store.loading.value).toBe(false);
    expect(store.items.value).toHaveLength(2);
    expect(store.outfits.value).toHaveLength(1);
    expect(store.colors.value).toHaveLength(1);
    expect(store.categories.value).toHaveLength(1);
    expect(store.subcategories.value).toHaveLength(1);
    expect(store.brands.value).toHaveLength(1);
  });

  it('keeps outfit snapshots in sync when an item changes or is deleted', async () => {
    const store = createWardrobeStore(makeClient());
    await store.load();

    store.replaceItem(clothingItem('i1', { name: 'Renamed Shirt', image_path: 'users/u/new.jpg' }));

    const stored = store.items.value.find(item => item.id === 'i1');
    const snapshot = store.outfits.value[0].items.find(item => item.id === 'i1');
    expect(stored?.name).toBe('Renamed Shirt');
    expect(snapshot?.name).toBe('Renamed Shirt');
    expect(snapshot?.image_path).toBe('users/u/new.jpg');

    store.patchItem('i1', { is_active: false });
    expect(store.items.value.find(item => item.id === 'i1')?.is_active).toBe(false);
    expect(store.outfits.value[0].items.find(item => item.id === 'i1')?.is_active).toBe(false);

    store.removeItem('i1');
    expect(store.items.value.some(item => item.id === 'i1')).toBe(false);
    expect(store.outfits.value[0].items.some(item => item.id === 'i1')).toBe(false);
    // The rest of the outfit is untouched.
    expect(store.outfits.value[0].items.map(item => item.id)).toEqual(['i2']);
  });

  it('adjusts wear counts when outfits are added or removed', async () => {
    const store = createWardrobeStore(makeClient());
    await store.load();

    const newOutfit = outfit('o2', '2024-01-02', ['i1', 'i2', 'i1']);
    store.addOutfit(newOutfit);

    expect(store.outfits.value.map(o => o.id)).toEqual(['o1', 'o2']);
    expect(store.items.value.find(item => item.id === 'i1')?.wear_count).toBe(4);
    expect(store.items.value.find(item => item.id === 'i2')?.wear_count).toBe(1);

    store.removeOutfit(newOutfit);

    expect(store.outfits.value.map(o => o.id)).toEqual(['o1']);
    expect(store.items.value.find(item => item.id === 'i1')?.wear_count).toBe(3);
    expect(store.items.value.find(item => item.id === 'i2')?.wear_count).toBe(0);
  });

  it('adds new reference data without duplicating existing rows', async () => {
    const store = createWardrobeStore(makeClient());
    await store.load();

    store.addColor({ id: 'blue', hex_value: '#111111' });
    store.addColor({ id: 'red', hex_value: '#ff0000' });
    store.addCategory({ id: 2, name: 'Bottoms' });
    store.addSubcategory({ id: 20, name: 'Denim', category_id: 2 });
    store.addBrand({ id: 'b2', name: 'Other' });

    expect(store.colors.value.map(c => c.id)).toEqual(['blue', 'red']);
    expect(store.colors.value.find(c => c.id === 'blue')?.hex_value).toBe('#0000ff');
    expect(store.categories.value.map(c => c.id)).toEqual([1, 2]);
    expect(store.subcategories.value.map(s => s.id)).toEqual([10, 20]);
    expect(store.brands.value.map(b => b.id)).toEqual(['b1', 'b2']);
  });

  it('keeps the cached snapshot when a background refresh fails', async () => {
    const client = makeClient();
    const store = createWardrobeStore(client);
    await store.load();

    (client.listOutfitsWithTotal as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error('offline'));
    await store.refresh();

    expect(store.loaded.value).toBe(true);
    expect(store.loading.value).toBe(false);
    expect(store.error.value).toBeNull();
    expect(store.outfits.value).toHaveLength(1);
  });

  it('stores server-reported totals from list responses', async () => {
    const store = createWardrobeStore(
      makeClient({
        listClothingItemsWithTotal: vi.fn().mockResolvedValue({ items: [], total: 42 }),
        listOutfitsWithTotal: vi.fn().mockResolvedValue({ items: [], total: 7 }),
      })
    );
    await store.load();

    expect(store.itemTotal.value).toBe(42);
    expect(store.outfitTotal.value).toBe(7);
  });

  it('does not let an in-flight refresh overwrite a newer local mutation', async () => {
    let resolveItems!: (items: { items: ClothingItem[]; total: number | null }) => void;
    const client = makeClient({
      listClothingItemsWithTotal: vi.fn(
        () =>
          new Promise<{ items: ClothingItem[]; total: number | null }>(resolve => {
            resolveItems = resolve;
          }),
      ),
    });
    const store = createWardrobeStore(client);

    const loading = store.load();
    store.addItem(clothingItem('new', { name: 'Added while loading' }));
    resolveItems({ items: [clothingItem('old')], total: null });
    await loading;

    expect(store.items.value.map(item => item.id)).toEqual(['new']);
  });
});
