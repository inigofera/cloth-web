import { defaultExclude, defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  plugins: [vue()],
  test: {
    // The Playwright smoke suite lives in e2e/ and is driven by
    // playwright.config.ts, not by vitest.
    exclude: [...defaultExclude, 'e2e/**'],
  },
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
        timeout: 300000,
      }
    }
  }
});
