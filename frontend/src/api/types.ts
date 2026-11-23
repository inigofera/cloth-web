export type UUID = string;

export const NEW_OPTION = '__add__';

export const ORIGIN_OPTIONS = [
  'Bought New',
  '2nd Hand',
  'Gift',
  'Borrowed',
  'Made Myself'
] as const;
export type Origin = (typeof ORIGIN_OPTIONS)[number];

export const LAUNDRY_IMPACT_OPTIONS = ['Nothing', 'Low', 'Medium', 'High'] as const;
export type LaundryImpact = (typeof LAUNDRY_IMPACT_OPTIONS)[number];

export interface ClothingItemFormState {
  name: string;
  category_id: number | string | null;
  subcategory_id: number | string | null;
  color_id: string | null;
  brand_id: string | null;
  purchase_price: number | null;
  owned_since: string;
  origin: Origin | '';
  laundry_impact: LaundryImpact | '';
  wear_count: number | null;
  repairable: boolean;
  is_active: boolean;
  notes: string;
}

export interface ColumnInfo {
  table_name: string;
  column_name: string;
  data_type: string;
}

// Generic JSON value type alias for convenience
export type JsonValue = unknown;

export interface ClothingItem {
  id: UUID;
  name: string;
  category_id: number | null;
  subcategory_id: number | null;
  color_id: string | null;
  brand_id: UUID | null;
  purchase_price: number | null;
  owned_since: string | null;
  origin: Origin | null;
  laundry_impact: LaundryImpact | null;
  repairable: boolean | null;
  notes: string | null;
  image_path: string | null;
  is_active: boolean;
  wear_count: number | null;
  created_at: string;
  updated_at: string;
  [key: string]: unknown;
}

export interface CreateClothingItemPayload {
  name: string;
  category_id?: number | null;
  subcategory_id?: number | null;
  color_id?: string | null;
  brand_id?: UUID | null;
  purchase_price?: number | null;
  owned_since?: string | null;
  origin?: Origin | null;
  laundry_impact?: LaundryImpact | null;
  repairable?: boolean;
  notes?: string | null;
  image_path?: string | null;
  is_active?: boolean;
  wear_count?: number | null;
}

export interface Color {
  id: string;
  hex_value: string | null;
}

export interface ClothingCategory {
  id: number;
  name: string;
}

export interface ClothingSubcategory {
  id: number;
  name: string;
  category_id: number;
}

export interface Brand {
  id: UUID;
  name: string;
}

// An outfit row carries the ids of its clothing items, not a copy of each row
// (the server used to inline a full object per item: see routes.rs OUTFITS_ROW_EXPR,
// which now emits `item_ids` ordered by item name). Consumers resolve ids
// against the clothing-items snapshot they already hold.
export interface Outfit {
  id: UUID;
  date: string;
  notes: string | null;
  image_path: string | null;
  is_active: boolean;
  created_at: string;
  item_ids: UUID[];
}

export interface CreateOutfitPayload {
  date: string;
  notes: string | null;
  item_ids: UUID[];
  image_path?: string | null;
}

/**
 * Full-replacement payload for `PATCH /outfits/:id`: same shape as creation,
 * the server diffs the item links and adjusts wear counts in one
 * transaction.
 */
export type UpdateOutfitPayload = CreateOutfitPayload;

/**
 * Versioned export document from `GET /export`: the caller's items and
 * outfits (carrying `item_ids`) plus every reference row visible to them.
 */
export interface ExportDocument {
  version: number;
  exported_at: string;
  user_id: UUID;
  items: unknown[];
  outfits: unknown[];
  colors: unknown[];
  categories: unknown[];
  subcategories: unknown[];
  brands: unknown[];
}

/** Result of `POST /import`: how much of the document was actually written. */
export interface ImportResult {
  items_imported: number;
  outfits_imported: number;
  links_added: number;
  colors_created: number;
  categories_created: number;
  subcategories_created: number;
  brands_created: number;
}

/**
 * Per-name reference counts from `GET /reference-usage`, for the settings
 * reference-data manager. Names that the caller owns but nothing references
 * are present with a zero count.
 */
export interface ReferenceUsage {
  colors: Record<string, number>;
  categories: Record<string, number>;
  subcategories: Record<string, number>;
  brands: Record<string, number>;
}

export type TryOnProvider = 'gemini' | 'qwen_local';

export interface UserSettings {
  tryon_provider: TryOnProvider;
  has_api_key: boolean;
  api_key_hint: string | null;
  local_qwen_url: string;
  reference_image_path: string | null;
  discord_user_id: string | null;
}

export interface UpdateUserSettingsPayload {
  tryon_provider?: TryOnProvider;
  api_key?: string;
  local_qwen_url?: string;
  reference_image_path?: string;
}

export interface GeneratePreviewResponse {
  image_path: string;
}


