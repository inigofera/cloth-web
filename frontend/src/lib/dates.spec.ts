import { describe, it, expect } from 'vitest';
import {
  pad2,
  formatLocalDateKey,
  parseOutfitDateKey,
  formatSelectedDateLabel
} from './dates';

describe('dates utility', () => {
  describe('pad2', () => {
    it('pads single-digit numbers', () => {
      expect(pad2(1)).toBe('01');
      expect(pad2(9)).toBe('09');
    });

    it('leaves two-digit numbers unchanged', () => {
      expect(pad2(10)).toBe('10');
      expect(pad2(31)).toBe('31');
    });
  });

  describe('formatLocalDateKey', () => {
    it('formats Date instance with local year, month, date', () => {
      const d = new Date(2026, 9, 14); // Note: month 9 = October
      expect(formatLocalDateKey(d)).toBe('2026-10-14');
    });

    it('correctly zero-pads month and day', () => {
      const d = new Date(2026, 0, 5); // January 5
      expect(formatLocalDateKey(d)).toBe('2026-01-05');
    });
  });

  describe('parseOutfitDateKey', () => {
    it('handles date-only strings without timezone drift', () => {
      expect(parseOutfitDateKey('2026-10-14')).toBe('2026-10-14');
      expect(parseOutfitDateKey('2026-01-01')).toBe('2026-01-01');
    });

    it('handles ISO timestamps with UTC Z suffix without shifting date', () => {
      expect(parseOutfitDateKey('2026-10-14T00:00:00Z')).toBe('2026-10-14');
      expect(parseOutfitDateKey('2026-10-14T12:30:00Z')).toBe('2026-10-14');
    });

    it('handles ISO timestamps with positive UTC offsets (e.g. Tokyo +09:00)', () => {
      expect(parseOutfitDateKey('2026-10-14T08:00:00+09:00')).toBe('2026-10-14');
      expect(parseOutfitDateKey('2026-10-14T00:00:00+02:00')).toBe('2026-10-14');
    });

    it('handles ISO timestamps with negative UTC offsets (e.g. New York -04:00)', () => {
      expect(parseOutfitDateKey('2026-10-14T21:00:00-04:00')).toBe('2026-10-14');
      expect(parseOutfitDateKey('2026-10-14T23:59:00-07:00')).toBe('2026-10-14');
    });

    it('handles SQL formatted timestamps', () => {
      expect(parseOutfitDateKey('2026-10-14 00:00:00')).toBe('2026-10-14');
      expect(parseOutfitDateKey('2026-10-14 20:15:30+00')).toBe('2026-10-14');
    });

    it('returns empty string for empty or nullish inputs', () => {
      expect(parseOutfitDateKey(null)).toBe('');
      expect(parseOutfitDateKey(undefined)).toBe('');
      expect(parseOutfitDateKey('')).toBe('');
      expect(parseOutfitDateKey('   ')).toBe('');
      expect(parseOutfitDateKey('not-a-date')).toBe('');
    });
  });

  describe('formatSelectedDateLabel', () => {
    it('formats YYYY-MM-DD into a localized weekday and date string', () => {
      const label = formatSelectedDateLabel('2026-10-14');
      expect(label).toContain('2026');
      expect(label).toContain('October');
      expect(label).toContain('14');
    });

    it('returns empty string on invalid inputs', () => {
      expect(formatSelectedDateLabel(null)).toBe('');
      expect(formatSelectedDateLabel('')).toBe('');
      expect(formatSelectedDateLabel('invalid')).toBe('');
    });
  });
});
