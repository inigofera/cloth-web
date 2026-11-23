/// <reference types="vite/client" />

interface ImportMetaEnv {
  /**
   * Public API origin for the GitHub Pages deployment
   * (`https://api.inigoartolozaga.com`). Unset in dev: the client falls back
   * to the same-origin `/api` path served by the Vite proxy.
   */
  VITE_API_BASE_URL?: string;
  VITE_SUPABASE_URL?: string;
  VITE_SUPABASE_PUBLISHABLE_KEY?: string;
  VITE_SUPABASE_BUCKET?: string;
}
