import type {
  EnrichedItem,
  EnrichedOutfit,
  FilteredData,
  GlobalFilters,
  InsightDataset,
  WidgetRange,
} from './types';
import { formatMonthLabel, formatPrice } from './format';

// ---------------------------------------------------------------------------
// Date helpers (UTC date keys, same convention as OutfitsView)
// ---------------------------------------------------------------------------

export function dateKeyOf(d: Date): string {
  return d.toISOString().slice(0, 10);
}

export function addDays(d: Date, n: number): Date {
  const c = new Date(d);
  c.setUTCDate(c.getUTCDate() + n);
  return c;
}

function parseKey(key: string): Date {
  const [y, m, d] = key.split('-').map(Number);
  return new Date(Date.UTC(y, m - 1, d));
}

export function daysBetween(a: string, b: string): number {
  return Math.round((parseKey(b).getTime() - parseKey(a).getTime()) / 86400000);
}

export function todayKey(): string {
  return dateKeyOf(new Date());
}

function mondayOfKey(key: string): string {
  const d = parseKey(key);
  const dow = (d.getUTCDay() + 6) % 7;
  return dateKeyOf(addDays(d, -dow));
}

const WEEKDAY_LABELS: string[] = Array.from({ length: 7 }, (_, i) =>
  new Date(Date.UTC(2024, 0, 1 + i)).toLocaleDateString(undefined, { weekday: 'short', timeZone: 'UTC' }),
);

export function shortDateLabel(key: string): string {
  return parseKey(key).toLocaleDateString(undefined, { month: 'short', day: 'numeric', timeZone: 'UTC' });
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

/** Resolve a per-widget range preset to concrete date bounds (relative to today). */
export function presetRange(preset: WidgetRange): { from: string | null; to: string | null } {
  if (preset === 'all') return { from: null, to: null };
  const days = preset === '30d' ? 30 : preset === '90d' ? 90 : 365;
  return { from: dateKeyOf(addDays(new Date(), -(days - 1))), to: dateKeyOf(new Date()) };
}

function resolveRange(f: GlobalFilters): { from: string | null; to: string | null } {
  if (f.rangePreset === 'all') return { from: null, to: null };
  if (f.rangePreset === 'custom') return { from: f.from, to: f.to };
  return presetRange(f.rangePreset);
}

/** Narrow a filtered dataset to a per-widget date range (intersects the global filter). */
export function rangeData(data: FilteredData, preset: WidgetRange): FilteredData {
  if (preset === 'all') return data;
  const { from, to } = presetRange(preset);
  return {
    ...data,
    outfits: data.outfits.filter(o => (from == null || o.dateKey >= from) && (to == null || o.dateKey <= to)),
  };
}

/** Read a per-widget range value from a widget config. */
export function widgetRange(config: Record<string, unknown>): WidgetRange {
  const v = config.range;
  return v === '30d' || v === '90d' || v === '1y' ? v : 'all';
}

/**
 * Resolve an item-select config value to a dataset item.
 * Returns null for '' (all items) or ids that no longer exist (deleted items).
 */
export function resolveItem(
  data: FilteredData,
  config: Record<string, unknown>,
  key: string,
): EnrichedItem | null {
  const id = config[key];
  if (typeof id !== 'string' || id === '') return null;
  return data.items.find(i => i.id === id) ?? null;
}

/** Parse a category/subcategory config value (string id) to a number, or null for "all". */
export function parseIdValue(value: unknown): number | null {
  if (typeof value === 'string' && value !== '') {
    const n = Number(value);
    if (Number.isFinite(n) && n > 0) return n;
  }
  return null;
}

/**
 * Filter items by category and/or subcategory. Outfits are left intact so that
 * wear counts for the remaining items stay global (a top's wears count every
 * outfit it appears in, regardless of what else was worn).
 */
export function filterItemsByCategory(
  data: FilteredData,
  categoryId: number | null,
  subcategoryId: number | null,
): FilteredData {
  if (categoryId == null && subcategoryId == null) return data;
  const match = (i: EnrichedItem) =>
    (categoryId == null || i.category_id === categoryId) &&
    (subcategoryId == null || i.subcategory_id === subcategoryId);
  return { ...data, items: data.items.filter(match) };
}

/**
 * Apply global filters. Item filters (category/color/brand/active) apply to
 * items and to the items within outfits; the date range applies to outfits.
 * Outfits with no matching items are dropped.
 */
export function applyFilters(ds: InsightDataset, f: GlobalFilters): FilteredData {
  const { from, to } = resolveRange(f);
  const matchItem = (it: EnrichedItem): boolean =>
    (f.categories.length === 0 || (it.category_id != null && f.categories.includes(it.category_id))) &&
    (f.colors.length === 0 || (it.color_id != null && f.colors.includes(it.color_id))) &&
    (f.brands.length === 0 || (it.brand_id != null && f.brands.includes(it.brand_id))) &&
    (!f.activeOnly || it.is_active);

  const items = ds.items.filter(matchItem);
  const outfits = ds.outfits
    .filter(o => (from == null || o.dateKey >= from) && (to == null || o.dateKey <= to))
    .map(o => ({ ...o, items: o.items.filter(matchItem) }))
    .filter(o => o.items.length > 0);

  return { items, outfits };
}

// ---------------------------------------------------------------------------
// Wear counting
// ---------------------------------------------------------------------------

/** How many (filtered) outfits each item appears in. */
export function outfitWearMap(data: FilteredData): Map<string, number> {
  const map = new Map<string, number>();
  for (const o of data.outfits) {
    for (const i of o.items) map.set(i.id, (map.get(i.id) ?? 0) + 1);
  }
  return map;
}

/** Latest outfit date key each item was worn on. */
export function lastWornMap(data: FilteredData): Map<string, string> {
  const map = new Map<string, string>();
  for (const o of data.outfits) {
    for (const i of o.items) {
      const prev = map.get(i.id);
      if (prev == null || o.dateKey > prev) map.set(i.id, o.dateKey);
    }
  }
  return map;
}

/**
 * Best estimate of total wears: the higher of logged outfit appearances and
 * the manually maintained wear_count (users often log outfits only
 * partially, so the manual count may be more complete).
 */
export function itemWears(item: EnrichedItem, wearMap: Map<string, number>): number {
  return Math.max(wearMap.get(item.id) ?? 0, item.wear_count ?? 0);
}

// ---------------------------------------------------------------------------
// Stat cards
// ---------------------------------------------------------------------------

export type StatMetric =
  | 'total_items'
  | 'active_items'
  | 'total_outfits'
  | 'total_wears'
  | 'never_worn'
  | 'dusty'
  | 'total_spend'
  | 'avg_price'
  | 'cost_per_wear'
  | 'items_per_outfit'
  | 'current_streak';

export interface StatResult {
  value: number | null;
  format: 'int' | 'price' | 'decimal';
}

export function computeStat(
  data: FilteredData,
  metric: StatMetric,
  opts: { dustyDays?: number; fullData?: FilteredData } = {},
): StatResult {
  switch (metric) {
    case 'total_items':
      return { value: data.items.length, format: 'int' };
    case 'active_items':
      return { value: data.items.filter(i => i.is_active).length, format: 'int' };
    case 'total_outfits':
      return { value: data.outfits.length, format: 'int' };
    case 'total_wears': {
      const wearMap = outfitWearMap(data);
      return { value: data.items.reduce((a, i) => a + itemWears(i, wearMap), 0), format: 'int' };
    }
    case 'never_worn': {
      // Wear history comes from the full (un-narrowed) dataset so a period
      // window doesn't make items worn outside it look never-worn.
      const wearMap = outfitWearMap(opts.fullData ?? data);
      return { value: data.items.filter(i => (wearMap.get(i.id) ?? 0) === 0).length, format: 'int' };
    }
    case 'dusty':
      return { value: dustyItems(data, opts.dustyDays ?? 30, { fullData: opts.fullData }).length, format: 'int' };
    case 'total_spend': {
      const sum = data.items.reduce((a, i) => a + (i.purchase_price ?? 0), 0);
      return { value: sum, format: 'price' };
    }
    case 'avg_price': {
      const prices = data.items.map(i => i.purchase_price).filter((p): p is number => p != null);
      if (prices.length === 0) return { value: null, format: 'price' };
      return { value: prices.reduce((a, b) => a + b, 0) / prices.length, format: 'price' };
    }
    case 'cost_per_wear': {
      const wearMap = outfitWearMap(data);
      const priced = data.items.filter(i => i.purchase_price != null);
      const spend = priced.reduce((a, i) => a + (i.purchase_price ?? 0), 0);
      const wears = priced.reduce((a, i) => a + itemWears(i, wearMap), 0);
      return wears > 0 ? { value: spend / wears, format: 'price' } : { value: null, format: 'price' };
    }
    case 'items_per_outfit': {
      if (data.outfits.length === 0) return { value: null, format: 'decimal' };
      const total = data.outfits.reduce((a, o) => a + o.items.length, 0);
      return { value: total / data.outfits.length, format: 'decimal' };
    }
    case 'current_streak':
      return { value: currentStreak(data.outfits), format: 'int' };
  }
}

/** Consecutive days (ending today, or yesterday if today isn't logged) with at least one outfit. */
export function currentStreak(outfits: EnrichedOutfit[]): number {
  const days = new Set(outfits.map(o => o.dateKey));
  if (days.size === 0) return 0;
  let d = new Date();
  if (!days.has(dateKeyOf(d))) d = addDays(d, -1);
  let streak = 0;
  while (days.has(dateKeyOf(d))) {
    streak++;
    d = addDays(d, -1);
  }
  return streak;
}

// ---------------------------------------------------------------------------
// Usage widgets
// ---------------------------------------------------------------------------

export interface TrendPoint {
  label: string;
  value: number;
}

/** What to count per period: outfits logged, unique items worn, or items per outfit. */
export type TrendMetric = 'outfits' | 'items' | 'items_per_outfit';

export function wearTrend(
  data: FilteredData,
  granularity: 'week' | 'month',
  metric: TrendMetric = 'outfits',
): TrendPoint[] {
  if (data.outfits.length === 0) return [];
  const keys = data.outfits.map(o => o.dateKey).sort();
  const first = keys[0];
  const last = keys[keys.length - 1];

  const bucketOf = (key: string) => (granularity === 'month' ? key.slice(0, 7) : mondayOfKey(key));
  const agg = new Map<string, { outfits: number; slots: number; items: Set<string> }>();
  for (const o of data.outfits) {
    const k = bucketOf(o.dateKey);
    const cur = agg.get(k) ?? { outfits: 0, slots: 0, items: new Set<string>() };
    cur.outfits++;
    cur.slots += o.items.length;
    for (const i of o.items) cur.items.add(i.id);
    agg.set(k, cur);
  }
  const valueOf = (k: string): number => {
    const c = agg.get(k);
    if (!c) return 0;
    if (metric === 'items') return c.items.size;
    if (metric === 'items_per_outfit') return c.outfits > 0 ? c.slots / c.outfits : 0;
    return c.outfits;
  };

  if (granularity === 'month') {
    const [sy, sm] = first.slice(0, 7).split('-').map(Number);
    const [ey, em] = last.slice(0, 7).split('-').map(Number);
    const out: TrendPoint[] = [];
    let y = sy;
    let m = sm;
    while (y < ey || (y === ey && m <= em)) {
      const ym = `${y}-${String(m).padStart(2, '0')}`;
      out.push({ label: formatMonthLabel(ym), value: valueOf(ym) });
      m++;
      if (m > 12) {
        m = 1;
        y++;
      }
    }
    return out;
  }

  const out: TrendPoint[] = [];
  for (let d = parseKey(mondayOfKey(first)); d <= parseKey(mondayOfKey(last)); d = addDays(d, 7)) {
    const k = dateKeyOf(d);
    out.push({ label: shortDateLabel(k), value: valueOf(k) });
  }
  return out;
}

export interface RankedItem {
  id: string;
  name: string;
  image_path: string | null;
  count: number;
  /** Wears per month (normalized by ownership duration); null when metric is 'total'. */
  rate: number | null;
}

export type TopWornMetric = 'total' | 'per_month';

export function topWorn(
  data: FilteredData,
  n: number,
  source: 'outfits' | 'wear_count',
  opts: { metric?: TopWornMetric; minWears?: number; fullData?: FilteredData } = {},
): RankedItem[] {
  const metric = opts.metric ?? 'total';
  const minWears = opts.minWears ?? 0;
  const wearMap = outfitWearMap(data);
  // Ownership-duration fallback uses the full (un-narrowed) dataset so a short
  // period window doesn't inflate wears-per-month rates.
  const history = opts.fullData ?? data;
  const oldestOutfit = history.outfits.length > 0 ? [...history.outfits.map(o => o.dateKey)].sort()[0] : null;
  const now = todayKey();
  const monthsOwned = (i: EnrichedItem): number => {
    const src = i.owned_since ?? (i.created_at ? i.created_at.slice(0, 10) : null) ?? oldestOutfit;
    if (!src) return 1;
    const days = Math.max(0, daysBetween(src.slice(0, 10), now));
    return Math.max(1, days / 30.44);
  };
  return data.items
    .map(i => {
      const count = source === 'outfits' ? (wearMap.get(i.id) ?? 0) : (i.wear_count ?? 0);
      return {
        id: i.id,
        name: i.name,
        image_path: i.image_path,
        count,
        rate: metric === 'per_month' ? count / monthsOwned(i) : null,
      };
    })
    .filter(i => i.count > 0 && i.count >= minWears)
    .sort(
      (a, b) =>
        (metric === 'per_month' ? (b.rate ?? 0) - (a.rate ?? 0) : b.count - a.count) ||
        a.name.localeCompare(b.name),
    )
    .slice(0, n);
}

export interface DustyItem {
  id: string;
  name: string;
  image_path: string | null;
  /** Days since last worn; null = never worn in logged outfits. */
  daysSince: number | null;
  price: number | null;
  /** Acquisition date key (owned_since or created_at), for sorting. */
  owned: string | null;
}

export interface DustyOptions {
  includeNeverWorn?: boolean;
  sortBy?: 'days' | 'price' | 'recent';
  /**
   * Dataset (typically not period-narrowed) used to determine last-worn dates,
   * so items worn outside the period aren't reported as never worn.
   */
  fullData?: FilteredData;
}

export function dustyItems(data: FilteredData, thresholdDays: number, opts: DustyOptions = {}): DustyItem[] {
  const includeNeverWorn = opts.includeNeverWorn ?? true;
  const sortBy = opts.sortBy ?? 'days';
  const lastWorn = lastWornMap(opts.fullData ?? data);
  const today = todayKey();
  const rows: DustyItem[] = [];
  for (const i of data.items) {
    const last = lastWorn.get(i.id);
    const daysSince = last == null ? null : daysBetween(last, today);
    if (daysSince == null ? includeNeverWorn : daysSince >= thresholdDays) {
      rows.push({
        id: i.id,
        name: i.name,
        image_path: i.image_path,
        daysSince,
        price: i.purchase_price,
        owned: i.owned_since ?? (i.created_at ? i.created_at.slice(0, 10) : null),
      });
    }
  }
  rows.sort((a, b) => {
    if (sortBy === 'price') return (b.price ?? -1) - (a.price ?? -1) || a.name.localeCompare(b.name);
    if (sortBy === 'recent') return (b.owned ?? '').localeCompare(a.owned ?? '') || a.name.localeCompare(b.name);
    return a.daysSince == null ? -1 : b.daysSince == null ? 1 : b.daysSince - a.daysSince;
  });
  return rows;
}

export function dayOfWeek(data: FilteredData, metric: TrendMetric = 'outfits'): TrendPoint[] {
  const counts = new Array<number>(7).fill(0);
  const itemSets: Set<string>[] = Array.from({ length: 7 }, () => new Set<string>());
  for (const o of data.outfits) {
    const idx = (parseKey(o.dateKey).getUTCDay() + 6) % 7;
    counts[idx]++;
    for (const i of o.items) itemSets[idx].add(i.id);
  }
  return WEEKDAY_LABELS.map((label, i) => ({
    label,
    value:
      metric === 'items'
        ? itemSets[i].size
        : metric === 'items_per_outfit'
          ? (counts[i] > 0 ? itemSets[i].size / counts[i] : 0)
          : counts[i],
  }));
}

/** Month (rows) x weekday (cols, Mon-first) grid for a heatmap. */
export function seasonPattern(data: FilteredData, metric: TrendMetric = 'outfits'): { values: number[][]; max: number } {
  const values = Array.from({ length: 12 }, () => new Array<number>(7).fill(0));
  const itemSets: Set<string>[][] = Array.from({ length: 12 }, () =>
    Array.from({ length: 7 }, () => new Set<string>()),
  );
  for (const o of data.outfits) {
    const d = parseKey(o.dateKey);
    const m = d.getUTCMonth();
    const w = (d.getUTCDay() + 6) % 7;
    values[m][w]++;
    for (const i of o.items) itemSets[m][w].add(i.id);
  }
  if (metric !== 'outfits') {
    for (let m = 0; m < 12; m++) {
      for (let w = 0; w < 7; w++) {
        values[m][w] =
          metric === 'items'
            ? itemSets[m][w].size
            : values[m][w] > 0
              ? itemSets[m][w].size / values[m][w]
              : 0;
      }
    }
  }
  const max = values.flat().reduce((a, b) => Math.max(a, b), 0);
  return { values, max };
}

// ---------------------------------------------------------------------------
// Wardrobe composition
// ---------------------------------------------------------------------------

/** How to value each group: item count, total spend, or average price. */
export type BreakdownMetric = 'items' | 'spend' | 'avg';

interface BreakdownAcc {
  count: number;
  spend: number;
}

function breakdownValue(acc: BreakdownAcc, metric: BreakdownMetric): number {
  if (metric === 'spend') return acc.spend;
  if (metric === 'avg') return acc.count > 0 ? acc.spend / acc.count : 0;
  return acc.count;
}

interface BreakdownOptions {
  metric?: BreakdownMetric;
  /** Keep the top N groups; the remainder is folded into an "Other" bucket. 0/undefined = keep all. */
  topN?: number;
  skipLabel?: (label: string) => boolean;
}

function breakdownBy(
  data: FilteredData,
  labelOf: (i: EnrichedItem) => string,
  opts: BreakdownOptions = {},
): TrendPoint[] {
  const metric = opts.metric ?? 'items';
  const map = new Map<string, BreakdownAcc>();
  for (const i of data.items) {
    const label = labelOf(i);
    if (opts.skipLabel?.(label)) continue;
    const cur = map.get(label) ?? { count: 0, spend: 0 };
    cur.count++;
    cur.spend += i.purchase_price ?? 0;
    map.set(label, cur);
  }
  let rows = [...map.entries()].map(([label, acc]) => ({ label, acc, value: breakdownValue(acc, metric) }));
  rows.sort((a, b) => b.value - a.value || a.label.localeCompare(b.label));
  const n = opts.topN;
  if (n != null && n > 0 && rows.length > n) {
    const head = rows.slice(0, n);
    const rest = rows.slice(n);
    const restAcc: BreakdownAcc = {
      count: rest.reduce((s, r) => s + r.acc.count, 0),
      spend: rest.reduce((s, r) => s + r.acc.spend, 0),
    };
    head.push({ label: 'Other', acc: restAcc, value: breakdownValue(restAcc, metric) });
    rows = head;
  }
  return rows.map(({ label, value }) => ({ label, value }));
}

export function categoryBreakdown(
  data: FilteredData,
  level: 'category' | 'subcategory',
  opts: BreakdownOptions = {},
): TrendPoint[] {
  return breakdownBy(
    data,
    i =>
      level === 'category'
        ? (i.category_name ?? 'Uncategorized')
        : (i.subcategory_name ?? i.category_name ?? 'Uncategorized'),
    opts,
  );
}

export interface ColorSlice extends TrendPoint {
  hex: string | null;
}

export function colorBreakdown(data: FilteredData, opts: BreakdownOptions = {}): ColorSlice[] {
  const metric = opts.metric ?? 'items';
  const map = new Map<string, { hex: string | null; acc: BreakdownAcc }>();
  for (const i of data.items) {
    const label = i.color_id ?? 'No color';
    if (opts.skipLabel?.(label)) continue;
    const cur = map.get(label) ?? { hex: i.color_hex, acc: { count: 0, spend: 0 } };
    cur.acc.count++;
    cur.acc.spend += i.purchase_price ?? 0;
    map.set(label, cur);
  }
  let rows = [...map.entries()].map(([label, v]) => ({
    label,
    hex: v.hex,
    acc: v.acc,
    value: breakdownValue(v.acc, metric),
  }));
  rows.sort((a, b) => b.value - a.value || a.label.localeCompare(b.label));
  const n = opts.topN;
  if (n != null && n > 0 && rows.length > n) {
    const head = rows.slice(0, n);
    const rest = rows.slice(n);
    const restAcc: BreakdownAcc = {
      count: rest.reduce((s, r) => s + r.acc.count, 0),
      spend: rest.reduce((s, r) => s + r.acc.spend, 0),
    };
    head.push({ label: 'Other', hex: null, acc: restAcc, value: breakdownValue(restAcc, metric) });
    rows = head;
  }
  return rows.map(({ label, hex, value }) => ({ label, hex, value }));
}

export function brandBreakdown(
  data: FilteredData,
  n: number,
  opts: BreakdownOptions & { includeNoBrand?: boolean } = {},
): TrendPoint[] {
  const { includeNoBrand, ...rest } = opts;
  return breakdownBy(data, i => i.brand_name ?? 'No brand', {
    ...rest,
    topN: n,
    skipLabel: includeNoBrand === false ? l => l === 'No brand' : undefined,
  });
}

export function priceHistogram(data: FilteredData, bucketCount: number): TrendPoint[] {
  const prices = data.items.map(i => i.purchase_price).filter((p): p is number => p != null);
  if (prices.length === 0) return [];
  const min = Math.min(...prices);
  const max = Math.max(...prices);
  if (min === max) return [{ label: formatPrice(min), value: prices.length }];
  const n = Math.max(1, Math.min(bucketCount, prices.length));
  const width = (max - min) / n;
  const buckets = new Array<number>(n).fill(0);
  for (const p of prices) {
    let idx = Math.floor((p - min) / width);
    if (idx >= n) idx = n - 1;
    buckets[idx]++;
  }
  return buckets.map((value, idx) => ({
    label: `${Math.round(min + idx * width)}–${Math.round(min + (idx + 1) * width)}`,
    value,
  }));
}

/** Items grouped by year acquired (owned_since, falling back to created_at). */
export function wardrobeAge(data: FilteredData, metric: BreakdownMetric = 'items'): TrendPoint[] {
  const map = new Map<number, BreakdownAcc>();
  for (const i of data.items) {
    const src = i.owned_since ?? i.created_at;
    if (!src) continue;
    const y = new Date(src).getUTCFullYear();
    if (Number.isNaN(y)) continue;
    const cur = map.get(y) ?? { count: 0, spend: 0 };
    cur.count++;
    cur.spend += i.purchase_price ?? 0;
    map.set(y, cur);
  }
  return [...map.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([year, acc]) => ({ label: String(year), value: breakdownValue(acc, metric) }));
}

// ---------------------------------------------------------------------------
// Value & cost
// ---------------------------------------------------------------------------

export interface SpendStats {
  total: number;
  avg: number | null;
  median: number | null;
  count: number;
}

export type SpendScope = 'all' | 'this_year' | 'last_year';

export function spendStats(data: FilteredData, scope: SpendScope = 'all'): SpendStats {
  const items =
    scope === 'all'
      ? data.items
      : data.items.filter(i => {
          const year = new Date().getUTCFullYear();
          const src = i.owned_since ?? (i.created_at ? i.created_at.slice(0, 4) : null);
          if (!src) return false;
          const y = Number(src.slice(0, 4));
          return scope === 'this_year' ? y === year : y === year - 1;
        });
  const prices = items
    .map(i => i.purchase_price)
    .filter((p): p is number => p != null)
    .sort((a, b) => a - b);
  if (prices.length === 0) return { total: 0, avg: null, median: null, count: 0 };
  const total = prices.reduce((a, b) => a + b, 0);
  const median =
    prices.length % 2 === 1
      ? prices[(prices.length - 1) / 2]
      : (prices[prices.length / 2 - 1] + prices[prices.length / 2]) / 2;
  return { total, avg: total / prices.length, median, count: prices.length };
}

export interface CpwEntry {
  id: string;
  name: string;
  image_path: string | null;
  price: number;
  wears: number;
  cpw: number;
}

export type CpwSource = 'best' | 'outfits' | 'wear_count';

export interface CpwOptions {
  /** Exclude items with fewer wears than this (0 = no filter). */
  minWears?: number;
  /** Which wear count to use: best estimate (default), logged outfits, or the manual field. */
  source?: CpwSource;
}

function wearsOf(item: EnrichedItem, wearMap: Map<string, number>, source: CpwSource): number {
  const logged = wearMap.get(item.id) ?? 0;
  const manual = item.wear_count ?? 0;
  if (source === 'outfits') return logged;
  if (source === 'wear_count') return manual;
  return Math.max(logged, manual);
}

export function costPerWear(
  data: FilteredData,
  direction: 'best' | 'worst',
  n: number,
  opts: CpwOptions = {},
): CpwEntry[] {
  const source = opts.source ?? 'best';
  const minWears = opts.minWears ?? 0;
  const wearMap = outfitWearMap(data);
  const rows = data.items
    .filter(i => i.purchase_price != null && i.purchase_price > 0)
    .map(i => {
      const price = i.purchase_price as number;
      const wears = wearsOf(i, wearMap, source);
      return {
        id: i.id,
        name: i.name,
        image_path: i.image_path,
        price,
        wears,
        cpw: wears > 0 ? price / wears : Number.POSITIVE_INFINITY,
      };
    })
    .filter(r => Number.isFinite(r.cpw) && r.wears >= minWears);
  rows.sort(direction === 'best' ? (a, b) => a.cpw - b.cpw : (a, b) => b.cpw - a.cpw);
  return rows.slice(0, n);
}

export interface CpwDetail {
  id: string;
  name: string;
  image_path: string | null;
  price: number;
  wears: number;
  /** Null when the item has zero wears (no finite cost per wear). */
  cpw: number | null;
  /** 1-based position in the full best-value ranking; null when the item is unranked. */
  rank: number | null;
  total: number;
  /** Wardrobe-wide average cost per wear (total spend / total wears over priced items). */
  average: number | null;
}

/** Wardrobe-wide average cost per wear (total spend / total wears over priced items). */
export function wardrobeCpwAverage(data: FilteredData, source: CpwSource = 'best'): number | null {
  const wearMap = outfitWearMap(data);
  const priced = data.items.filter(i => i.purchase_price != null && i.purchase_price > 0);
  if (priced.length === 0) return null;
  const spend = priced.reduce((a, i) => a + (i.purchase_price ?? 0), 0);
  const totalWears = priced.reduce((a, i) => a + wearsOf(i, wearMap, source), 0);
  return totalWears > 0 ? spend / totalWears : null;
}

/** Cost-per-wear detail for a single focused item, with wardrobe context. */
export function cpwDetail(data: FilteredData, itemId: string, opts: CpwOptions = {}): CpwDetail | null {
  const source = opts.source ?? 'best';
  const wearMap = outfitWearMap(data);
  const item = data.items.find(i => i.id === itemId);
  if (!item || item.purchase_price == null) return null;
  const wears = wearsOf(item, wearMap, source);

  const ranked = data.items
    .filter(i => i.purchase_price != null && i.purchase_price > 0)
    .map(i => {
      const w = wearsOf(i, wearMap, source);
      return { id: i.id, cpw: w > 0 ? (i.purchase_price as number) / w : Number.POSITIVE_INFINITY };
    })
    .filter(r => Number.isFinite(r.cpw))
    .sort((a, b) => a.cpw - b.cpw);
  const idx = ranked.findIndex(r => r.id === itemId);

  return {
    id: item.id,
    name: item.name,
    image_path: item.image_path,
    price: item.purchase_price,
    wears,
    cpw: wears > 0 ? item.purchase_price / wears : null,
    rank: idx >= 0 ? idx + 1 : null,
    total: ranked.length,
    average: wardrobeCpwAverage(data, source),
  };
}

export interface ExpensiveDustyEntry {
  id: string;
  name: string;
  image_path: string | null;
  price: number;
  wears: number;
}

export type ExpensiveDustySort = 'price' | 'wears' | 'cpw';

export function expensiveDusty(
  data: FilteredData,
  minPrice: number,
  maxWears: number,
  opts: { sortBy?: ExpensiveDustySort; topN?: number } = {},
): ExpensiveDustyEntry[] {
  const sortBy = opts.sortBy ?? 'price';
  const wearMap = outfitWearMap(data);
  const rows = data.items
    .filter(i => (i.purchase_price ?? 0) >= minPrice && itemWears(i, wearMap) <= maxWears)
    .map(i => ({
      id: i.id,
      name: i.name,
      image_path: i.image_path,
      price: i.purchase_price ?? 0,
      wears: itemWears(i, wearMap),
    }));
  rows.sort((a, b) => {
    if (sortBy === 'wears') return a.wears - b.wears || b.price - a.price;
    if (sortBy === 'cpw') {
      const av = a.wears > 0 ? a.price / a.wears : Number.POSITIVE_INFINITY;
      const bv = b.wears > 0 ? b.price / b.wears : Number.POSITIVE_INFINITY;
      return bv - av || b.price - a.price;
    }
    return b.price - a.price;
  });
  const n = opts.topN;
  return n != null && n > 0 ? rows.slice(0, n) : rows;
}

// ---------------------------------------------------------------------------
// Sustainability
// ---------------------------------------------------------------------------

/** Normalize free-text laundry impact values into Low/Medium/High buckets. */
export function normalizeImpact(value: string): string {
  const s = value.trim().toLowerCase();
  if (/high/.test(s)) return 'High';
  if (/med/.test(s)) return 'Medium';
  if (/low/.test(s)) return 'Low';
  return value.trim();
}

export function laundryMix(
  data: FilteredData,
  opts: BreakdownOptions & { includeNotSet?: boolean } = {},
): TrendPoint[] {
  const { includeNotSet, ...rest } = opts;
  return breakdownBy(data, i => (i.laundry_impact ? normalizeImpact(i.laundry_impact) : 'Not set'), {
    ...rest,
    skipLabel: includeNotSet === false ? l => l === 'Not set' : undefined,
  });
}

export interface ImpactEntry {
  id: string;
  name: string;
  image_path: string | null;
  impact: string;
  wears: number;
}

export function impactHotspots(
  data: FilteredData,
  n: number,
  opts: { includeMedium?: boolean; minWears?: number } = {},
): ImpactEntry[] {
  const minWears = opts.minWears ?? 0;
  const wearMap = outfitWearMap(data);
  return data.items
    .filter(i => {
      if (i.laundry_impact == null) return false;
      const s = i.laundry_impact.toLowerCase();
      if (/high/.test(s)) return true;
      return (opts.includeMedium ?? false) && /med/.test(s);
    })
    .map(i => ({
      id: i.id,
      name: i.name,
      image_path: i.image_path,
      impact: normalizeImpact(i.laundry_impact as string),
      wears: itemWears(i, wearMap),
    }))
    .filter(e => e.wears >= minWears)
    .sort((a, b) => b.wears - a.wears || a.name.localeCompare(b.name))
    .slice(0, n);
}

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

export interface PairEntry {
  aId: string;
  aName: string;
  bId: string;
  bName: string;
  count: number;
}

/**
 * Most frequent item pairs worn together in the same outfit.
 * With `focusItemId`, only pairs containing that item are returned.
 */
export function itemPairings(
  data: FilteredData,
  n: number,
  opts: { focusItemId?: string | null; minCount?: number } = {},
): PairEntry[] {
  const minCount = opts.minCount ?? 1;
  const counts = new Map<string, PairEntry & { count: number }>();
  for (const o of data.outfits) {
    const items = o.items;
    for (let i = 0; i < items.length; i++) {
      for (let j = i + 1; j < items.length; j++) {
        const [a, b] = items[i].id < items[j].id ? [items[i], items[j]] : [items[j], items[i]];
        const key = `${a.id}|${b.id}`;
        const cur = counts.get(key);
        if (cur) {
          cur.count++;
        } else {
          counts.set(key, { aId: a.id, aName: a.name, bId: b.id, bName: b.name, count: 1 });
        }
      }
    }
  }
  let rows = [...counts.values()];
  if (opts.focusItemId) {
    rows = rows.filter(p => p.aId === opts.focusItemId || p.bId === opts.focusItemId);
  }
  return rows
    .filter(p => p.count >= minCount)
    .sort((x, y) => y.count - x.count)
    .slice(0, n);
}

export interface RepeatEntry {
  id: string;
  name: string;
  image_path: string | null;
  repeats: number;
}

/** Items worn again within `windowDays` of a previous wear, ranked by repeat count. */
export function repeatWears(data: FilteredData, windowDays: number, n?: number): RepeatEntry[] {
  const byId = new Map(data.items.map(i => [i.id, i]));
  const datesByItem = new Map<string, string[]>();
  for (const o of data.outfits) {
    for (const i of o.items) {
      const list = datesByItem.get(i.id);
      if (list) list.push(o.dateKey);
      else datesByItem.set(i.id, [o.dateKey]);
    }
  }
  const rows: RepeatEntry[] = [];
  for (const [id, dates] of datesByItem) {
    if (dates.length < 2) continue;
    dates.sort();
    let repeats = 0;
    for (let i = 0; i < dates.length; i++) {
      for (let j = i + 1; j < dates.length; j++) {
        if (daysBetween(dates[i], dates[j]) <= windowDays) repeats++;
        else break;
      }
    }
    if (repeats > 0) {
      const item = byId.get(id);
      rows.push({ id, name: item?.name ?? id, image_path: item?.image_path ?? null, repeats });
    }
  }
  const sorted = rows.sort((a, b) => b.repeats - a.repeats || a.name.localeCompare(b.name));
  return n != null && n > 0 ? sorted.slice(0, n) : sorted;
}

// ---------------------------------------------------------------------------
// Generated text insights
// ---------------------------------------------------------------------------

export type InsightCategory = 'usage' | 'value' | 'sustainability';

export interface TextInsight {
  text: string;
  category: InsightCategory;
}

export function textInsights(data: FilteredData): TextInsight[] {
  const { items, outfits } = data;
  if (items.length === 0) {
    return [{ text: 'Add clothing items to start seeing insights.', category: 'usage' }];
  }
  if (outfits.length === 0) {
    return [{ text: 'Log outfits in the Outfits tab to unlock usage insights.', category: 'usage' }];
  }

  const out: TextInsight[] = [];
  const wearMap = outfitWearMap(data);
  const totalWears = [...wearMap.values()].reduce((a, b) => a + b, 0);

  const neverWorn = items.filter(i => (wearMap.get(i.id) ?? 0) === 0).length;
  if (neverWorn > 0) {
    out.push({
      text: `${neverWorn} item${neverWorn === 1 ? ' is' : 's are'} never worn in your logged outfits.`,
      category: 'usage',
    });
  }

  const top = topWorn(data, 1, 'outfits')[0];
  if (top) out.push({ text: `Most worn: ${top.name} (${top.count}\u00d7).`, category: 'usage' });

  if (totalWears > 0) {
    const counts = items.map(i => wearMap.get(i.id) ?? 0).sort((a, b) => b - a);
    const topCount = Math.max(1, Math.ceil(items.length * 0.2));
    const share = counts.slice(0, topCount).reduce((a, b) => a + b, 0) / totalWears;
    if (share >= 0.5) {
      out.push({
        text: `Top ${Math.round((topCount / items.length) * 100)}% of items account for ${Math.round(share * 100)}% of wears.`,
        category: 'usage',
      });
    }
  }

  const best = costPerWear(data, 'best', 1)[0];
  if (best) {
    out.push({ text: `Best value: ${best.name} at ${formatPrice(best.cpw)} per wear.`, category: 'value' });
  }

  const mismatch = items.filter(i => {
    const logged = wearMap.get(i.id) ?? 0;
    const manual = i.wear_count ?? 0;
    return Math.abs(logged - manual) > Math.max(2, Math.round(Math.max(logged, manual) * 0.2));
  }).length;
  if (mismatch > 0) {
    out.push({
      text: `${mismatch} item${mismatch === 1 ? ' has' : 's have'} a wear count that doesn\u2019t match your logged outfits.`,
      category: 'usage',
    });
  }

  const dow = dayOfWeek(data);
  const maxDow = Math.max(...dow.map(d => d.value));
  if (maxDow > 0) {
    const busiest = dow.filter(d => d.value === maxDow).map(d => d.label);
    out.push({ text: `You log outfits most on ${busiest.join(' and ')}.`, category: 'usage' });
  }

  const ed = expensiveDusty(data, 50, 0);
  if (ed.length > 0) {
    const sum = ed.reduce((a, e) => a + e.price, 0);
    out.push({
      text: `${ed.length} expensive item${ed.length === 1 ? '' : 's'} (total ${formatPrice(sum)}) have never been worn.`,
      category: 'value',
    });
  }

  if (totalWears > 0) {
    const highWears = items
      .filter(i => i.laundry_impact != null && /high/i.test(i.laundry_impact))
      .reduce((a, i) => a + (wearMap.get(i.id) ?? 0), 0);
    const share = highWears / totalWears;
    if (share >= 0.2) {
      out.push({
        text: `High-impact items account for ${Math.round(share * 100)}% of your wears.`,
        category: 'sustainability',
      });
    }
  }

  return out;
}
