import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

/**
 * `imageUrl` must hand `<img>` tags the short-lived signed file token from
 * `GET /files/sign` — never the long-lived JWT (which would leak into access
 * logs) — and must not serve a token that is about to expire. The Supabase
 * client and `fetch` are stubbed; everything in between is the real code.
 */

const state = vi.hoisted(() => ({
  listeners: [] as Array<(event: string, session: { access_token: string } | null) => void>,
}));

vi.mock('../lib/supabase', () => ({
  supabase: {
    auth: {
      // The client only reads the session for the Authorization header;
      // the sign endpoint under test does not care about it here.
      getSession: vi.fn().mockResolvedValue({ data: { session: null } }),
      onAuthStateChange: (
        listener: (event: string, session: { access_token: string } | null) => void
      ) => {
        state.listeners.push(listener);
        return { data: { subscription: {} } };
      },
      refreshSession: vi.fn().mockResolvedValue({ data: { session: null } }),
    },
  },
}));

import { imageUrl } from './client';

// A well-formed backend object path: users/{user_id}/{uuid}-{name}.{ext}
const PATH =
  'users/11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222-Blue-Shirt.jpg';

let fetchMock: ReturnType<typeof vi.fn>;

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
  });
}

function signCalls(): number {
  return fetchMock.mock.calls.filter((c) => String(c[0]).includes('/files/sign')).length;
}

function emitAuth(event: string, session: { access_token: string } | null): void {
  for (const listener of state.listeners) listener(event, session);
}

// Cross a macrotask boundary so fire-and-forget async work (the client's
// `void fetchFileToken()`) has run and settled.
async function settle(): Promise<void> {
  await new Promise((r) => setTimeout(r, 0));
}

beforeEach(() => {
  fetchMock = vi.fn();
  vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('imageUrl', () => {
  it('embeds the signed file token, never the JWT', async () => {
    const exp = Math.floor(Date.now() / 1000) + 600;
    fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
      expect(String(input)).toContain('/files/sign');
      return jsonResponse({ token: 'SIG', expires_at: exp });
    });

    emitAuth('SIGNED_IN', { access_token: 'SECRET-JWT' });
    await vi.waitFor(() => expect(imageUrl(PATH)).toContain('token=SIG'));

    const url = imageUrl(PATH) as string;
    expect(url).toBe(`/api/files/${PATH}?token=SIG&exp=${exp}`);
    expect(url).not.toContain('SECRET-JWT');
    expect(imageUrl(PATH, { width: 96 })).toBe(`/api/files/${PATH}?width=96&token=SIG&exp=${exp}`);
  });

  it('omits an expired token and refreshes it in the background', async () => {
    // Below the 30 s safety margin the client refuses to embed.
    const exp = Math.floor(Date.now() / 1000) + 10;
    fetchMock.mockImplementation(async () => jsonResponse({ token: 'OLD', expires_at: exp }));

    emitAuth('SIGNED_IN', { access_token: 'SECRET-JWT' });
    await vi.waitFor(() => expect(signCalls()).toBe(1));
    await settle();

    expect(imageUrl(PATH)).toBe(`/api/files/${PATH}`);

    // Rendering the expired URL triggered the background re-sign.
    await vi.waitFor(() => expect(signCalls()).toBe(2));
  });

  it('serves unsigned URLs after sign-out and signs nothing new', async () => {
    fetchMock.mockImplementation(async () => jsonResponse({ token: 'SIG', expires_at: 9999999999 }));
    emitAuth('SIGNED_IN', { access_token: 'SECRET-JWT' });
    await vi.waitFor(() => expect(imageUrl(PATH)).toContain('token=SIG'));

    const calls = signCalls();
    emitAuth('SIGNED_OUT', null);
    await settle();

    expect(imageUrl(PATH)).toBe(`/api/files/${PATH}`);
    expect(signCalls()).toBe(calls);
  });

  it('returns null for missing or malformed object paths', () => {
    expect(imageUrl(null)).toBeNull();
    expect(imageUrl(undefined)).toBeNull();
    // Traversal, foreign prefixes and non-image extensions are refused.
    expect(imageUrl('users/11111111-1111-4111-8111-111111111111/../../etc/passwd')).toBeNull();
    expect(imageUrl('../other-user/photo.jpg')).toBeNull();
    expect(
      imageUrl('users/11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222-notes.txt')
    ).toBeNull();
  });

  it('accepts full Supabase storage URLs and rewrites them to /api/files', async () => {
    const exp = Math.floor(Date.now() / 1000) + 600;
    fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
      expect(String(input)).toContain('/files/sign');
      return jsonResponse({ token: 'SIG', expires_at: exp });
    });

    emitAuth('SIGNED_IN', { access_token: 'SECRET-JWT' });
    await vi.waitFor(() => expect(imageUrl(PATH)).toContain('token=SIG'));

    const storageUrl = `https://cdn.example.co/storage/v1/object/public/clothing-wardrobe/${PATH}`;
    expect(imageUrl(storageUrl)).toBe(`/api/files/${PATH}?token=SIG&exp=${exp}`);
  });
});

describe('API base URL', () => {
  // The base is read from `import.meta.env` at module load, so each case
  // re-imports a fresh module after setting (or clearing) the variable.
  afterEach(() => {
    delete import.meta.env.VITE_API_BASE_URL;
    vi.resetModules();
  });

  it('defaults to the same-origin /api path in dev (Vite proxy)', async () => {
    delete import.meta.env.VITE_API_BASE_URL;
    vi.resetModules();
    const { imageUrl } = await import('./client');
    expect(imageUrl(PATH)).toBe(`/api/files/${PATH}`);
  });

  it('uses VITE_API_BASE_URL when set (Pages deployment)', async () => {
    import.meta.env.VITE_API_BASE_URL = 'https://api.inigoartolozaga.com';
    vi.resetModules();
    const { imageUrl } = await import('./client');
    expect(imageUrl(PATH)).toBe(`https://api.inigoartolozaga.com/files/${PATH}`);
  });
});
