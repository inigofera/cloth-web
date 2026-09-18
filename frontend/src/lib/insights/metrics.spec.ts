import { describe, it, expect } from 'vitest';
import {
  applyFilters,
  computeStat,
  currentStreak,
  dayOfWeek,
  daysBetween,
  dustyItems,
  expensiveDusty,
  costPerWear,
  categoryBreakdown,
  colorBreakdown,
  brandBreakdown,
  priceHistogram,
  wardrobeAge,
  itemPairings,
  itemWears,
  laundryMix,
  normalizeImpact,
  outfitWearMap,
  repeatWears,
  seasonPattern,
  spendStats,
  textInsights,
  topWorn,
  wearTrend,
  addDays,
  dateKeyOf,
} from './metrics';
import { DEFAULT_FILTERS, type EnrichedItem, type EnrichedOutfit, type InsightDataset } from './types';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

let itemSeq = 0;

function makeItem(overrides: Partial<EnrichedItem> = {}): EnrichedItem {
  itemSeq++;
  return {
    id: overrides.id ?? `item-${itemSeq}`,
    name: overrides.name ?? `Item ${itemSeq}`,
    image_path: null,
    category_id: null,
    subcategory_id: null,
    color_id: null,
    brand_id: null,
    purchase_price: null,
    owned_since: null,
    laundry_impact: null,
    is_active: true,
    wear_count: null,
    created_at: '2024-01-01T00:00:00Z',
    category_name: null,
    subcategory_name: null,
    color_hex: null,
    brand_name: null,
    ...overrides,
  };
}

let outfitSeq = 0;

function makeOutfit(dateKey: string, items: EnrichedItem[], overrides: Partial<EnrichedOutfit> = {}): EnrichedOutfit {
  outfitSeq++;
  return {
    id: `outfit-${outfitSeq}`,
    date: `${dateKey}T12:00:00Z`,
    dateKey,
    notes: null,
    items,
    ...overrides,
  };
}

function makeDataset(items: EnrichedItem[], outfits: EnrichedOutfit[]): InsightDataset {
  return {
    items,
    outfits,
    colors: new Map(),
    categories: new Map(),
    subcategories: new Map(),
    brands: new Map(),
  };
}

const noFilters = { ...DEFAULT_FILTERS };

function daysAgo(n: number): string {
  return dateKeyOf(addDays(new Date(), -n));
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

describe('applyFilters', () => {
  it('returns everything with no filters', () => {
    const a = makeItem({ category_id: 1 });
    const b = makeItem({ category_id: 2, is_active: false });
    const ds = makeDataset([a, b], [makeOutfit('2025-01-01', [a])]);
    const out = applyFilters(ds, noFilters);
    expect(out.items).toHaveLength(2);
    expect(out.outfits).toHaveLength(1);
  });

  it('filters items by category, brand, color and active flag', () => {
    const a = makeItem({ category_id: 1, brand_id: 'b1', color_id: 'blue' });
    const b = makeItem({ category_id: 2, brand_id: 'b2', color_id: 'red', is_active: false });
    const ds = makeDataset([a, b], []);

    expect(applyFilters(ds, { ...noFilters, categories: [1] }).items).toEqual([a]);
    expect(applyFilters(ds, { ...noFilters, brands: ['b2'] }).items).toEqual([b]);
    expect(applyFilters(ds, { ...noFilters, colors: ['blue'] }).items).toEqual([a]);
    expect(applyFilters(ds, { ...noFilters, activeOnly: true }).items).toEqual([a]);
  });

  it('applies the date range to outfits and drops outfits with no matching items', () => {
    const a = makeItem({ category_id: 1 });
    const b = makeItem({ category_id: 2 });
    const ds = makeDataset([a, b], [
      makeOutfit('2025-01-01', [a]),
      makeOutfit('2025-06-01', [b]),
    ]);

    const ranged = applyFilters(ds, { ...noFilters, rangePreset: 'custom', from: '2025-05-01', to: '2025-12-31' });
    expect(ranged.outfits).toHaveLength(1);
    expect(ranged.outfits[0].items[0].id).toBe(b.id);

    const catFiltered = applyFilters(ds, { ...noFilters, categories: [1] });
    expect(catFiltered.outfits).toHaveLength(1);
    expect(catFiltered.outfits[0].items).toEqual([a]);
  });

  it('presets resolve relative to today', () => {
    const a = makeItem();
    const ds = makeDataset([a], [
      makeOutfit(daysAgo(10), [a]),
      makeOutfit(daysAgo(300), [a]),
    ]);
    expect(applyFilters(ds, { ...noFilters, rangePreset: '30d' }).outfits).toHaveLength(1);
    expect(applyFilters(ds, { ...noFilters, rangePreset: '1y' }).outfits).toHaveLength(2);
  });
});

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

describe('computeStat', () => {
  it('counts items and outfits', () => {
    const a = makeItem();
    const b = makeItem({ is_active: false });
    const data = applyFilters(makeDataset([a, b], [makeOutfit('2025-01-01', [a])]), noFilters);
    expect(computeStat(data, 'total_items').value).toBe(2);
    expect(computeStat(data, 'active_items').value).toBe(1);
    expect(computeStat(data, 'total_outfits').value).toBe(1);
  });

  it('computes spend and average price, ignoring null prices', () => {
    const data = applyFilters(
      makeDataset([makeItem({ purchase_price: 10 }), makeItem({ purchase_price: 30 }), makeItem()], []),
      noFilters,
    );
    expect(computeStat(data, 'total_spend').value).toBe(40);
    expect(computeStat(data, 'avg_price').value).toBe(20);
  });

  it('computes cost per wear from priced items', () => {
    const a = makeItem({ purchase_price: 100 });
    const data = applyFilters(
      makeDataset([a, makeItem({ purchase_price: 50 })], [makeOutfit('2025-01-01', [a])]),
      noFilters,
    );
    // spend 150, wears: a=1 (logged), b=0 → 150 / 1
    expect(computeStat(data, 'cost_per_wear').value).toBe(150);
  });

  it('returns null cost per wear with no wears', () => {
    const data = applyFilters(makeDataset([makeItem({ purchase_price: 100 })], []), noFilters);
    expect(computeStat(data, 'cost_per_wear').value).toBeNull();
  });

  it('computes items per outfit', () => {
    const a = makeItem();
    const b = makeItem();
    const data = applyFilters(
      makeDataset([a, b], [makeOutfit('2025-01-01', [a, b]), makeOutfit('2025-01-02', [a])]),
      noFilters,
    );
    expect(computeStat(data, 'items_per_outfit').value).toBe(1.5);
  });
});

describe('currentStreak', () => {
  it('counts consecutive days ending today', () => {
    const a = makeItem();
    const outfits = [daysAgo(0), daysAgo(1), daysAgo(2)].map(d => makeOutfit(d, [a]));
    expect(currentStreak(outfits)).toBe(3);
  });

  it('streak survives if today is not logged yet', () => {
    const a = makeItem();
    const outfits = [daysAgo(1), daysAgo(2)].map(d => makeOutfit(d, [a]));
    expect(currentStreak(outfits)).toBe(2);
  });

  it('is zero with no outfits and breaks on gaps', () => {
    expect(currentStreak([])).toBe(0);
    const a = makeItem();
    expect(currentStreak([makeOutfit(daysAgo(5), [a]), makeOutfit(daysAgo(6), [a])])).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// Usage widgets
// ---------------------------------------------------------------------------

describe('wearTrend', () => {
  it('is empty without outfits', () => {
    expect(wearTrend(applyFilters(makeDataset([], []), noFilters), 'week')).toEqual([]);
  });

  it('buckets by month and fills gaps', () => {
    const a = makeItem();
    const data = applyFilters(
      makeDataset([a], [makeOutfit('2025-01-15', [a]), makeOutfit('2025-03-10', [a])]),
      noFilters,
    );
    const trend = wearTrend(data, 'month');
    expect(trend.map(p => p.value)).toEqual([1, 0, 1]);
    expect(trend).toHaveLength(3);
  });

  it('buckets by week (Monday start) and fills gaps', () => {
    const a = makeItem();
    // 2025-01-06 is a Monday; 2025-01-08 is in the same week; 2025-01-13 is the next Monday.
    const data = applyFilters(
      makeDataset([a], [makeOutfit('2025-01-06', [a]), makeOutfit('2025-01-08', [a]), makeOutfit('2025-01-13', [a])]),
      noFilters,
    );
    const trend = wearTrend(data, 'week');
    expect(trend.map(p => p.value)).toEqual([2, 1]);
  });
});

describe('topWorn', () => {
  const a = makeItem({ name: 'A' });
  const b = makeItem({ name: 'B' });
  const c = makeItem({ name: 'C', wear_count: 99 });

  it('ranks by logged outfits', () => {
    const data = applyFilters(
      makeDataset([a, b, c], [
        makeOutfit('2025-01-01', [a, b]),
        makeOutfit('2025-01-02', [a]),
      ]),
      noFilters,
    );
    const top = topWorn(data, 2, 'outfits');
    expect(top.map(t => t.name)).toEqual(['A', 'B']);
    expect(top[0].count).toBe(2);
  });

  it('can rank by manual wear_count and skips zero counts', () => {
    const data = applyFilters(makeDataset([a, b, c], []), noFilters);
    const top = topWorn(data, 5, 'wear_count');
    expect(top.map(t => t.name)).toEqual(['C']);
  });
});

describe('dustyItems', () => {
  it('lists items not worn within the threshold, never-worn first', () => {
    const worn = makeItem({ name: 'Worn' });
    const dusty = makeItem({ name: 'Dusty' });
    const never = makeItem({ name: 'Never' });
    const data = applyFilters(
      makeDataset([worn, dusty, never], [
        makeOutfit(daysAgo(1), [worn]),
        makeOutfit(daysAgo(60), [dusty]),
      ]),
      noFilters,
    );
    const dustyList = dustyItems(data, 30);
    expect(dustyList.map(d => d.name)).toEqual(['Never', 'Dusty']);
    expect(dustyList[0].daysSince).toBeNull();
    expect(dustyList[1].daysSince).toBe(60);
  });
});

describe('dayOfWeek', () => {
  it('counts outfits per weekday, Monday first', () => {
    const a = makeItem();
    // 2024-01-01 is a Monday, 2024-01-06 a Saturday, 2024-01-07 a Sunday.
    const data = applyFilters(
      makeDataset([a], [makeOutfit('2024-01-01', [a]), makeOutfit('2024-01-06', [a]), makeOutfit('2024-01-07', [a])]),
      noFilters,
    );
    const dow = dayOfWeek(data);
    expect(dow).toHaveLength(7);
    expect(dow[0].value).toBe(1); // Mon
    expect(dow[5].value).toBe(1); // Sat
    expect(dow[6].value).toBe(1); // Sun
    expect(dow.reduce((s, d) => s + d.value, 0)).toBe(3);
  });
});

describe('seasonPattern', () => {
  it('fills a 12x7 grid', () => {
    const a = makeItem();
    const data = applyFilters(makeDataset([a], [makeOutfit('2024-03-11', [a]), makeOutfit('2024-03-11', [a])]), noFilters);
    const { values, max } = seasonPattern(data);
    expect(values).toHaveLength(12);
    expect(values[2]).toHaveLength(7);
    // 2024-03-11 is a Monday
    expect(values[2][0]).toBe(2);
    expect(max).toBe(2);
  });
});

// ---------------------------------------------------------------------------
// Wardrobe composition
// ---------------------------------------------------------------------------

describe('categoryBreakdown', () => {
  it('groups by category or subcategory with fallbacks', () => {
    const a = makeItem({ category_id: 1, category_name: 'Tops' });
    const b = makeItem({ category_id: 1, category_name: 'Tops', subcategory_name: 'Shirts' });
    const c = makeItem();
    const data = applyFilters(makeDataset([a, b, c], []), noFilters);

    expect(categoryBreakdown(data, 'category')).toEqual([
      { label: 'Tops', value: 2 },
      { label: 'Uncategorized', value: 1 },
    ]);
    expect(categoryBreakdown(data, 'subcategory')).toEqual([
      { label: 'Shirts', value: 1 },
      { label: 'Tops', value: 1 },
      { label: 'Uncategorized', value: 1 },
    ]);
  });
});

describe('colorBreakdown', () => {
  it('keeps hex values and sorts descending', () => {
    const data = applyFilters(
      makeDataset(
        [
          makeItem({ color_id: 'blue', color_hex: '#0000ff' }),
          makeItem({ color_id: 'blue', color_hex: '#0000ff' }),
          makeItem({ color_id: 'red', color_hex: '#ff0000' }),
          makeItem(),
        ],
        [],
      ),
      noFilters,
    );
    expect(colorBreakdown(data)).toEqual([
      { label: 'blue', hex: '#0000ff', value: 2 },
      { label: 'No color', hex: null, value: 1 },
      { label: 'red', hex: '#ff0000', value: 1 },
    ]);
  });
});

describe('brandBreakdown', () => {
  it('limits to top N', () => {
    const data = applyFilters(
      makeDataset(
        [makeItem({ brand_name: 'A' }), makeItem({ brand_name: 'A' }), makeItem({ brand_name: 'B' }), makeItem({ brand_name: 'C' })],
        [],
      ),
      noFilters,
    );
    expect(brandBreakdown(data, 2)).toEqual([
      { label: 'A', value: 2 },
      { label: 'B', value: 1 },
    ]);
  });
});

describe('priceHistogram', () => {
  it('buckets prices and handles a single value', () => {
    const data = applyFilters(
      makeDataset(
        [makeItem({ purchase_price: 10 }), makeItem({ purchase_price: 20 }), makeItem({ purchase_price: 30 }), makeItem({ purchase_price: 90 })],
        [],
      ),
      noFilters,
    );
    const hist = priceHistogram(data, 4);
    expect(hist).toHaveLength(4);
    expect(hist.reduce((s, h) => s + h.value, 0)).toBe(4);

    const single = priceHistogram(applyFilters(makeDataset([makeItem({ purchase_price: 42 })], []), noFilters), 4);
    expect(single).toEqual([{ label: '42.00', value: 1 }]);
  });

  it('is empty without prices', () => {
    expect(priceHistogram(applyFilters(makeDataset([makeItem()], []), noFilters), 4)).toEqual([]);
  });
});

describe('wardrobeAge', () => {
  it('groups by owned year, falling back to created_at', () => {
    const data = applyFilters(
      makeDataset(
        [
          makeItem({ owned_since: '2022-05-01' }),
          makeItem({ owned_since: '2022-11-01' }),
          makeItem({ created_at: '2023-06-01T00:00:00Z' }),
        ],
        [],
      ),
      noFilters,
    );
    expect(wardrobeAge(data)).toEqual([
      { label: '2022', value: 2 },
      { label: '2023', value: 1 },
    ]);
  });
});

// ---------------------------------------------------------------------------
// Value & cost
// ---------------------------------------------------------------------------

describe('spendStats', () => {
  it('computes total, avg and median', () => {
    const data = applyFilters(
      makeDataset([makeItem({ purchase_price: 10 }), makeItem({ purchase_price: 20 }), makeItem({ purchase_price: 30 })], []),
      noFilters,
    );
    expect(spendStats(data)).toEqual({ total: 60, avg: 20, median: 20, count: 3 });
  });

  it('averages the middle two for even counts', () => {
    const data = applyFilters(
      makeDataset([makeItem({ purchase_price: 10 }), makeItem({ purchase_price: 30 })], []),
      noFilters,
    );
    expect(spendStats(data).median).toBe(20);
  });

  it('returns zeros/nulls without prices', () => {
    expect(spendStats(applyFilters(makeDataset([makeItem()], []), noFilters))).toEqual({
      total: 0,
      avg: null,
      median: null,
      count: 0,
    });
  });
});

describe('costPerWear', () => {
  const a = makeItem({ name: 'Cheap', purchase_price: 10 });
  const b = makeItem({ name: 'Expensive', purchase_price: 200 });
  const data = applyFilters(
    makeDataset([a, b], [
      makeOutfit('2025-01-01', [a, b]),
      makeOutfit('2025-01-02', [a]),
      makeOutfit('2025-01-03', [a]),
    ]),
    noFilters,
  );

  it('ranks best and worst by cost per wear', () => {
    const best = costPerWear(data, 'best', 5);
    expect(best.map(r => r.name)).toEqual(['Cheap', 'Expensive']);
    expect(best[0].cpw).toBeCloseTo(10 / 3);
    expect(best[1].cpw).toBe(200);
  });

  it('excludes items that were never worn', () => {
    const never = makeItem({ name: 'Never', purchase_price: 500 });
    const d2 = applyFilters(makeDataset([a, never], [makeOutfit('2025-01-01', [a])]), noFilters);
    expect(costPerWear(d2, 'best', 5).map(r => r.name)).toEqual(['Cheap']);
  });
});

describe('expensiveDusty', () => {
  it('finds pricey items with few wears', () => {
    const cheap = makeItem({ name: 'Cheap', purchase_price: 10 });
    const pricey = makeItem({ name: 'Pricey', purchase_price: 300 });
    const data = applyFilters(
      makeDataset([cheap, pricey], [makeOutfit('2025-01-01', [cheap])]),
      noFilters,
    );
    const rows = expensiveDusty(data, 100, 1);
    expect(rows.map(r => r.name)).toEqual(['Pricey']);
  });
});

// ---------------------------------------------------------------------------
// Sustainability
// ---------------------------------------------------------------------------

describe('normalizeImpact', () => {
  it('buckets free text into Low/Medium/High', () => {
    expect(normalizeImpact('high')).toBe('High');
    expect(normalizeImpact('Very High')).toBe('High');
    expect(normalizeImpact('medium')).toBe('Medium');
    expect(normalizeImpact('low')).toBe('Low');
    expect(normalizeImpact('  Dry clean  ')).toBe('Dry clean');
  });
});

describe('laundryMix', () => {
  it('groups normalized impact values', () => {
    const data = applyFilters(
      makeDataset(
        [makeItem({ laundry_impact: 'low' }), makeItem({ laundry_impact: 'Low' }), makeItem({ laundry_impact: 'HIGH' }), makeItem()],
        [],
      ),
      noFilters,
    );
    expect(laundryMix(data)).toEqual([
      { label: 'Low', value: 2 },
      { label: 'High', value: 1 },
      { label: 'Not set', value: 1 },
    ]);
  });
});

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

describe('itemPairings', () => {
  it('counts co-occurrence within outfits', () => {
    const a = makeItem({ name: 'A' });
    const b = makeItem({ name: 'B' });
    const c = makeItem({ name: 'C' });
    const data = applyFilters(
      makeDataset([a, b, c], [
        makeOutfit('2025-01-01', [a, b, c]),
        makeOutfit('2025-01-02', [a, b]),
        makeOutfit('2025-01-03', [a, c]),
      ]),
      noFilters,
    );
    const pairs = itemPairings(data, 5);
    expect(pairs[0].count).toBe(2);
    expect([pairs[0].aName, pairs[0].bName].sort()).toEqual(['A', 'B']);
    expect(pairs).toHaveLength(3);
  });
});

describe('repeatWears', () => {
  it('counts re-wears within the window', () => {
    const a = makeItem({ name: 'A' });
    const b = makeItem({ name: 'B' });
    const data = applyFilters(
      makeDataset([a, b], [
        makeOutfit('2025-01-01', [a]),
        makeOutfit('2025-01-03', [a]),
        makeOutfit('2025-01-20', [a]),
        makeOutfit('2025-01-01', [b]),
      ]),
      noFilters,
    );
    const rows = repeatWears(data, 7);
    expect(rows).toHaveLength(1);
    expect(rows[0].name).toBe('A');
    expect(rows[0].repeats).toBe(1); // Jan 1 + Jan 3 only; Jan 20 is outside the window
  });
});

// ---------------------------------------------------------------------------
// Text insights
// ---------------------------------------------------------------------------

describe('textInsights', () => {
  it('asks for items when the wardrobe is empty', () => {
    expect(textInsights(applyFilters(makeDataset([], []), noFilters))).toEqual([
      'Add clothing items to start seeing insights.',
    ]);
  });

  it('asks for outfits when nothing is logged', () => {
    const out = textInsights(applyFilters(makeDataset([makeItem()], []), noFilters));
    expect(out).toEqual(['Log outfits in the Outfits tab to unlock usage insights.']);
  });

  it('generates bullets from real data', () => {
    const hero = makeItem({ name: 'Hero', purchase_price: 30 });
    const dusty = makeItem({ name: 'Dusty', purchase_price: 200 });
    const data = applyFilters(
      makeDataset([hero, dusty], [
        makeOutfit('2025-01-01', [hero]),
        makeOutfit('2025-01-02', [hero]),
      ]),
      noFilters,
    );
    const out = textInsights(data);
    expect(out.some(t => t.includes('1 item is never worn'))).toBe(true);
    expect(out.some(t => t.includes('Most worn: Hero'))).toBe(true);
    expect(out.some(t => t.includes('Best value: Hero'))).toBe(true);
    expect(out.some(t => t.includes('expensive'))).toBe(true);
    expect(out.length).toBeLessThanOrEqual(6);
  });

  it('flags wear count mismatches', () => {
    const a = makeItem({ name: 'A', wear_count: 40 });
    const data = applyFilters(makeDataset([a], [makeOutfit('2025-01-01', [a])]), noFilters);
    const out = textInsights(data);
    expect(out.some(t => t.includes('doesn\u2019t match'))).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

describe('itemWears / daysBetween', () => {
  it('takes the max of logged and manual counts', () => {
    const a = makeItem({ wear_count: 10 });
    const map = outfitWearMap(applyFilters(makeDataset([a], [makeOutfit('2025-01-01', [a])]), noFilters));
    expect(itemWears(a, map)).toBe(10);
    // manual count is kept even when the item never appears in logged outfits
    const b = makeItem({ wear_count: 2 });
    expect(itemWears(b, map)).toBe(2);
    const c = makeItem();
    expect(itemWears(c, map)).toBe(0);
  });

  it('computes day differences', () => {
    expect(daysBetween('2025-01-01', '2025-01-11')).toBe(10);
    expect(daysBetween('2025-01-31', '2025-03-03')).toBe(31);
  });
});
