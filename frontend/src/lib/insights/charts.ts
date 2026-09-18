import {
  ArcElement,
  BarController,
  BarElement,
  CategoryScale,
  Chart,
  DoughnutController,
  Filler,
  Legend,
  LineController,
  LineElement,
  LinearScale,
  PointElement,
  Tooltip,
  type ChartOptions,
} from 'chart.js';

Chart.register(
  ArcElement,
  BarController,
  BarElement,
  CategoryScale,
  DoughnutController,
  Filler,
  Legend,
  LineController,
  LineElement,
  LinearScale,
  PointElement,
  Tooltip,
);

export interface ChartTheme {
  primary: string;
  onPrimary: string;
  primaryContainer: string;
  onPrimaryContainer: string;
  secondary: string;
  tertiary: string;
  error: string;
  onSurface: string;
  onSurfaceVariant: string;
  surface: string;
  surfaceContainer: string;
  surfaceContainerHighest: string;
  outlineVariant: string;
  inverseSurface: string;
  inverseOnSurface: string;
  font: string;
}

function cssVar(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

/** Read the active MD3 theme (light or dark) from CSS custom properties. */
export function chartTheme(): ChartTheme {
  return {
    primary: cssVar('--md-primary', '#6750a4'),
    onPrimary: cssVar('--md-on-primary', '#ffffff'),
    primaryContainer: cssVar('--md-primary-container', '#eaddff'),
    onPrimaryContainer: cssVar('--md-on-primary-container', '#21005d'),
    secondary: cssVar('--md-secondary', '#625b71'),
    tertiary: cssVar('--md-tertiary', '#7c5872'),
    error: cssVar('--md-error', '#b3261e'),
    onSurface: cssVar('--md-on-surface', '#1d1b20'),
    onSurfaceVariant: cssVar('--md-on-surface-variant', '#49454f'),
    surface: cssVar('--md-surface', '#fef7ff'),
    surfaceContainer: cssVar('--md-surface-container', '#f3edf7'),
    surfaceContainerHighest: cssVar('--md-surface-container-highest', '#e6e0e9'),
    outlineVariant: cssVar('--md-outline-variant', '#cac4d0'),
    inverseSurface: cssVar('--md-inverse-surface', '#322f35'),
    inverseOnSurface: cssVar('--md-inverse-on-surface', '#f5eff7'),
    font: cssVar('--md-font-family', 'Roboto, sans-serif'),
  };
}

/** Harmonized categorical palette (MD3 purple family, light + dark friendly). */
export const CATEGORICAL_PALETTE: string[] = [
  '#6750a4',
  '#7c5872',
  '#625b71',
  '#9a82db',
  '#b69df8',
  '#4f378b',
  '#8f7cc0',
  '#c5a3ff',
  '#d8c4f0',
  '#5d4a8f',
];

export function paletteColor(index: number): string {
  return CATEGORICAL_PALETTE[index % CATEGORICAL_PALETTE.length];
}

/** Shared Chart.js options derived from the MD3 theme. */
export function baseOptions(): ChartOptions {
  const t = chartTheme();
  return {
    responsive: true,
    maintainAspectRatio: false,
    animation: { duration: 300 },
    plugins: {
      legend: {
        labels: {
          color: t.onSurfaceVariant,
          font: { family: t.font, size: 11 },
          boxWidth: 12,
          boxHeight: 12,
        },
      },
      tooltip: {
        backgroundColor: t.inverseSurface,
        titleColor: t.inverseOnSurface,
        bodyColor: t.inverseOnSurface,
        titleFont: { family: t.font, size: 12 },
        bodyFont: { family: t.font, size: 12 },
        padding: 10,
        cornerRadius: 8,
      },
    },
  };
}

/** Tick/grid styling for cartesian charts. */
export function axisOptions(): {
  ticks: { color: string; font: { family: string; size: number } };
  grid: { color: string };
  border: { color: string };
} {
  const t = chartTheme();
  return {
    ticks: { color: t.onSurfaceVariant, font: { family: t.font, size: 11 } },
    grid: { color: `${t.outlineVariant}55` },
    border: { color: t.outlineVariant },
  };
}
