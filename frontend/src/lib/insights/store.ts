import type { WidgetDef, WidgetInstance, WidgetSize } from './types';

const STORAGE_KEY = 'insights.layout.v1';
const LAYOUT_VERSION = 1;
const SIZES: WidgetSize[] = ['sm', 'md', 'lg'];

interface LayoutState {
  version: number;
  widgets: WidgetInstance[];
}

export function newWidgetId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `w-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

export function createInstance(
  type: string,
  catalog: Map<string, WidgetDef>,
  overrides?: { id?: string; size?: WidgetSize; config?: Record<string, unknown> },
): WidgetInstance | null {
  const def = catalog.get(type);
  if (!def) return null;
  const { config, ...rest } = overrides ?? {};
  return {
    id: newWidgetId(),
    type,
    size: def.defaultSize,
    config: { ...def.defaultConfig, ...config },
    ...rest,
  };
}

/** First-run layout: a curated default set (types missing from the catalog are skipped). */
export function defaultLayout(catalog: Map<string, WidgetDef>): WidgetInstance[] {
  return [
    createInstance('stat-card', catalog, { config: { metric: 'total_items' } }),
    createInstance('stat-card', catalog, { config: { metric: 'total_outfits' } }),
    createInstance('stat-card', catalog, { config: { metric: 'cost_per_wear' } }),
    createInstance('wear-trend', catalog),
    createInstance('category-donut', catalog),
    createInstance('color-bar', catalog),
    createInstance('top-worn', catalog),
    createInstance('insights-callout', catalog),
  ].filter((w): w is WidgetInstance => w !== null);
}

/**
 * Load and validate the persisted layout. Unknown widget types are dropped,
 * invalid sizes fall back to the widget default, and missing config keys are
 * re-merged from the catalog defaults so old layouts never crash.
 */
export function loadLayout(catalog: Map<string, WidgetDef>): WidgetInstance[] {
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(STORAGE_KEY);
  } catch {
    return defaultLayout(catalog);
  }
  if (!raw) return defaultLayout(catalog);

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return defaultLayout(catalog);
  }
  if (typeof parsed !== 'object' || parsed === null) return defaultLayout(catalog);
  const state = parsed as Partial<LayoutState>;
  if (state.version !== LAYOUT_VERSION || !Array.isArray(state.widgets)) {
    return defaultLayout(catalog);
  }

  const widgets: WidgetInstance[] = [];
  for (const w of state.widgets) {
    if (typeof w !== 'object' || w === null) continue;
    const inst = w as Partial<WidgetInstance>;
    if (typeof inst.type !== 'string') continue;
    const def = catalog.get(inst.type);
    if (!def) continue;
    widgets.push({
      id: typeof inst.id === 'string' && inst.id ? inst.id : newWidgetId(),
      type: inst.type,
      size: SIZES.includes(inst.size as WidgetSize) ? (inst.size as WidgetSize) : def.defaultSize,
      config: { ...def.defaultConfig, ...(typeof inst.config === 'object' && inst.config !== null ? inst.config : {}) },
    });
  }
  return widgets;
}

export function saveLayout(widgets: WidgetInstance[]): void {
  try {
    const state: LayoutState = { version: LAYOUT_VERSION, widgets };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Storage full or unavailable (private mode) — layout just won't persist.
  }
}
