import { describe, it, expect } from 'vitest';
import {
  applyFilters,
  computeStat,
  currentStreak,
  cpwDetail,
  dayOfWeek,
  daysBetween,
  dustyItems,
  expensiveDusty,
  costPerWear,
  categoryBreakdown,
  colorBreakdown,
  brandBreakdown,
  filterItemsByCategory,
  parseIdValue,
  priceHistogram,
  presetRange,
  rangeData,
  resolveItem,
  wardrobeAge,
  impactHotspots,
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
  wardrobeCpwAverage,
  wearTrend,
  widgetRange,
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
  it('limits to top N and folds the rest into Other', () => {
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
      { label: 'Other', value: 1 },
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
      { text: 'Add clothing items to start seeing insights.', category: 'usage' },
    ]);
  });

  it('asks for outfits when nothing is logged', () => {
    const out = textInsights(applyFilters(makeDataset([makeItem()], []), noFilters));
    expect(out).toEqual([{ text: 'Log outfits in the Outfits tab to unlock usage insights.', category: 'usage' }]);
  });

  it('generates tagged bullets from real data', () => {
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
    const texts = out.map(i => i.text);
    expect(texts.some(t => t.includes('1 item is never worn'))).toBe(true);
    expect(texts.some(t => t.includes('Most worn: Hero'))).toBe(true);
    expect(texts.some(t => t.includes('Best value: Hero'))).toBe(true);
    expect(texts.some(t => t.includes('expensive'))).toBe(true);
    expect(out.find(i => i.text.includes('Best value'))?.category).toBe('value');
    expect(out.find(i => i.text.includes('Most worn'))?.category).toBe('usage');
  });

  it('flags wear count mismatches', () => {
    const a = makeItem({ name: 'A', wear_count: 40 });
    const data = applyFilters(makeDataset([a], [makeOutfit('2025-01-01', [a])]), noFilters);
    const out = textInsights(data);
    expect(out.some(i => i.text.includes('doesn\u2019t match'))).toBe(true);
  });

  it('adds a sustainability bullet when high-impact items dominate wears', () => {
    const high = makeItem({ name: 'High', laundry_impact: 'high' });
    const low = makeItem({ name: 'Low', laundry_impact: 'low' });
    const data = applyFilters(
      makeDataset([high, low], [
        makeOutfit('2025-01-01', [high]),
        makeOutfit('2025-01-02', [high]),
        makeOutfit('2025-01-03', [high]),
        makeOutfit('2025-01-04', [low]),
      ]),
      noFilters,
    );
    const out = textInsights(data);
    const sus = out.find(i => i.category === 'sustainability');
    expect(sus).toBeTruthy();
    expect(sus!.text).toContain('75%');
  });
});

// ---------------------------------------------------------------------------
// Per-widget range + item resolution helpers
// ---------------------------------------------------------------------------

describe('presetRange / rangeData / widgetRange', () => {
  it('presetRange resolves relative to today', () => {
    expect(presetRange('all')).toEqual({ from: null, to: null });
    const r = presetRange('30d');
    expect(r.from).toBe(daysAgo(29));
    expect(r.to).toBe(daysAgo(0));
  });

  it('rangeData narrows outfits to the preset window', () => {
    const a = makeItem();
    const data = applyFilters(
      makeDataset([a], [makeOutfit(daysAgo(5), [a]), makeOutfit(daysAgo(100), [a])]),
      noFilters,
    );
    expect(data.outfits).toHaveLength(2);
    expect(rangeData(data, 'all').outfits).toHaveLength(2);
    expect(rangeData(data, '30d').outfits).toHaveLength(1);
    expect(rangeData(data, '1y').outfits).toHaveLength(2);
  });

  it('widgetRange reads the config value with a safe default', () => {
    expect(widgetRange({})).toBe('all');
    expect(widgetRange({ range: '90d' })).toBe('90d');
    expect(widgetRange({ range: 'bogus' })).toBe('all');
  });
});

describe('resolveItem', () => {
  const a = makeItem({ id: 'a', name: 'A' });
  const data = applyFilters(makeDataset([a], []), noFilters);

  it('returns null for empty or unknown ids', () => {
    expect(resolveItem(data, { focusItem: '' }, 'focusItem')).toBeNull();
    expect(resolveItem(data, { focusItem: 'missing' }, 'focusItem')).toBeNull();
  });

  it('returns the matching item', () => {
    expect(resolveItem(data, { focusItem: 'a' }, 'focusItem')?.id).toBe('a');
  });
});

describe('parseIdValue', () => {
  it('parses string ids and treats empty/invalid as null', () => {
    expect(parseIdValue('3')).toBe(3);
    expect(parseIdValue('')).toBeNull();
    expect(parseIdValue(undefined)).toBeNull();
    expect(parseIdValue('abc')).toBeNull();
    expect(parseIdValue('0')).toBeNull();
  });
});

describe('filterItemsByCategory', () => {
  const top = makeItem({ name: 'Shirt', category_id: 1, category_name: 'Tops', subcategory_id: 10, subcategory_name: 'Shirts' });
  const top2 = makeItem({ name: 'Jacket', category_id: 1, category_name: 'Tops', subcategory_id: 11, subcategory_name: 'Jackets' });
  const bottom = makeItem({ name: 'Jeans', category_id: 2, category_name: 'Bottoms', subcategory_id: 20, subcategory_name: 'Denim' });
  const data = applyFilters(makeDataset([top, top2, bottom], []), noFilters);

  it('returns everything when no filter is set', () => {
    expect(filterItemsByCategory(data, null, null)).toBe(data);
  });

  it('filters by category', () => {
    expect(filterItemsByCategory(data, 1, null).items.map(i => i.name)).toEqual(['Shirt', 'Jacket']);
  });

  it('filters by subcategory', () => {
    expect(filterItemsByCategory(data, null, 20).items.map(i => i.name)).toEqual(['Jeans']);
  });

  it('combines category and subcategory with AND', () => {
    expect(filterItemsByCategory(data, 1, 10).items.map(i => i.name)).toEqual(['Shirt']);
    // mismatched category + subcategory → empty
    expect(filterItemsByCategory(data, 1, 20).items).toHaveLength(0);
  });

  it('leaves outfits intact so wear counts stay global', () => {
    const worn = makeItem({ name: 'Worn Top', category_id: 1, category_name: 'Tops' });
    const d = applyFilters(
      makeDataset([worn, makeItem({ name: 'Other', category_id: 2 })], [makeOutfit('2025-01-01', [worn])]),
      noFilters,
    );
    const filtered = filterItemsByCategory(d, 1, null);
    expect(filtered.items).toHaveLength(1);
    expect(filtered.outfits).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// New stat-card metrics
// ---------------------------------------------------------------------------

describe('computeStat (new metrics)', () => {
  it('counts total wears and never-worn items', () => {
    const a = makeItem({ wear_count: 5 });
    const b = makeItem();
    const data = applyFilters(
      makeDataset([a, b], [makeOutfit('2025-01-01', [a]), makeOutfit('2025-01-02', [a])]),
      noFilters,
    );
    expect(computeStat(data, 'total_wears').value).toBe(5);
    expect(computeStat(data, 'never_worn').value).toBe(1);
  });

  it('counts dusty items using the threshold option', () => {
    const worn = makeItem();
    const dusty = makeItem();
    const data = applyFilters(
      makeDataset([worn, dusty], [makeOutfit(daysAgo(1), [worn]), makeOutfit(daysAgo(60), [dusty])]),
      noFilters,
    );
    expect(computeStat(data, 'dusty', { dustyDays: 30 }).value).toBe(1);
    expect(computeStat(data, 'dusty', { dustyDays: 90 }).value).toBe(0);
  });

  it('never_worn and dusty use fullData for wear history across a period window', () => {
    const a = makeItem();
    const full = applyFilters(makeDataset([a], [makeOutfit(daysAgo(60), [a])]), noFilters);
    const narrowed = rangeData(full, '30d');
    expect(narrowed.outfits).toHaveLength(0);
    // Without fullData the item looks never-worn inside the 30d window.
    expect(computeStat(narrowed, 'never_worn').value).toBe(1);
    expect(computeStat(narrowed, 'never_worn', { fullData: full }).value).toBe(0);
    // Worn 60 days ago: dusty at a 90d threshold only when history is visible.
    expect(computeStat(narrowed, 'dusty', { dustyDays: 90 }).value).toBe(1);
    expect(computeStat(narrowed, 'dusty', { dustyDays: 90, fullData: full }).value).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// Trend metric variants
// ---------------------------------------------------------------------------

describe('wearTrend metrics', () => {
  const a = makeItem();
  const b = makeItem();
  const data = applyFilters(
    makeDataset([a, b], [
      makeOutfit('2025-01-06', [a, b]),
      makeOutfit('2025-01-08', [a]),
    ]),
    noFilters,
  );

  it('counts unique items per bucket', () => {
    expect(wearTrend(data, 'week', 'items').map(p => p.value)).toEqual([2]);
  });

  it('computes items per outfit', () => {
    expect(wearTrend(data, 'week', 'items_per_outfit').map(p => p.value)).toEqual([1.5]);
  });
});

describe('dayOfWeek / seasonPattern metrics', () => {
  const a = makeItem();
  const b = makeItem();
  const data = applyFilters(
    makeDataset([a, b], [makeOutfit('2024-01-01', [a, b]), makeOutfit('2024-01-01', [a])]),
    noFilters,
  );

  it('dayOfWeek counts unique items', () => {
    const dow = dayOfWeek(data, 'items');
    expect(dow[0].value).toBe(2);
  });

  it('seasonPattern counts unique items per cell', () => {
    const { values } = seasonPattern(data, 'items');
    expect(values[0][0]).toBe(2);
  });
});

describe('topWorn per_month + minWears', () => {
  it('ranks by wears per month and honors minWears', () => {
    // Owned ~12 months ago vs ~1 month ago; same total wears → different rates.
    const old = makeItem({ name: 'Old', owned_since: '2020-01-01' });
    const fresh = makeItem({ name: 'Fresh', owned_since: dateKeyOf(addDays(new Date(), -31)) });
    const data = applyFilters(
      makeDataset([old, fresh], [
        makeOutfit('2025-01-01', [old]),
        makeOutfit('2025-01-02', [old]),
        makeOutfit('2025-01-03', [fresh]),
        makeOutfit('2025-01-04', [fresh]),
      ]),
      noFilters,
    );
    const byMonth = topWorn(data, 5, 'outfits', { metric: 'per_month' });
    expect(byMonth[0].name).toBe('Fresh');
    expect(byMonth[0].rate).toBeGreaterThan(byMonth[1].rate!);

    const filtered = topWorn(data, 5, 'outfits', { minWears: 3 });
    expect(filtered).toHaveLength(0);
  });

  it('uses fullData for the ownership fallback when dates are missing', () => {
    const a = makeItem({ name: 'A', created_at: '' });
    const full = applyFilters(
      makeDataset([a], [makeOutfit('2020-01-01', [a]), makeOutfit(daysAgo(5), [a])]),
      noFilters,
    );
    const narrowed = rangeData(full, '30d');
    // Without fullData the fallback window is the 30d period; with it, since 2020.
    const narrow = topWorn(narrowed, 5, 'outfits', { metric: 'per_month' })[0];
    const wide = topWorn(narrowed, 5, 'outfits', { metric: 'per_month', fullData: full })[0];
    expect(wide.rate!).toBeLessThan(narrow.rate!);
  });
});

describe('dustyItems options', () => {
  const never = makeItem({ name: 'Never', purchase_price: 100, owned_since: '2024-01-01' });
  const pricey = makeItem({ name: 'Pricey', purchase_price: 500, owned_since: '2023-01-01' });
  const cheap = makeItem({ name: 'Cheap', purchase_price: 10, owned_since: '2025-01-01' });
  const data = applyFilters(
    makeDataset([never, pricey, cheap], [makeOutfit(daysAgo(60), [pricey]), makeOutfit(daysAgo(60), [cheap])]),
    noFilters,
  );

  it('can exclude never-worn items', () => {
    expect(dustyItems(data, 30, { includeNeverWorn: false }).map(d => d.name)).toEqual(['Pricey', 'Cheap']);
  });

  it('sorts by price and by recency', () => {
    expect(dustyItems(data, 30, { sortBy: 'price' }).map(d => d.name)).toEqual(['Pricey', 'Never', 'Cheap']);
    expect(dustyItems(data, 30, { sortBy: 'recent' }).map(d => d.name)).toEqual(['Cheap', 'Never', 'Pricey']);
  });

  it('uses fullData for last-worn so a period window does not fake never-worn', () => {
    const a = makeItem({ name: 'A' });
    const full = applyFilters(makeDataset([a], [makeOutfit(daysAgo(60), [a])]), noFilters);
    const narrowed = rangeData(full, '30d');
    expect(narrowed.outfits).toHaveLength(0);
    expect(dustyItems(narrowed, 30).map(d => d.daysSince)).toEqual([null]);
    expect(dustyItems(narrowed, 30, { fullData: full }).map(d => d.daysSince)).toEqual([60]);
  });
});

// ---------------------------------------------------------------------------
// Breakdown metric + topN variants
// ---------------------------------------------------------------------------

describe('categoryBreakdown spend + topN', () => {
  const data = applyFilters(
    makeDataset(
      [
        makeItem({ category_name: 'Tops', purchase_price: 100 }),
        makeItem({ category_name: 'Tops', purchase_price: 50 }),
        makeItem({ category_name: 'Bottoms', purchase_price: 200 }),
        makeItem({ category_name: 'Shoes', purchase_price: 30 }),
      ],
      [],
    ),
    noFilters,
  );

  it('values groups by spend', () => {
    expect(categoryBreakdown(data, 'category', { metric: 'spend' })).toEqual([
      { label: 'Bottoms', value: 200 },
      { label: 'Tops', value: 150 },
      { label: 'Shoes', value: 30 },
    ]);
  });

  it('folds the remainder into an Other bucket', () => {
    expect(categoryBreakdown(data, 'category', { topN: 1 })).toEqual([
      { label: 'Tops', value: 2 },
      { label: 'Other', value: 2 },
    ]);
  });
});

describe('colorBreakdown topN', () => {
  it('folds the remainder into an Other bucket', () => {
    const data = applyFilters(
      makeDataset(
        [
          makeItem({ color_id: 'blue', color_hex: '#00f', purchase_price: 10 }),
          makeItem({ color_id: 'blue', color_hex: '#00f' }),
          makeItem({ color_id: 'red', color_hex: '#f00' }),
          makeItem({ color_id: 'green', color_hex: '#0f0' }),
        ],
        [],
      ),
      noFilters,
    );
    expect(colorBreakdown(data, { topN: 1 })).toEqual([
      { label: 'blue', hex: '#00f', value: 2 },
      { label: 'Other', hex: null, value: 2 },
    ]);
  });
});

describe('brandBreakdown includeNoBrand + avg', () => {
  const data = applyFilters(
    makeDataset(
      [
        makeItem({ brand_name: 'A', purchase_price: 100 }),
        makeItem({ brand_name: 'A', purchase_price: 50 }),
        makeItem({ purchase_price: 10 }),
      ],
      [],
    ),
    noFilters,
  );

  it('can drop the no-brand group', () => {
    expect(brandBreakdown(data, 5, { includeNoBrand: false })).toEqual([{ label: 'A', value: 2 }]);
  });

  it('computes average price per brand', () => {
    expect(brandBreakdown(data, 5, { metric: 'avg' }).find(r => r.label === 'A')?.value).toBe(75);
  });
});

describe('wardrobeAge spend', () => {
  it('values years by spend', () => {
    const data = applyFilters(
      makeDataset(
        [makeItem({ owned_since: '2022-05-01', purchase_price: 100 }), makeItem({ owned_since: '2023-05-01', purchase_price: 50 })],
        [],
      ),
      noFilters,
    );
    expect(wardrobeAge(data, 'spend')).toEqual([
      { label: '2022', value: 100 },
      { label: '2023', value: 50 },
    ]);
  });
});

// ---------------------------------------------------------------------------
// Value & cost options
// ---------------------------------------------------------------------------

describe('spendStats scope', () => {
  it('filters by acquisition year', () => {
    const thisYear = new Date().getUTCFullYear();
    const data = applyFilters(
      makeDataset(
        [
          makeItem({ owned_since: `${thisYear}-01-01`, purchase_price: 100 }),
          makeItem({ owned_since: `${thisYear - 1}-06-01`, purchase_price: 50 }),
        ],
        [],
      ),
      noFilters,
    );
    expect(spendStats(data, 'this_year').total).toBe(100);
    expect(spendStats(data, 'last_year').total).toBe(50);
    expect(spendStats(data, 'all').total).toBe(150);
  });
});

describe('costPerWear options', () => {
  const a = makeItem({ name: 'A', purchase_price: 10, wear_count: 10 });
  const b = makeItem({ name: 'B', purchase_price: 200, wear_count: 1 });
  const data = applyFilters(
    makeDataset([a, b], [makeOutfit('2025-01-01', [a]), makeOutfit('2025-01-02', [a])]),
    noFilters,
  );

  it('filters by minWears', () => {
    expect(costPerWear(data, 'best', 5, { minWears: 2, source: 'wear_count' }).map(r => r.name)).toEqual(['A']);
  });

  it('uses the selected wear source', () => {
    // Logged outfits: A=2, B=0 → B excluded. Wear count: A=10, B=1 → both included.
    expect(costPerWear(data, 'best', 5, { source: 'outfits' }).map(r => r.name)).toEqual(['A']);
    expect(costPerWear(data, 'best', 5, { source: 'wear_count' }).map(r => r.name)).toEqual(['A', 'B']);
  });
});

describe('cpwDetail', () => {
  const a = makeItem({ name: 'A', purchase_price: 10 });
  const b = makeItem({ name: 'B', purchase_price: 200 });
  const data = applyFilters(
    makeDataset([a, b], [
      makeOutfit('2025-01-01', [a, b]),
      makeOutfit('2025-01-02', [a]),
      makeOutfit('2025-01-03', [a]),
    ]),
    noFilters,
  );

  it('returns the item with rank, total and wardrobe average', () => {
    const d = cpwDetail(data, a.id);
    expect(d?.cpw).toBeCloseTo(10 / 3);
    expect(d?.rank).toBe(1);
    expect(d?.total).toBe(2);
    // spend 210, wears 4 → 52.5
    expect(d?.average).toBeCloseTo(52.5);
  });

  it('returns null for unknown or unpriced items', () => {
    expect(cpwDetail(data, 'missing')).toBeNull();
  });
});

describe('wardrobeCpwAverage', () => {
  it('is total spend over total wears for priced items', () => {
    const a = makeItem({ purchase_price: 10 });
    const b = makeItem({ purchase_price: 200 });
    const data = applyFilters(
      makeDataset([a, b, makeItem()], [
        makeOutfit('2025-01-01', [a, b]),
        makeOutfit('2025-01-02', [a]),
        makeOutfit('2025-01-03', [a]),
      ]),
      noFilters,
    );
    // spend 210, wears 4 → 52.5
    expect(wardrobeCpwAverage(data)).toBeCloseTo(52.5);
  });

  it('is null with no wears', () => {
    const data = applyFilters(makeDataset([makeItem({ purchase_price: 10 })], []), noFilters);
    expect(wardrobeCpwAverage(data)).toBeNull();
  });

  it('excludes zero-priced items from the average', () => {
    const a = makeItem({ purchase_price: 10 });
    const zero = makeItem({ purchase_price: 0 });
    const data = applyFilters(
      makeDataset([a, zero], [
        makeOutfit('2025-01-01', [a, zero]),
        makeOutfit('2025-01-02', [a]),
        makeOutfit('2025-01-03', [zero]),
      ]),
      noFilters,
    );
    // spend 10 over the priced item's 2 wears → 5 (zero-priced wears ignored).
    expect(wardrobeCpwAverage(data)).toBeCloseTo(5);
  });
});

describe('expensiveDusty sortBy + topN', () => {
  const cheap = makeItem({ name: 'Cheap', purchase_price: 100 });
  const pricey = makeItem({ name: 'Pricey', purchase_price: 300 });
  const worn = makeItem({ name: 'Worn', purchase_price: 200 });
  const data = applyFilters(
    makeDataset([cheap, pricey, worn], [makeOutfit('2025-01-01', [worn])]),
    noFilters,
  );

  it('sorts by fewest wears and worst cpw', () => {
    // 0-wear items tie, broken by price descending (Pricey 300 > Cheap 100).
    expect(expensiveDusty(data, 50, 1, { sortBy: 'wears' }).map(r => r.name)).toEqual(['Pricey', 'Cheap', 'Worn']);
    expect(expensiveDusty(data, 50, 1, { sortBy: 'cpw' }).map(r => r.name)).toEqual(['Pricey', 'Cheap', 'Worn']);
  });

  it('limits to topN', () => {
    expect(expensiveDusty(data, 50, 1, { topN: 2 })).toHaveLength(2);
  });
});

// ---------------------------------------------------------------------------
// Sustainability options
// ---------------------------------------------------------------------------

describe('laundryMix includeNotSet', () => {
  it('can drop the not-set group', () => {
    const data = applyFilters(
      makeDataset([makeItem({ laundry_impact: 'low' }), makeItem()], []),
      noFilters,
    );
    expect(laundryMix(data, { includeNotSet: false })).toEqual([{ label: 'Low', value: 1 }]);
  });
});

describe('impactHotspots options', () => {
  const high = makeItem({ name: 'High', laundry_impact: 'high' });
  const med = makeItem({ name: 'Med', laundry_impact: 'medium' });
  const data = applyFilters(
    makeDataset([high, med], [makeOutfit('2025-01-01', [high, med]), makeOutfit('2025-01-02', [high])]),
    noFilters,
  );

  it('includes medium impact only when asked', () => {
    expect(impactHotspots(data, 5).map(r => r.name)).toEqual(['High']);
    expect(impactHotspots(data, 5, { includeMedium: true }).map(r => r.name)).toEqual(['High', 'Med']);
  });

  it('filters by minWears', () => {
    // High has 2 wears, Med has 1.
    expect(impactHotspots(data, 5, { minWears: 2 }).map(r => r.name)).toEqual(['High']);
    expect(impactHotspots(data, 5, { minWears: 3 })).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// Relationship options
// ---------------------------------------------------------------------------

describe('itemPairings focus + minCount', () => {
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

  it('focuses on pairs containing one item', () => {
    const pairs = itemPairings(data, 5, { focusItemId: a.id });
    expect(pairs).toHaveLength(2);
    for (const p of pairs) expect(p.aId === a.id || p.bId === a.id).toBe(true);
  });

  it('drops pairs below minCount', () => {
    // a-b and a-c each appear twice; b-c only once.
    expect(itemPairings(data, 5, { minCount: 2 })).toHaveLength(2);
    expect(itemPairings(data, 5, { minCount: 3 })).toHaveLength(0);
  });
});

describe('repeatWears topN', () => {
  it('limits the result length', () => {
    const a = makeItem({ name: 'A' });
    const b = makeItem({ name: 'B' });
    const data = applyFilters(
      makeDataset([a, b], [
        makeOutfit('2025-01-01', [a]),
        makeOutfit('2025-01-03', [a]),
        makeOutfit('2025-01-01', [b]),
        makeOutfit('2025-01-02', [b]),
        makeOutfit('2025-01-03', [b]),
      ]),
      noFilters,
    );
    expect(repeatWears(data, 7, 1)).toHaveLength(1);
    expect(repeatWears(data, 7, 5)).toHaveLength(2);
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
