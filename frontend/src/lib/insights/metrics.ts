import type {
  EnrichedItem,
  EnrichedOutfit,
  FilteredData,
  GlobalFilters,
  InsightDataset,
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

function resolveRange(f: GlobalFilters): { from: string | null; to: string | null } {
  if (f.rangePreset === 'all') return { from: null, to: null };
  if (f.rangePreset === 'custom') return { from: f.from, to: f.to };
  const days = f.rangePreset === '30d' ? 30 : f.rangePreset === '90d' ? 90 : 365;
  return { from: dateKeyOf(addDays(new Date(), -(days - 1))), to: dateKeyOf(new Date()) };
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
  | 'total_spend'
  | 'avg_price'
  | 'cost_per_wear'
  | 'items_per_outfit'
  | 'current_streak';

export interface StatResult {
  value: number | null;
  format: 'int' | 'price' | 'decimal';
}

export function computeStat(data: FilteredData, metric: StatMetric): StatResult {
  switch (metric) {
    case 'total_items':
      return { value: data.items.length, format: 'int' };
    case 'active_items':
      return { value: data.items.filter(i => i.is_active).length, format: 'int' };
    case 'total_outfits':
      return { value: data.outfits.length, format: 'int' };
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

export function wearTrend(data: FilteredData, granularity: 'week' | 'month'): TrendPoint[] {
  if (data.outfits.length === 0) return [];
  const keys = data.outfits.map(o => o.dateKey).sort();
  const first = keys[0];
  const last = keys[keys.length - 1];

  if (granularity === 'month') {
    const map = new Map<string, number>();
    for (const o of data.outfits) {
      const ym = o.dateKey.slice(0, 7);
      map.set(ym, (map.get(ym) ?? 0) + 1);
    }
    const [sy, sm] = first.slice(0, 7).split('-').map(Number);
    const [ey, em] = last.slice(0, 7).split('-').map(Number);
    const out: TrendPoint[] = [];
    let y = sy;
    let m = sm;
    while (y < ey || (y === ey && m <= em)) {
      const ym = `${y}-${String(m).padStart(2, '0')}`;
      out.push({ label: formatMonthLabel(ym), value: map.get(ym) ?? 0 });
      m++;
      if (m > 12) {
        m = 1;
        y++;
      }
    }
    return out;
  }

  const map = new Map<string, number>();
  for (const o of data.outfits) {
    const k = mondayOfKey(o.dateKey);
    map.set(k, (map.get(k) ?? 0) + 1);
  }
  const out: TrendPoint[] = [];
  for (let d = parseKey(mondayOfKey(first)); d <= parseKey(mondayOfKey(last)); d = addDays(d, 7)) {
    const k = dateKeyOf(d);
    out.push({ label: shortDateLabel(k), value: map.get(k) ?? 0 });
  }
  return out;
}

export interface RankedItem {
  id: string;
  name: string;
  image_path: string | null;
  count: number;
}

export function topWorn(data: FilteredData, n: number, source: 'outfits' | 'wear_count'): RankedItem[] {
  const wearMap = outfitWearMap(data);
  return data.items
    .map(i => ({
      id: i.id,
      name: i.name,
      image_path: i.image_path,
      count: source === 'outfits' ? (wearMap.get(i.id) ?? 0) : (i.wear_count ?? 0),
    }))
    .filter(i => i.count > 0)
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
    .slice(0, n);
}

export interface DustyItem {
  id: string;
  name: string;
  image_path: string | null;
  /** Days since last worn; null = never worn in logged outfits. */
  daysSince: number | null;
}

export function dustyItems(data: FilteredData, thresholdDays: number): DustyItem[] {
  const lastWorn = lastWornMap(data);
  const today = todayKey();
  const rows: DustyItem[] = [];
  for (const i of data.items) {
    const last = lastWorn.get(i.id);
    const daysSince = last == null ? null : daysBetween(last, today);
    if (daysSince == null || daysSince >= thresholdDays) {
      rows.push({ id: i.id, name: i.name, image_path: i.image_path, daysSince });
    }
  }
  rows.sort((a, b) =>
    a.daysSince == null ? -1 : b.daysSince == null ? 1 : b.daysSince - a.daysSince,
  );
  return rows;
}

export function dayOfWeek(data: FilteredData): TrendPoint[] {
  const counts = new Array<number>(7).fill(0);
  for (const o of data.outfits) {
    const d = parseKey(o.dateKey);
    counts[(d.getUTCDay() + 6) % 7]++;
  }
  return WEEKDAY_LABELS.map((label, i) => ({ label, value: counts[i] }));
}

/** Month (rows) x weekday (cols, Mon-first) outfit counts for a heatmap. */
export function seasonPattern(data: FilteredData): { values: number[][]; max: number } {
  const values = Array.from({ length: 12 }, () => new Array<number>(7).fill(0));
  for (const o of data.outfits) {
    const d = parseKey(o.dateKey);
    values[d.getUTCMonth()][(d.getUTCDay() + 6) % 7]++;
  }
  const max = values.flat().reduce((a, b) => Math.max(a, b), 0);
  return { values, max };
}

// ---------------------------------------------------------------------------
// Wardrobe composition
// ---------------------------------------------------------------------------

export function categoryBreakdown(data: FilteredData, level: 'category' | 'subcategory'): TrendPoint[] {
  const map = new Map<string, number>();
  for (const i of data.items) {
    const label =
      level === 'category'
        ? (i.category_name ?? 'Uncategorized')
        : (i.subcategory_name ?? i.category_name ?? 'Uncategorized');
    map.set(label, (map.get(label) ?? 0) + 1);
  }
  return [...map.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value || a.label.localeCompare(b.label));
}

export interface ColorSlice extends TrendPoint {
  hex: string | null;
}

export function colorBreakdown(data: FilteredData): ColorSlice[] {
  const map = new Map<string, { hex: string | null; value: number }>();
  for (const i of data.items) {
    const label = i.color_id ?? 'No color';
    const cur = map.get(label) ?? { hex: i.color_hex, value: 0 };
    cur.value++;
    map.set(label, cur);
  }
  return [...map.entries()]
    .map(([label, v]) => ({ label, hex: v.hex, value: v.value }))
    .sort((a, b) => b.value - a.value || a.label.localeCompare(b.label));
}

export function brandBreakdown(data: FilteredData, n: number): TrendPoint[] {
  const map = new Map<string, number>();
  for (const i of data.items) {
    const label = i.brand_name ?? 'No brand';
    map.set(label, (map.get(label) ?? 0) + 1);
  }
  return [...map.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value || a.label.localeCompare(b.label))
    .slice(0, n);
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

/** Item counts grouped by year acquired (owned_since, falling back to created_at). */
export function wardrobeAge(data: FilteredData): TrendPoint[] {
  const map = new Map<number, number>();
  for (const i of data.items) {
    const src = i.owned_since ?? i.created_at;
    if (!src) continue;
    const y = new Date(src).getUTCFullYear();
    if (Number.isNaN(y)) continue;
    map.set(y, (map.get(y) ?? 0) + 1);
  }
  return [...map.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([year, value]) => ({ label: String(year), value }));
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

export function spendStats(data: FilteredData): SpendStats {
  const prices = data.items
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

export function costPerWear(data: FilteredData, direction: 'best' | 'worst', n: number): CpwEntry[] {
  const wearMap = outfitWearMap(data);
  const rows = data.items
    .filter(i => i.purchase_price != null && i.purchase_price > 0)
    .map(i => {
      const price = i.purchase_price as number;
      const wears = itemWears(i, wearMap);
      return {
        id: i.id,
        name: i.name,
        image_path: i.image_path,
        price,
        wears,
        cpw: wears > 0 ? price / wears : Number.POSITIVE_INFINITY,
      };
    })
    .filter(r => Number.isFinite(r.cpw));
  rows.sort(direction === 'best' ? (a, b) => a.cpw - b.cpw : (a, b) => b.cpw - a.cpw);
  return rows.slice(0, n);
}

export interface ExpensiveDustyEntry {
  id: string;
  name: string;
  image_path: string | null;
  price: number;
  wears: number;
}

export function expensiveDusty(data: FilteredData, minPrice: number, maxWears: number): ExpensiveDustyEntry[] {
  const wearMap = outfitWearMap(data);
  return data.items
    .filter(i => (i.purchase_price ?? 0) >= minPrice && itemWears(i, wearMap) <= maxWears)
    .map(i => ({
      id: i.id,
      name: i.name,
      image_path: i.image_path,
      price: i.purchase_price ?? 0,
      wears: itemWears(i, wearMap),
    }))
    .sort((a, b) => b.price - a.price);
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

export function laundryMix(data: FilteredData): TrendPoint[] {
  const map = new Map<string, number>();
  for (const i of data.items) {
    const label = i.laundry_impact ? normalizeImpact(i.laundry_impact) : 'Not set';
    map.set(label, (map.get(label) ?? 0) + 1);
  }
  return [...map.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((a, b) => b.value - a.value || a.label.localeCompare(b.label));
}

export interface ImpactEntry {
  id: string;
  name: string;
  image_path: string | null;
  impact: string;
  wears: number;
}

export function impactHotspots(data: FilteredData, n: number): ImpactEntry[] {
  const wearMap = outfitWearMap(data);
  return data.items
    .filter(i => i.laundry_impact != null && /high/i.test(i.laundry_impact))
    .map(i => ({
      id: i.id,
      name: i.name,
      image_path: i.image_path,
      impact: normalizeImpact(i.laundry_impact as string),
      wears: itemWears(i, wearMap),
    }))
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

/** Most frequent item pairs worn together in the same outfit. */
export function itemPairings(data: FilteredData, n: number): PairEntry[] {
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
  return [...counts.values()].sort((x, y) => y.count - x.count).slice(0, n);
}

export interface RepeatEntry {
  id: string;
  name: string;
  image_path: string | null;
  repeats: number;
}

/** Items worn again within `windowDays` of a previous wear, ranked by repeat count. */
export function repeatWears(data: FilteredData, windowDays: number): RepeatEntry[] {
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
  return rows.sort((a, b) => b.repeats - a.repeats || a.name.localeCompare(b.name));
}

// ---------------------------------------------------------------------------
// Generated text insights
// ---------------------------------------------------------------------------

export function textInsights(data: FilteredData): string[] {
  const { items, outfits } = data;
  if (items.length === 0) return ['Add clothing items to start seeing insights.'];
  if (outfits.length === 0) return ['Log outfits in the Outfits tab to unlock usage insights.'];

  const out: string[] = [];
  const wearMap = outfitWearMap(data);
  const totalWears = [...wearMap.values()].reduce((a, b) => a + b, 0);

  const neverWorn = items.filter(i => (wearMap.get(i.id) ?? 0) === 0).length;
  if (neverWorn > 0) {
    out.push(`${neverWorn} item${neverWorn === 1 ? ' is' : 's are'} never worn in your logged outfits.`);
  }

  const top = topWorn(data, 1, 'outfits')[0];
  if (top) out.push(`Most worn: ${top.name} (${top.count}\u00d7).`);

  if (totalWears > 0) {
    const counts = items.map(i => wearMap.get(i.id) ?? 0).sort((a, b) => b - a);
    const topCount = Math.max(1, Math.ceil(items.length * 0.2));
    const share = counts.slice(0, topCount).reduce((a, b) => a + b, 0) / totalWears;
    if (share >= 0.5) {
      out.push(
        `Top ${Math.round((topCount / items.length) * 100)}% of items account for ${Math.round(share * 100)}% of wears.`,
      );
    }
  }

  const best = costPerWear(data, 'best', 1)[0];
  if (best) out.push(`Best value: ${best.name} at ${formatPrice(best.cpw)} per wear.`);

  const mismatch = items.filter(i => {
    const logged = wearMap.get(i.id) ?? 0;
    const manual = i.wear_count ?? 0;
    return Math.abs(logged - manual) > Math.max(2, Math.round(Math.max(logged, manual) * 0.2));
  }).length;
  if (mismatch > 0) {
    out.push(`${mismatch} item${mismatch === 1 ? ' has' : 's have'} a wear count that doesn\u2019t match your logged outfits.`);
  }

  const dow = dayOfWeek(data);
  const maxDow = Math.max(...dow.map(d => d.value));
  if (maxDow > 0) {
    const busiest = dow.filter(d => d.value === maxDow).map(d => d.label);
    out.push(`You log outfits most on ${busiest.join(' and ')}.`);
  }

  const ed = expensiveDusty(data, 50, 0);
  if (ed.length > 0) {
    const sum = ed.reduce((a, e) => a + e.price, 0);
    out.push(
      `${ed.length} expensive item${ed.length === 1 ? '' : 's'} (total ${formatPrice(sum)}) have never been worn.`,
    );
  }

  return out.slice(0, 6);
}
