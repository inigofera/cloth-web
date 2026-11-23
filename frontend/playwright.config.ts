import { defineConfig } from '@playwright/test';

// Build-time env for the dev server: point the Supabase client at the local
// GoTrue stub. Everything else the page talks to is proxied /api -> :3000,
// where the API stub answers (see e2e/stub-server.mjs).
const ENV = {
  VITE_SUPABASE_URL: 'http://127.0.0.1:5433',
  VITE_SUPABASE_PUBLISHABLE_KEY: 'e2e-stub-key',
  VITE_SUPABASE_BUCKET: 'clothing-wardrobe',
};

export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  expect: { timeout: 10_000 },
  fullyParallel: true,
  workers: 4,
  retries: 1,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'retain-on-failure',
  },
  webServer: [
    {
      // One process serves both stubs; wait on the auth port.
      command: 'node e2e/stub-server.mjs',
      port: 5433,
      stdout: 'pipe',
    },
    {
      command: 'npm run dev',
      port: 5173,
      env: ENV,
    },
  ],
});
