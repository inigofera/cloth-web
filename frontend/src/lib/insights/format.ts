const numberFmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
const numberFmt1 = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });

export function formatNumber(n: number): string {
  return numberFmt.format(n);
}

export function formatNumber1(n: number): string {
  return numberFmt1.format(n);
}

/** Plain 2-decimal price, matching the rest of the app (no currency symbol). */
export function formatPrice(n: number): string {
  return n.toFixed(2);
}

export function formatPercent(fraction: number): string {
  return `${Math.round(fraction * 100)}%`;
}

export function formatDateKey(key: string): string {
  const [y, m, d] = key.split('-').map(Number);
  if (!y || !m || !d) return key;
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

export function formatMonthLabel(ym: string): string {
  const [y, m] = ym.split('-').map(Number);
  if (!y || !m) return ym;
  return new Date(y, m - 1, 1).toLocaleDateString(undefined, { month: 'short', year: '2-digit' });
}
