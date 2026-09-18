import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { defineComponent } from 'vue';
import { defaultLayout, loadLayout, saveLayout, newWidgetId } from './store';
import type { WidgetDef, WidgetInstance } from './types';

const stub = defineComponent({ name: 'Stub', render: () => null });

function def(type: string, defaultSize: 'sm' | 'md' | 'lg' = 'md', defaultConfig: Record<string, unknown> = {}): WidgetDef {
  return {
    type,
    title: type,
    category: 'usage',
    description: '',
    defaultSize,
    defaultConfig,
    configSchema: [],
    component: stub,
  };
}

function makeCatalog(): Map<string, WidgetDef> {
  return new Map(
    [
      def('stat-card', 'sm', { metric: 'total_items' }),
      def('wear-trend', 'lg', { granularity: 'week' }),
      def('category-donut'),
      def('color-bar'),
      def('top-worn'),
      def('insights-callout'),
    ].map(d => [d.type, d]),
  );
}

function fakeLocalStorage() {
  let store: Record<string, string> = {};
  return {
    getItem: (k: string) => (k in store ? store[k] : null),
    setItem: (k: string, v: string) => {
      store[k] = String(v);
    },
    removeItem: (k: string) => {
      delete store[k];
    },
  };
}

beforeEach(() => {
  vi.stubGlobal('localStorage', fakeLocalStorage());
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('defaultLayout', () => {
  it('returns the curated default set', () => {
    const layout = defaultLayout(makeCatalog());
    expect(layout).toHaveLength(8);
    expect(layout.map(w => w.type)).toEqual([
      'stat-card',
      'stat-card',
      'stat-card',
      'wear-trend',
      'category-donut',
      'color-bar',
      'top-worn',
      'insights-callout',
    ]);
    expect(layout[3].size).toBe('lg');
    expect(layout[0].config).toEqual({ metric: 'total_items' });
    expect(new Set(layout.map(w => w.id)).size).toBe(layout.length);
  });
});

function withoutIds(widgets: WidgetInstance[]): Omit<WidgetInstance, 'id'>[] {
  return widgets.map(({ id: _id, ...rest }) => rest);
}

describe('loadLayout', () => {
  it('returns defaults when nothing is stored', () => {
    expect(withoutIds(loadLayout(makeCatalog()))).toEqual(withoutIds(defaultLayout(makeCatalog())));
  });

  it('returns defaults for corrupt JSON or wrong version', () => {
    localStorage.setItem('insights.layout.v1', '{not json');
    expect(withoutIds(loadLayout(makeCatalog()))).toEqual(withoutIds(defaultLayout(makeCatalog())));

    localStorage.setItem('insights.layout.v1', JSON.stringify({ version: 99, widgets: [] }));
    expect(withoutIds(loadLayout(makeCatalog()))).toEqual(withoutIds(defaultLayout(makeCatalog())));
  });

  it('round-trips a saved layout', () => {
    const catalog = makeCatalog();
    const layout = defaultLayout(catalog);
    layout[0].config = { metric: 'total_spend' };
    layout[0].size = 'md';
    saveLayout(layout);
    expect(loadLayout(catalog)).toEqual(layout);
  });

  it('drops unknown widget types and invalid entries', () => {
    const catalog = makeCatalog();
    saveLayout([
      { id: 'x', type: 'does-not-exist', size: 'sm', config: {} },
      { id: 'y', size: 'sm', config: {} } as unknown as WidgetInstance,
      { id: 'z', type: 'wear-trend', size: 'xxl' as never, config: { granularity: 'month' } },
    ]);
    const loaded = loadLayout(catalog);
    expect(loaded).toHaveLength(1);
    expect(loaded[0].type).toBe('wear-trend');
    expect(loaded[0].size).toBe('lg'); // invalid size → widget default
    expect(loaded[0].config).toEqual({ granularity: 'month' });
  });

  it('re-merges missing config keys from defaults', () => {
    const catalog = makeCatalog();
    saveLayout([{ id: 'a', type: 'stat-card', size: 'sm', config: {} }]);
    const loaded = loadLayout(catalog);
    expect(loaded[0].config).toEqual({ metric: 'total_items' });
  });

  it('allows multiple instances of the same type', () => {
    const catalog = makeCatalog();
    saveLayout([
      { id: 'a', type: 'stat-card', size: 'sm', config: { metric: 'total_items' } },
      { id: 'b', type: 'stat-card', size: 'sm', config: { metric: 'total_spend' } },
    ]);
    expect(loadLayout(catalog)).toHaveLength(2);
  });
});

describe('newWidgetId', () => {
  it('generates unique ids', () => {
    const ids = new Set(Array.from({ length: 100 }, () => newWidgetId()));
    expect(ids.size).toBe(100);
  });
});
