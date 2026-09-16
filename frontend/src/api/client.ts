import type { ColumnInfo, CreateOutfitPayload, JsonValue, Outfit } from './types';
import { supabase } from '../lib/supabase';

const API_BASE = '/api';
const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL as string;
const STORAGE_BUCKET = import.meta.env.VITE_SUPABASE_BUCKET as string;

export function imageUrl(path: string | null | undefined): string | null {
  if (!path) return null;
  if (path.startsWith('http')) return path;
  return `${SUPABASE_URL}/storage/v1/object/public/${STORAGE_BUCKET}/${path}`;
}

async function fetchJson<T>(input: RequestInfo, init?: RequestInit): Promise<T> {
  const { data } = await supabase.auth.getSession();
  const headers = new Headers(init?.headers);
  if (data.session) {
    headers.set('Authorization', `Bearer ${data.session.access_token}`);
  }

  const res = await fetch(input, { ...init, headers });
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`${res.status} ${res.statusText}${text ? `: ${text}` : ''}`);
  }
  return res.json() as Promise<T>;
}

export const api = {
  listTables: () => fetchJson<ColumnInfo[]>(`${API_BASE}/tables`),
  getTableRows: (table: string) => fetchJson<JsonValue[]>(`${API_BASE}/db/${encodeURIComponent(table)}`),
  insertTableRow: (table: string, data: JsonValue) => fetchJson<JsonValue | null>(
    `${API_BASE}/db/${encodeURIComponent(table)}`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }
  ),
  updateTableRow: (table: string, id: string, data: JsonValue) => fetchJson<JsonValue | null>(
    `${API_BASE}/db/${encodeURIComponent(table)}/${encodeURIComponent(id)}`,
    { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }
  ),
  deleteTableRow: (table: string, id: string) => fetchJson<JsonValue | null>(
    `${API_BASE}/db/${encodeURIComponent(table)}/${encodeURIComponent(id)}`,
    { method: 'DELETE' }
  ),
  uploadFile: async (file: File) => {
    const form = new FormData();
    form.append('file', file);
    return fetchJson<string>(`${API_BASE}/upload`, { method: 'POST', body: form });
  },
  listOutfits: () => fetchJson<Outfit[]>(`${API_BASE}/outfits`),
  createOutfit: (data: CreateOutfitPayload) => fetchJson<Outfit>(
    `${API_BASE}/outfits`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }
  )
};
