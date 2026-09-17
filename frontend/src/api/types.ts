export type UUID = string;

export const NEW_OPTION = '__add__';

export interface ClothingItemFormState {
  name: string;
  category_id: number | string | null;
  subcategory_id: number | string | null;
  color_id: string | null;
  brand_id: string | null;
  purchase_price: number | null;
  owned_since: string;
  origin: string;
  laundry_impact: string;
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
  origin: string | null;
  laundry_impact: string | null;
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
  origin?: string | null;
  laundry_impact?: string | null;
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

export interface OutfitItem {
  id: UUID;
  name: string;
  image_path: string | null;
  color_id: string | null;
  [key: string]: unknown;
}

export interface Outfit {
  id: UUID;
  date: string;
  notes: string | null;
  image_path: string | null;
  is_active: boolean;
  created_at: string;
  items: OutfitItem[];
}

export interface CreateOutfitPayload {
  date: string;
  notes: string | null;
  item_ids: UUID[];
}


