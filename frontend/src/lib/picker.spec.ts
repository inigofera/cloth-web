import { describe, it, expect } from 'vitest';
import { buildLastWornMap, sortByRecency, formatLastWorn } from './picker';

interface FakeItem {
  id: string;
  name?: string;
}

describe('buildLastWornMap', () => {
  it('records the most recent date key per item', () => {
    const map = buildLastWornMap([
      { date: '2026-11-01', item_ids: ['a', 'b'] },
      { date: '2026-11-03', item_ids: ['a'] },
    ]);
    expect(map.get('a')).toBe('2026-11-03');
    expect(map.get('b')).toBe('2026-11-01');
  });

  it('ignores outfits with unparseable dates', () => {
    const map = buildLastWornMap([
      { date: 'not-a-date', item_ids: ['a'] },
      { date: '', item_ids: ['b'] },
      { date: '2026-11-04T08:30:00+09:00', item_ids: ['c'] },
    ]);
    expect(map.has('a')).toBe(false);
    expect(map.has('b')).toBe(false);
    // ISO timestamps are normalized to their calendar date.
    expect(map.get('c')).toBe('2026-11-04');
  });

  it('returns an empty map for no outfits', () => {
    expect(buildLastWornMap([]).size).toBe(0);
  });

  it('does not mutate the input array', () => {
    const outfits = [{ date: '2026-11-04', item_ids: ['a'] }];
    buildLastWornMap(outfits);
    expect(outfits).toHaveLength(1);
  });
});

describe('sortByRecency', () => {
  it('puts recently worn items first, never-worn last (name order tiebreak)', () => {
    const items: FakeItem[] = [
      { id: 'never-2', name: 'Zeta Jacket' },
      { id: 'recent', name: 'Blue Jeans' },
      { id: 'old', name: 'Red Dress' },
      { id: 'never-1', name: 'Alpha Shirt' },
    ];
    const lastWorn = new Map<string, string>([
      ['recent', '2026-11-03'],
      ['old', '2025-01-15'],
    ]);

    expect(sortByRecency(items, lastWorn)).toEqual([
      { id: 'recent', name: 'Blue Jeans' },
      { id: 'old', name: 'Red Dress' },
      { id: 'never-1', name: 'Alpha Shirt' },
      { id: 'never-2', name: 'Zeta Jacket' },
    ]);
  });

  it('does not mutate the input array', () => {
    const items: FakeItem[] = [{ id: 'a' }];
    const result = sortByRecency(items, new Map());
    expect(result).not.toBe(items);
    expect(items).toHaveLength(1);
  });

  it('returns an empty array for no items', () => {
    expect(sortByRecency([], new Map())).toEqual([]);
  });
});

describe('formatLastWorn', () => {
  it('labels today as "today"', () => {
    expect(formatLastWorn('2026-11-04', '2026-11-04')).toBe('today');
  });

  it('labels yesterday as "yesterday" (deterministic across timezones)', () => {
    expect(formatLastWorn('2026-11-03', '2026-11-04')).toBe('yesterday');
  });

  it('handles the month boundary when computing yesterday', () => {
    // Nov 1 relative to Dec 1 is exactly one day ago.
    expect(formatLastWorn('2025-11-30', '2025-12-01')).toBe('yesterday');
  });

  it('returns a short date string for older keys', () => {
    const label = formatLastWorn('2026-01-15', '2026-11-04');
    expect(label).not.toBeNull();
    // toLocaleDateString on an invalid Date returns the literal string
    // "Invalid Date", so guard against that regression explicitly.
    expect(label).not.toBe('Invalid Date');
  });

  it('returns null for keys that are not plain calendar dates', () => {
    expect(formatLastWorn('', '2026-11-04')).toBeNull();
    expect(formatLastWorn('garbage', '2026-11-04')).toBeNull();
  });
});
