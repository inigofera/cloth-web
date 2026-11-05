/**
 * Pure helpers for the Outfits item picker. Kept free of Vue so they can be
 * unit-tested directly (see `picker.spec.ts`).
 */
import { formatLocalDateKey, parseOutfitDateKey } from './dates';

// Capture groups are required: the match array only exposes `m[1..n]` for
// parenthesized groups.
const DATE_KEY_RE = /^(\d{4})-(\d{2})-(\d{2})$/;

/** Minimal shape accepted by {@link buildLastWornMap}. */
export interface LastWornSourceOutfit {
  date: string;
  item_ids: string[];
}

/**
 * For each item id, the most recent date key on which it was logged in an
 * outfit. Outfits without a parseable date are ignored. Returns a new Map so
 * callers never mutate shared state.
 */
export function buildLastWornMap(outfits: readonly LastWornSourceOutfit[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const outfit of outfits) {
    const key = parseOutfitDateKey(outfit.date);
    if (!key) continue; // unparseable date: skip rather than guess
    for (const id of outfit.item_ids ?? []) {
      const current = map.get(id);
      if (current === undefined || key > current) {
        map.set(id, key);
      }
    }
  }
  return map;
}

/**
 * Sort a picker list so recently-worn items come first. Items that were never
 * logged sink to the bottom with a stable, case-insensitive name tiebreak.
 * Returns a new array; the input is not mutated.
 */
export function sortByRecency<T extends { id: string; name?: string }>(
  items: readonly T[],
  lastWorn: ReadonlyMap<string, string>,
): T[] {
  return [...items].sort((a, b) => {
    const aKey = lastWorn.get(a.id) ?? '';
    const bKey = lastWorn.get(b.id) ?? '';
    if (aKey !== bKey) {
      // '' (never worn) sorts first, so it ends up last after the flip below.
      return aKey < bKey ? 1 : -1;
    }
    const an = typeof a.name === 'string' ? a.name.toLowerCase() : '';
    const bn = typeof b.name === 'string' ? b.name.toLowerCase() : '';
    if (an !== bn) return an < bn ? -1 : 1;
    return 0;
  });
}

/**
 * Human-friendly "last worn" label relative to `todayKey`. Returns `null` when
 * the key is not a plain calendar date, so callers can hide the hint instead
 * of rendering raw data.
 */
export function formatLastWorn(key: string, todayKey: string): string | null {
  if (key === todayKey) return 'today';

  const yesterday = keyOfDaysAgo(todayKey, 1);
  if (yesterday !== null && key === yesterday) return 'yesterday';

  const m = DATE_KEY_RE.exec(key);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  const now = new Date();
  const options: Intl.DateTimeFormatOptions = { day: 'numeric', month: 'short' };
  if (d.getFullYear() !== now.getFullYear()) options.year = 'numeric';
  return d.toLocaleDateString(undefined, options);
}

function keyOfDaysAgo(todayKey: string, days: number): string | null {
  const m = DATE_KEY_RE.exec(todayKey);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  d.setDate(d.getDate() - days);
  return formatLocalDateKey(d);
}
