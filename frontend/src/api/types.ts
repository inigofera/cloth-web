export type UUID = string;

export interface User {
  id: UUID;
  email: string | null;
}

export interface ColumnInfo {
  table_name: string;
  column_name: string;
  data_type: string;
}

// Generic JSON value type alias for convenience
export type JsonValue = unknown;


