/**
 * Date utilities for calendar and outfit management.
 * Ensures consistent, timezone-safe date extraction, comparison, and formatting.
 */

export function pad2(n: number): string {
  return n.toString().padStart(2, '0');
}

/**
 * Format a Date instance using local calendar date parts: YYYY-MM-DD.
 */
export function formatLocalDateKey(d: Date): string {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

/**
 * Extracts the canonical calendar date key (YYYY-MM-DD) from an outfit date string.
 *
 * Outfits represent items worn on a specific calendar date ("what I wore today").
 * Parsing date strings via `new Date(str).toISOString()` converts dates to UTC,
 * which shifts dates forward or backward by a day for users in timezones with non-zero
 * offsets (e.g. UTC-5 in New York or UTC+9 in Tokyo).
 *
 * This function preserves the calendar date invariant across all timezones.
 */
export function parseOutfitDateKey(dateStr: string | null | undefined): string {
  if (!dateStr) return '';
  const trimmed = dateStr.trim();
  const match = trimmed.match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (match) {
    return `${match[1]}-${match[2]}-${match[3]}`;
  }
  const d = new Date(trimmed);
  if (isNaN(d.getTime())) return '';
  return formatLocalDateKey(d);
}

/**
 * Format a YYYY-MM-DD date key into a user-friendly localized string.
 * Constructs local date using (year, month - 1, day) integers to prevent UTC midnight shifts.
 */
export function formatSelectedDateLabel(dateKey: string | null | undefined): string {
  if (!dateKey) return '';
  const match = dateKey.match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (!match) return '';
  const y = Number(match[1]);
  const m = Number(match[2]);
  const d = Number(match[3]);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
    year: 'numeric'
  });
}
