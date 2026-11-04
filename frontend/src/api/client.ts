import type {
  Brand,
  ClothingCategory,
  ClothingItem,
  ClothingSubcategory,
  Color,
  ColumnInfo,
  CreateClothingItemPayload,
  CreateOutfitPayload,
  GeneratePreviewResponse,
  JsonValue,
  Outfit,
  UpdateUserSettingsPayload,
  UserSettings
} from './types';
import { supabase } from '../lib/supabase';
import { isValidObjectPath } from '../lib/sanitize';

const API_BASE = '/api';
const STORAGE_BUCKET = (import.meta.env.VITE_SUPABASE_BUCKET as string) || 'clothing-wardrobe';

let accessToken: string | null = null;

function syncToken(session: { access_token: string } | null): void {
  accessToken = session?.access_token ?? null;
}

supabase.auth.getSession().then(({ data }) => syncToken(data.session));
supabase.auth.onAuthStateChange((_event, session) => syncToken(session));

export interface ImageUrlOptions {
  width?: number;
}

function objectPathOf(path: string): string {
  const usersIdx = path.indexOf('users/');
  if (usersIdx !== -1) return path.slice(usersIdx);
  const publicMarker = `/storage/v1/object/public/${STORAGE_BUCKET}/`;
  const authMarker = `/storage/v1/object/authenticated/${STORAGE_BUCKET}/`;
  const idx = path.indexOf(publicMarker);
  if (idx !== -1) return path.slice(idx + publicMarker.length);
  const aidx = path.indexOf(authMarker);
  if (aidx !== -1) return path.slice(aidx + authMarker.length);
  return path.replace(/^\/+/, '');
}

export function imageUrl(path: string | null | undefined, opts?: ImageUrlOptions): string | null {
  if (!path) return null;
  const objectPath = objectPathOf(path);
  // image_path is DB-sourced and attacker-writable via the generic table API;
  // only render well-formed, single-user object paths.
  if (!isValidObjectPath(objectPath)) return null;
  const params = new URLSearchParams();
  if (opts?.width) params.set('width', String(opts.width));
  if (accessToken) params.set('token', accessToken);
  const qs = params.toString();
  return `${API_BASE}/files/${objectPath}${qs ? `?${qs}` : ''}`;
}

async function fetchWithHeaders(
  input: RequestInfo,
  init?: RequestInit
): Promise<{ data: unknown; headers: Headers }> {
  let { data } = await supabase.auth.getSession();
  const headers = new Headers(init?.headers);
  if (data.session) {
    headers.set('Authorization', `Bearer ${data.session.access_token}`);
  }

  let res = await fetch(input, { ...init, headers });
  if (res.status === 401) {
    // Attempt session refresh if token expired
    const { data: refreshData } = await supabase.auth.refreshSession();
    if (refreshData.session) {
      syncToken(refreshData.session);
      headers.set('Authorization', `Bearer ${refreshData.session.access_token}`);
      res = await fetch(input, { ...init, headers });
    }
  }

  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`${res.status} ${res.statusText}${text ? `: ${text}` : ''}`);
  }
  return { data: (await res.json()) as unknown, headers: res.headers };
}

async function fetchJson<T>(input: RequestInfo, init?: RequestInit): Promise<T> {
  const { data } = await fetchWithHeaders(input, init);
  return data as T;
}

/**
 * Read the `X-Total-Count` header set by list endpoints (UX_ANALYSIS.md P1-B1):
 * it tells the UI how much data actually exists so a capped snapshot can be
 * surfaced honestly instead of silently hidden.
 */
function readTotalCount(headers: Headers | null | undefined): number | null {
  if (!headers) return null;
  const raw = headers.get('X-Total-Count');
  if (raw == null) return null; // older server / endpoint without the signal
  const n = Number(raw);
  return Number.isSafeInteger(n) && n >= 0 ? n : null;
}

async function listTableWithTotal<T>(table: string): Promise<{ items: T[]; total: number | null }> {
  const { data, headers } = await fetchWithHeaders(
    `${API_BASE}/db/${encodeURIComponent(table)}`
  );
  return { items: data as T[], total: readTotalCount(headers) };
}

async function listOutfitsWithTotal(): Promise<{ items: Outfit[]; total: number | null }> {
  const { data, headers } = await fetchWithHeaders(`${API_BASE}/outfits`);
  return { items: data as Outfit[], total: readTotalCount(headers) };
}

export const api = {
  listClothingItemsWithTotal: () => listTableWithTotal<ClothingItem>('clothing-items'),
  listOutfitsWithTotal,

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
  ),
  deleteOutfit: (id: string) => fetchJson<JsonValue>(
    `${API_BASE}/outfits/${encodeURIComponent(id)}`,
    { method: 'DELETE' }
  ),
  listClothingItems: () => fetchJson<ClothingItem[]>(`${API_BASE}/db/clothing-items`),
  createClothingItem: (data: CreateClothingItemPayload) => fetchJson<ClothingItem>(
    `${API_BASE}/db/clothing-items`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }
  ),
  listColors: () => fetchJson<Color[]>(`${API_BASE}/db/colors`),
  listCategories: () => fetchJson<ClothingCategory[]>(`${API_BASE}/db/clothing-categories`),
  listSubcategories: () => fetchJson<ClothingSubcategory[]>(`${API_BASE}/db/clothing-subcategories`),
  listBrands: () => fetchJson<Brand[]>(`${API_BASE}/db/brands`),
  createColor: (data: { id: string; hex_value: string | null }) => fetchJson<Color>(
    `${API_BASE}/colors`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }
  ),
  createCategory: (data: { name: string }) => fetchJson<ClothingCategory>(
    `${API_BASE}/clothing-categories`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }
  ),
  createSubcategory: (data: { name: string; category_id: number }) => fetchJson<ClothingSubcategory>(
    `${API_BASE}/clothing-subcategories`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }
  ),
  getSettings: () => fetchJson<UserSettings>(`${API_BASE}/settings`),
  updateSettings: (payload: UpdateUserSettingsPayload) => fetchJson<UserSettings>(
    `${API_BASE}/settings`,
    { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) }
  ),
  generateOutfitPreview: (itemIds: string[], signal?: AbortSignal) => fetchJson<GeneratePreviewResponse>(
    `${API_BASE}/outfits/generate-preview`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ item_ids: itemIds }),
      // Optional so callers may cancel an in-flight generation (the try-on
      // flow offers a cancel action while it runs for 10-20s).
      signal,
    }
  ),
  syncDiscordOutfits: () => fetchJson<{
    imported_count: number;
    messages_processed: number;
    new_items_created: number;
    summary: string[];
  }>(`${API_BASE}/outfits/discord-sync`, { method: 'POST' }),
  generateDiscordLinkCode: () => fetchJson<{ code: string; expires_in_seconds: number }>(
    `${API_BASE}/settings/discord-code`,
    { method: 'POST' }
  ),
  unlinkDiscordAccount: () => fetchJson<UserSettings>(
    `${API_BASE}/settings/discord-unlink`,
    { method: 'POST' }
  ),
  testQwenConnection: (url?: string) => {
    const params = new URLSearchParams();
    if (url) params.set('url', url);
    const qs = params.toString();
    return fetchJson<{ status: string; model: string; ready: boolean; [key: string]: unknown }>(
      `${API_BASE}/settings/test-qwen${qs ? `?${qs}` : ''}`
    );
  }
};

