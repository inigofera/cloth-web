import type { Component } from 'vue';

// ---------------------------------------------------------------------------
// Widget system
// ---------------------------------------------------------------------------

export type WidgetSize = 'sm' | 'md' | 'lg';

export type WidgetCategory =
  | 'usage'
  | 'wardrobe'
  | 'value'
  | 'sustainability'
  | 'relationships'
  | 'insights';

export const WIDGET_CATEGORIES: { id: WidgetCategory; label: string }[] = [
  { id: 'usage', label: 'Usage habits' },
  { id: 'wardrobe', label: 'Wardrobe composition' },
  { id: 'value', label: 'Value & cost' },
  { id: 'sustainability', label: 'Sustainability' },
  { id: 'relationships', label: 'Relationships' },
  { id: 'insights', label: 'Insights' },
];

export interface WidgetInstance {
  id: string;
  type: string;
  size: WidgetSize;
  config: Record<string, unknown>;
}

export interface ConfigOption {
  value: string | number;
  label: string;
}

export type ConfigField =
  | { key: string; label: string; kind: 'select'; options: ConfigOption[] }
  | { key: string; label: string; kind: 'number'; min: number; max: number; step?: number }
  | { key: string; label: string; kind: 'toggle' };

export interface WidgetDef {
  type: string;
  title: string;
  category: WidgetCategory;
  description: string;
  defaultSize: WidgetSize;
  defaultConfig: Record<string, unknown>;
  configSchema: ConfigField[];
  component: Component;
}

// ---------------------------------------------------------------------------
// Dataset
// ---------------------------------------------------------------------------

export interface EnrichedItem {
  id: string;
  name: string;
  image_path: string | null;
  category_id: number | null;
  subcategory_id: number | null;
  color_id: string | null;
  brand_id: string | null;
  purchase_price: number | null;
  owned_since: string | null;
  laundry_impact: string | null;
  is_active: boolean;
  wear_count: number | null;
  created_at: string;
  // enriched joins
  category_name: string | null;
  subcategory_name: string | null;
  color_hex: string | null;
  brand_name: string | null;
}

export interface EnrichedOutfit {
  id: string;
  date: string;
  /** YYYY-MM-DD (UTC) — same convention as OutfitsView. */
  dateKey: string;
  notes: string | null;
  items: EnrichedItem[];
}

export interface InsightDataset {
  items: EnrichedItem[];
  outfits: EnrichedOutfit[];
  colors: Map<string, { id: string; hex_value: string | null }>;
  categories: Map<number, string>;
  subcategories: Map<number, { name: string; category_id: number }>;
  brands: Map<string, string>;
}

export type RangePreset = '30d' | '90d' | '1y' | 'all' | 'custom';

export interface GlobalFilters {
  rangePreset: RangePreset;
  from: string | null;
  to: string | null;
  categories: number[];
  colors: string[];
  brands: string[];
  activeOnly: boolean;
}

export const DEFAULT_FILTERS: GlobalFilters = {
  rangePreset: 'all',
  from: null,
  to: null,
  categories: [],
  colors: [],
  brands: [],
  activeOnly: false,
};

/** Items and outfits after global filters have been applied. */
export interface FilteredData {
  items: EnrichedItem[];
  outfits: EnrichedOutfit[];
}
