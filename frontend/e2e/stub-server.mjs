// E2E stub: a fake Supabase GoTrue auth endpoint and a fake cloth-web API.
//
// Both are deliberately thin — only the surface the smoke flows touch,
// plus the M5 endpoints (outfit edit, export/import, reference usage and
// rename/delete) that the settings page loads on mount.
// The auth stub lives on :5433 (the value VITE_SUPABASE_URL is pointed at),
// the API stub on :3000 (where vite's dev proxy forwards /api).
//
// State is in-memory and process-wide; every Playwright worker context shares
// it, so specs avoid asserting on absolute counts that other flows mutate.

import http from 'node:http';
import crypto from 'node:crypto';

const API_PORT = Number(process.env.E2E_API_PORT || 3000);
const AUTH_PORT = Number(process.env.E2E_AUTH_PORT || 5433);

const USER = {
  id: 'c0a80001-0000-4000-8000-000000000001',
  email: 'e2e@example.com',
  password: 'e2e-password',
};
const ACCESS_TOKEN = 'e2e-access-token';
const BRAND_ID = 'b1a2c3d4-0000-4000-8000-000000000001';

// 1x1 PNG: the stub's "photo" for seeded items.
const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAAABJRUkErkJggg==',
  'base64',
);

function pad2(n) {
  return String(n).padStart(2, '0');
}
function nowIso() {
  return new Date().toISOString();
}
function daysAgo(n) {
  return new Date(Date.now() - n * 86400_000);
}
function daysAgoKey(n) {
  const d = daysAgo(n);
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

function makeItem(id, name, category_id, subcategory_id, color_id, purchase_price, owned_since, wear_count) {
  // The filename must start with a UUID: the client's isValidObjectPath
  // rejects anything else, and an unrendered image is a broken card.
  return {
    id,
    user_id: USER.id,
    name,
    category_id,
    subcategory_id,
    color_id,
    brand_id: null,
    purchase_price,
    owned_since,
    origin: 'Bought New',
    laundry_impact: 'Low',
    repairable: true,
    notes: null,
    image_path: `users/${USER.id}/${crypto.randomUUID()}-${name.toLowerCase().replace(/\s+/g, '-')}.png`,
    is_active: true,
    wear_count,
    created_at: daysAgo(40).toISOString(),
    updated_at: daysAgo(40).toISOString(),
  };
}

function makeOutfit(id, date, itemIds, notes) {
  return {
    id,
    date,
    notes,
    image_path: null,
    is_active: true,
    created_at: daysAgo(2).toISOString(),
    item_ids: itemIds,
  };
}

const state = {
  colors: [
    { id: 'black', hex_value: '#000000' },
    { id: 'white', hex_value: '#ffffff' },
    { id: 'blue', hex_value: '#1e3a8a' },
  ],
  categories: [
    { id: 1, name: 'Tops' },
    { id: 2, name: 'Bottoms' },
    { id: 3, name: 'Outerwear' },
  ],
  subcategories: [
    { id: 1, name: 'T-Shirts', category_id: 1 },
    { id: 2, name: 'Jeans', category_id: 2 },
  ],
  brands: [{ id: BRAND_ID, name: 'Acme' }],
  items: [],
  outfits: [],
  files: new Map(), // object path -> { data: Buffer, contentType: string }
  settings: {
    tryon_provider: 'gemini',
    has_api_key: false,
    api_key_hint: null,
    local_qwen_url: 'http://127.0.0.1:8001',
    reference_image_path: null,
    discord_user_id: null,
  },
};

// Seed: four items with photos and three outfits spread over recent days, so
// the Insights tab has real data to chart on first open.
const SEED = [
  makeItem('a1000000-0000-4000-8000-000000000001', 'White Tee', 1, 1, 'white', 19.99, '2025-06-01', 12),
  makeItem('a2000000-0000-4000-8000-000000000002', 'Blue Jeans', 2, 2, 'blue', 59.0, '2025-03-15', 8),
  makeItem('a3000000-0000-4000-8000-000000000003', 'Black Jacket', 3, null, 'black', 120.0, '2024-11-20', 4),
  makeItem('a4000000-0000-4000-8000-000000000004', 'Green Hoodie', 1, 1, null, 35.5, '2025-09-01', 2),
];
state.items.push(...SEED);
for (const item of SEED) {
  state.files.set(item.image_path, { data: PNG_1X1, contentType: 'image/png' });
}
state.outfits.push(
  makeOutfit('d1000000-0000-4000-8000-000000000001', daysAgoKey(3), [SEED[0].id, SEED[1].id], 'seeded errand day'),
  makeOutfit('d2000000-0000-4000-8000-000000000002', daysAgoKey(6), [SEED[2].id], null),
  makeOutfit('d3000000-0000-4000-8000-000000000003', daysAgoKey(10), [SEED[0].id, SEED[3].id], null),
);

function publicSettings() {
  const s = state.settings;
  return {
    tryon_provider: s.tryon_provider,
    has_api_key: s.has_api_key,
    api_key_hint: s.api_key_hint,
    local_qwen_url: s.local_qwen_url,
    reference_image_path: s.reference_image_path,
    discord_user_id: s.discord_user_id,
  };
}

function sessionPayload() {
  return {
    access_token: ACCESS_TOKEN,
    token_type: 'bearer',
    expires_in: 3600,
    refresh_token: 'e2e-refresh-token',
    user: {
      id: USER.id,
      aud: 'authenticated',
      role: 'authenticated',
      email: USER.email,
      email_confirmed_at: '2026-01-01T00:00:00.000Z',
      created_at: '2026-01-01T00:00:00.000Z',
    },
  };
}

function sendJson(res, status, body, extraHeaders = {}) {
  // String bodies pass through raw: the backend's (StatusCode, String)
  // error responses are plain text, and the frontend reads them with res.text().
  const payload = typeof body === 'string' ? body : JSON.stringify(body);
  res.writeHead(status, { 'content-type': 'application/json', ...extraHeaders });
  res.end(payload);
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}

/** Extract the single `file` part of a multipart/form-data body. */
function parseMultipart(body, contentType) {
  const match = /boundary=(?:"([^"]+)"|([^;]+))/i.exec(contentType || '');
  if (!match) return null;
  const boundary = Buffer.from('--' + (match[1] || match[2]).trim());
  let partStart = body.indexOf(boundary);
  if (partStart === -1) return null;
  partStart += boundary.length;
  for (;;) {
    const partEnd = body.indexOf(boundary, partStart);
    if (partEnd === -1) return null;
    let end = partEnd;
    if (body[end - 1] === 0x0a) end -= 1;
    if (body[end - 1] === 0x0d) end -= 1;
    const headersEnd = body.indexOf('\r\n\r\n', partStart);
    if (headersEnd !== -1 && headersEnd < end) {
      const headers = body.slice(partStart, headersEnd).toString('utf8');
      const name = /name="([^"]*)"/i.exec(headers);
      const filename = /filename="([^"]*)"/i.exec(headers);
      const partType = /content-type:\s*([^\r\n]+)/i.exec(headers);
      if (name && name[1] === 'file') {
        return {
          filename: filename ? filename[1] : 'file',
          contentType: partType ? partType[1].trim() : 'application/octet-stream',
          data: body.slice(headersEnd + 4, end),
        };
      }
    }
    partStart = partEnd + boundary.length;
  }
}

function authed(req) {
  return req.headers.authorization === `Bearer ${ACCESS_TOKEN}`;
}

async function handleApi(req, res) {
  const url = new URL(req.url, `http://localhost:${API_PORT}`);
  const path = url.pathname;
  const method = req.method;

  // The signed-token endpoint must be checked before the /files/* wildcard.
  if (method === 'GET' && path === '/files/sign') {
    if (!authed(req)) return sendJson(res, 401, 'unauthorized');
    return sendJson(res, 200, {
      token: 'e2e-file-token',
      expires_at: Math.floor(Date.now() / 1000) + 600,
    });
  }
  // Image serving: <img> tags cannot send headers, so no auth here (the real
  // server gates this with JWT/signed tokens; the stub accepts anything).
  if (method === 'GET' && path.startsWith('/files/')) {
    const file = state.files.get(path.slice('/files/'.length));
    if (!file) return sendJson(res, 404, 'not found');
    res.writeHead(200, { 'content-type': file.contentType, 'cache-control': 'private, max-age=86400' });
    return res.end(file.data);
  }

  if (!authed(req)) return sendJson(res, 401, 'unauthorized');

  const body = await readBody(req);
  const json = () => JSON.parse(body.length ? body.toString('utf8') : '{}');

  if (method === 'GET' && path === '/tables') return sendJson(res, 200, []);

  const tableMap = {
    'clothing-items': state.items,
    colors: state.colors,
    'clothing-categories': state.categories,
    'clothing-subcategories': state.subcategories,
    brands: state.brands,
  };
  const dbRead = path.match(/^\/db\/([^/]+)$/);
  if (dbRead && method === 'GET') {
    const rows = tableMap[dbRead[1]];
    if (!rows) return sendJson(res, 404, 'unknown table');
    const offset = Number(url.searchParams.get('offset') || 0);
    const limit = Number(url.searchParams.get('limit') || 100);
    return sendJson(res, 200, rows.slice(offset, offset + limit), {
      'x-total-count': String(rows.length),
    });
  }
  if (path === '/db/clothing-items' && method === 'POST') {
    const payload = json();
    if (!payload.name || typeof payload.name !== 'string') {
      return sendJson(res, 400, 'name is required');
    }
    const item = {
      id: crypto.randomUUID(),
      user_id: USER.id,
      name: payload.name,
      category_id: payload.category_id ?? null,
      subcategory_id: payload.subcategory_id ?? null,
      color_id: payload.color_id ?? null,
      brand_id: payload.brand_id ?? null,
      purchase_price: payload.purchase_price ?? null,
      owned_since: payload.owned_since ?? null,
      origin: payload.origin ?? null,
      laundry_impact: payload.laundry_impact ?? null,
      repairable: payload.repairable ?? null,
      notes: payload.notes ?? null,
      image_path: payload.image_path ?? null,
      is_active: payload.is_active ?? true,
      wear_count: payload.wear_count ?? 0,
      created_at: nowIso(),
      updated_at: nowIso(),
    };
    state.items.push(item);
    return sendJson(res, 200, item);
  }
  const dbPatch = path.match(/^\/db\/([^/]+)\/([^/]+)$/);
  if (dbPatch && method === 'PATCH') {
    const rows = tableMap[dbPatch[1]];
    const row = rows?.find((r) => String(r.id) === dbPatch[2]);
    if (!rows || !row) return sendJson(res, 404, 'row not found');
    for (const [key, value] of Object.entries(json())) {
      if (key !== 'id' && key !== 'user_id' && key !== 'created_at') row[key] = value;
    }
    row.updated_at = nowIso();
    return sendJson(res, 200, row);
  }

  if (path === '/upload' && method === 'POST') {
    const file = parseMultipart(body, req.headers['content-type']);
    if (!file || file.data.length === 0) return sendJson(res, 400, 'missing file part');
    const safeName = file.filename.replace(/[^a-zA-Z0-9._-]/g, '_') || 'upload.png';
    const key = `users/${USER.id}/${crypto.randomUUID()}-${safeName}`;
    state.files.set(key, { data: file.data, contentType: file.contentType });
    // The real handler returns Json<String>: a JSON-encoded string, not bare text.
    return sendJson(res, 200, JSON.stringify(`http://localhost:${API_PORT}/files/${key}`));
  }

  if (path === '/outfits' && method === 'GET') {
    const offset = Number(url.searchParams.get('offset') || 0);
    const limit = Number(url.searchParams.get('limit') || 100);
    return sendJson(res, 200, state.outfits.slice(offset, offset + limit), {
      'x-total-count': String(state.outfits.length),
    });
  }
  if (path === '/outfits' && method === 'POST') {
    const payload = json();
    if (!/^\d{4}-\d{2}-\d{2}$/.test(payload.date || '')) return sendJson(res, 400, 'invalid date');
    const ids = Array.isArray(payload.item_ids) ? payload.item_ids : [];
    for (const id of ids) {
      if (!state.items.some((i) => i.id === id)) {
        return sendJson(res, 400, `item ${id} is not in your wardrobe`);
      }
    }
    const outfit = {
      id: crypto.randomUUID(),
      date: payload.date,
      notes: payload.notes || null,
      image_path: payload.image_path || null,
      is_active: true,
      created_at: nowIso(),
      item_ids: ids,
    };
    state.outfits.push(outfit);
    return sendJson(res, 200, outfit);
  }
  const outfitDelete = path.match(/^\/outfits\/([^/]+)$/);
  if (outfitDelete && method === 'DELETE') {
    const idx = state.outfits.findIndex((o) => o.id === outfitDelete[1]);
    if (idx === -1) return sendJson(res, 404, 'not found');
    state.outfits.splice(idx, 1);
    return sendJson(res, 200, null);
  }
  if (path === '/outfits/discord-sync' && method === 'POST') {
    return sendJson(res, 200, {
      imported_count: 0,
      messages_processed: 0,
      new_items_created: 0,
      summary: [],
    });
  }
  if (path === '/outfits/generate-preview' && method === 'POST') {
    return sendJson(res, 400, 'try-on generation is not supported by the e2e stub');
  }

  if (path === '/settings' && method === 'GET') return sendJson(res, 200, publicSettings());
  if (path === '/settings' && method === 'PUT') {
    const payload = json();
    const s = state.settings;
    if (payload.tryon_provider !== undefined) {
      if (payload.tryon_provider !== 'gemini' && payload.tryon_provider !== 'qwen_local') {
        return sendJson(res, 400, 'tryon_provider must be gemini or qwen_local');
      }
      s.tryon_provider = payload.tryon_provider;
    }
    if (payload.api_key !== undefined) {
      if (payload.api_key === '') {
        s.has_api_key = false;
        s.api_key_hint = null;
      } else {
        s.has_api_key = true;
        s.api_key_hint = '...' + payload.api_key.slice(-4);
      }
    }
    if (payload.local_qwen_url !== undefined) {
      if (!/^https?:\/\/\S+$/.test(payload.local_qwen_url)) {
        return sendJson(res, 400, 'invalid local Qwen URL');
      }
      s.local_qwen_url = payload.local_qwen_url;
    }
    if (payload.reference_image_path !== undefined) {
      s.reference_image_path = payload.reference_image_path === '' ? null : payload.reference_image_path;
    }
    return sendJson(res, 200, publicSettings());
  }
  if (path === '/settings/test-qwen' && method === 'GET') {
    return sendJson(res, 200, {
      status: 'ok',
      model: 'qwen-image-2.1-turbo-Q6_K_XL.gguf',
      ready: true,
    });
  }

  // The "+ Add new…" affordances on the item form.
  if (path === '/clothing-categories' && method === 'POST') {
    const payload = json();
    const nextId = state.categories.length ? Math.max(...state.categories.map((c) => c.id)) + 1 : 1;
    const cat = { id: nextId, name: payload.name || 'New Category' };
    state.categories.push(cat);
    return sendJson(res, 200, cat);
  }
  if (path === '/clothing-subcategories' && method === 'POST') {
    const payload = json();
    const nextId = state.subcategories.length
      ? Math.max(...state.subcategories.map((c) => c.id)) + 1
      : 1;
    const sub = { id: nextId, name: payload.name || 'New Subcategory', category_id: payload.category_id || 1 };
    state.subcategories.push(sub);
    return sendJson(res, 200, sub);
  }
  if (path === '/colors' && method === 'POST') {
    const payload = json();
    const id = payload.id || 'custom';
    const color = { id, hex_value: payload.hex_value || null };
    state.colors.push(color);
    return sendJson(res, 200, color);
  }
  if (path === '/brands' && method === 'POST') {
    const payload = json();
    const brand = { id: crypto.randomUUID(), name: payload.name || 'New Brand', user_id: USER.id };
    state.brands.push(brand);
    return sendJson(res, 200, brand);
  }

  // M5 surface: outfit editing, export/import, reference usage and the
  // dedicated rename/delete endpoints. Thin mirrors of the real handlers —
  // enough for the settings page to load and the flows to exercise them.
  if (outfitDelete && method === 'PATCH') {
    const idx = state.outfits.findIndex((o) => o.id === outfitDelete[1]);
    if (idx === -1) return sendJson(res, 404, 'not found');
    const payload = json();
    if (!/^\d{4}-\d{2}-\d{2}$/.test(payload.date || '')) return sendJson(res, 400, 'invalid date');
    const ids = Array.isArray(payload.item_ids) ? payload.item_ids : [];
    for (const id of ids) {
      if (!state.items.some((i) => i.id === id)) {
        return sendJson(res, 400, `item ${id} is not in your wardrobe`);
      }
    }
    // Wear counts follow the link diff, like the real handler.
    const prev = new Set(state.outfits[idx].item_ids || []);
    const next = new Set(ids);
    for (const item of state.items) {
      if (next.has(item.id) && !prev.has(item.id)) item.wear_count = (item.wear_count || 0) + 1;
      else if (prev.has(item.id) && !next.has(item.id)) item.wear_count = Math.max(0, (item.wear_count || 0) - 1);
    }
    state.outfits[idx] = {
      ...state.outfits[idx],
      date: payload.date,
      notes: payload.notes ?? null,
      image_path: payload.image_path ?? null,
      item_ids: ids,
      updated_at: nowIso(),
    };
    return sendJson(res, 200, state.outfits[idx]);
  }

  if (method === 'GET' && path === '/export') {
    return sendJson(
      res,
      200,
      {
        version: 1,
        exported_at: nowIso(),
        user_id: USER.id,
        items: state.items,
        outfits: state.outfits,
        colors: state.colors,
        categories: state.categories,
        subcategories: state.subcategories,
        brands: state.brands,
      },
      { 'content-disposition': 'attachment; filename="cloth-web-export.json"' },
    );
  }

  if (method === 'POST' && path === '/import') {
    const doc = json();
    const result = {
      items_imported: 0,
      outfits_imported: 0,
      links_added: 0,
      colors_created: 0,
      categories_created: 0,
      subcategories_created: 0,
      brands_created: 0,
    };
    for (const c of doc.colors || []) {
      if (c && c.id && !state.colors.some((x) => x.id === c.id)) {
        state.colors.push({ id: c.id, hex_value: c.hex_value ?? null });
        result.colors_created += 1;
      }
    }
    for (const c of doc.categories || []) {
      if (c && c.name && !state.categories.some((x) => x.name.toLowerCase() === c.name.toLowerCase())) {
        const nextId = state.categories.length ? Math.max(...state.categories.map((x) => x.id)) + 1 : 1;
        state.categories.push({ id: nextId, name: c.name });
        result.categories_created += 1;
      }
    }
    for (const s of doc.subcategories || []) {
      if (s && s.name && !state.subcategories.some((x) => x.name.toLowerCase() === s.name.toLowerCase())) {
        const nextId = state.subcategories.length ? Math.max(...state.subcategories.map((x) => x.id)) + 1 : 1;
        state.subcategories.push({ id: nextId, name: s.name, category_id: s.category_id || 1 });
        result.subcategories_created += 1;
      }
    }
    for (const b of doc.brands || []) {
      if (b && b.name && !state.brands.some((x) => x.name.toLowerCase() === b.name.toLowerCase())) {
        state.brands.push({ id: crypto.randomUUID(), name: b.name, user_id: USER.id });
        result.brands_created += 1;
      }
    }
    for (const item of doc.items || []) {
      if (item && item.id && !state.items.some((x) => x.id === item.id)) {
        state.items.push({ ...item, user_id: USER.id });
        result.items_imported += 1;
      }
    }
    for (const o of doc.outfits || []) {
      if (o && o.id && !state.outfits.some((x) => x.id === o.id)) {
        state.outfits.push({ ...o, user_id: USER.id, updated_at: nowIso() });
        result.outfits_imported += 1;
        result.links_added += (o.item_ids || []).length;
      }
    }
    return sendJson(res, 200, result);
  }

  if (method === 'GET' && path === '/reference-usage') {
    const usage = { colors: {}, categories: {}, subcategories: {}, brands: {} };
    for (const c of state.colors) usage.colors[c.id] = 0;
    for (const c of state.categories) usage.categories[c.name] = 0;
    for (const s of state.subcategories) usage.subcategories[s.name] = 0;
    for (const b of state.brands) usage.brands[b.name] = 0;
    for (const item of state.items) {
      if (item.color_id && item.color_id in usage.colors) usage.colors[item.color_id] += 1;
      const cat = state.categories.find((c) => c.id === item.category_id);
      if (cat) usage.categories[cat.name] = (usage.categories[cat.name] || 0) + 1;
      const sub = state.subcategories.find((s) => s.id === item.subcategory_id);
      if (sub) usage.subcategories[sub.name] = (usage.subcategories[sub.name] || 0) + 1;
      const brand = state.brands.find((b) => b.id === item.brand_id);
      if (brand) usage.brands[brand.name] = (usage.brands[brand.name] || 0) + 1;
    }
    return sendJson(res, 200, usage);
  }

  const colorRef = path.match(/^\/colors\/([^/]+)$/);
  if (colorRef) {
    const color = state.colors.find((c) => c.id === colorRef[1]);
    if (!color) return sendJson(res, 404, 'color not found');
    if (method === 'PATCH') {
      const payload = json();
      const inUse = state.items.filter((i) => i.color_id === color.id).length;
      if (inUse > 0 && !color.user_id) return sendJson(res, 409, 'color is in use');
      const taken = state.colors.some((c) => c.id === (payload.id || '').toLowerCase() && c.id !== color.id);
      if (taken) return sendJson(res, 409, 'color slug already exists');
      for (const item of state.items) if (item.color_id === color.id) item.color_id = (payload.id || '').toLowerCase();
      color.id = (payload.id || '').toLowerCase();
      return sendJson(res, 200, color);
    }
    if (method === 'DELETE') {
      const inUse = state.items.filter((i) => i.color_id === color.id).length;
      if (inUse > 0) return sendJson(res, 409, 'color is in use');
      state.colors = state.colors.filter((c) => c.id !== color.id);
      return sendJson(res, 200, { deleted: true, items_affected: 0 });
    }
  }
  const categoryRef = path.match(/^\/clothing-categories\/([^/]+)$/);
  if (categoryRef) {
    const category = state.categories.find((c) => c.id === Number(categoryRef[1]));
    if (!category) return sendJson(res, 404, 'category not found');
    if (method === 'PATCH') {
      const payload = json();
      category.name = payload.name || category.name;
      return sendJson(res, 200, category);
    }
    if (method === 'DELETE') {
      const inUse = state.items.filter((i) => i.category_id === category.id).length;
      state.categories = state.categories.filter((c) => c.id !== category.id);
      state.subcategories = state.subcategories.filter((s) => s.category_id !== category.id);
      for (const item of state.items) {
        if (item.category_id === category.id) item.category_id = null;
      }
      return sendJson(res, 200, { deleted: true, items_affected: inUse });
    }
  }
  const subcategoryRef = path.match(/^\/clothing-subcategories\/([^/]+)$/);
  if (subcategoryRef) {
    const sub = state.subcategories.find((s) => s.id === Number(subcategoryRef[1]));
    if (!sub) return sendJson(res, 404, 'subcategory not found');
    if (method === 'PATCH') {
      const payload = json();
      sub.name = payload.name || sub.name;
      if (payload.category_id != null) sub.category_id = payload.category_id;
      return sendJson(res, 200, sub);
    }
    if (method === 'DELETE') {
      const inUse = state.items.filter((i) => i.subcategory_id === sub.id).length;
      state.subcategories = state.subcategories.filter((s) => s.id !== sub.id);
      for (const item of state.items) {
        if (item.subcategory_id === sub.id) item.subcategory_id = null;
      }
      return sendJson(res, 200, { deleted: true, items_affected: inUse });
    }
  }
  const brandRef = path.match(/^\/brands\/([^/]+)$/);
  if (brandRef) {
    const brand = state.brands.find((b) => b.id === brandRef[1]);
    if (!brand) return sendJson(res, 404, 'brand not found');
    if (method === 'PATCH') {
      const payload = json();
      brand.name = payload.name || brand.name;
      return sendJson(res, 200, brand);
    }
    if (method === 'DELETE') {
      const inUse = state.items.filter((i) => i.brand_id === brand.id).length;
      state.brands = state.brands.filter((b) => b.id !== brand.id);
      for (const item of state.items) {
        if (item.brand_id === brand.id) item.brand_id = null;
      }
      return sendJson(res, 200, { deleted: true, items_affected: inUse });
    }
  }

  return sendJson(res, 404, `no stub route for ${method} ${path}`);
}

// ---------------------------------------------------------------------------
// Fake Supabase GoTrue: just enough for signInWithPassword / refresh / logout.
// Cross-origin (the page lives on :5173), so it must answer CORS.
// ---------------------------------------------------------------------------

function handleAuth(req, res) {
  const origin = req.headers.origin || '*';
  res.setHeader('access-control-allow-origin', origin);
  // Echo whatever headers the client preflighted for (supabase-js adds
  // x-client-info, x-supabase-api-version, and friends between releases).
  res.setHeader(
    'access-control-allow-headers',
    req.headers['access-control-request-headers'] ||
      'authorization, apikey, content-type, x-client-info, x-supabase-api-version',
  );
  res.setHeader('access-control-allow-methods', 'GET, POST, OPTIONS');
  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    return res.end();
  }

  const url = new URL(req.url, `http://localhost:${AUTH_PORT}`);
  if (req.method === 'POST' && url.pathname === '/auth/v1/token') {
    const grant = url.searchParams.get('grant_type');
    if (grant === 'password') {
      return readBody(req).then((body) => {
        let email;
        let password;
        try {
          ({ email, password } = JSON.parse(body.toString('utf8')));
        } catch {
          return sendJson(res, 400, { error_code: 'bad_json', message: 'Invalid JSON' });
        }
        if (email === USER.email && password === USER.password) {
          return sendJson(res, 200, sessionPayload());
        }
        return sendJson(res, 400, {
          error_code: 'creds_invalid',
          message: 'Invalid login credentials',
        });
      });
    }
    if (grant === 'refresh_token') {
      return sendJson(res, 200, sessionPayload());
    }
    return sendJson(res, 400, { error_code: 'unsupported_grant', message: `Unsupported grant: ${grant}` });
  }
  if (req.method === 'POST' && url.pathname === '/auth/v1/logout') {
    res.writeHead(204);
    return res.end();
  }
  return sendJson(res, 404, {});
}

const apiServer = http.createServer((req, res) => {
  handleApi(req, res).catch((err) => {
    console.error('[e2e-api]', err);
    if (!res.headersSent) sendJson(res, 500, 'stub error');
  });
});
const authServer = http.createServer((req, res) => {
  try {
    handleAuth(req, res);
  } catch (err) {
    console.error('[e2e-auth]', err);
    if (!res.headersSent) sendJson(res, 500, 'stub error');
  }
});

apiServer.listen(API_PORT, '127.0.0.1', () => {
  console.log(`[e2e] API stub listening on http://127.0.0.1:${API_PORT}`);
});
authServer.listen(AUTH_PORT, '127.0.0.1', () => {
  console.log(`[e2e] auth stub listening on http://127.0.0.1:${AUTH_PORT}`);
});
