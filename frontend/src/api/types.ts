export type UUID = string;

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
  image_path: string | null;
  color_id: string | null;
  is_active: boolean;
  [key: string]: unknown;
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


