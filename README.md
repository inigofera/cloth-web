# cloth-web

A web app for managing clothing items and outfits, with analytics and AI-assisted
virtual try-on, backed by Supabase (Postgres + Storage).

## Stack

- **Backend** — Rust / Axum on port 3000 (`src/`): generic paginated table CRUD, outfit management, encrypted user settings, image upload/resize, virtual try-on orchestration, and optional Discord import. Auth via Supabase JWTs (HS256, `authenticated` role only).
- **Frontend** — Vue 3 + TypeScript + Vite (`frontend/`), using the Supabase JS client for auth and a custom Material Design 3 design system with zero UI-framework dependencies.
- **Local inference (optional)** — FastAPI worker (`local_inference/`) wrapping `stable-diffusion.cpp` for self-hosted try-on with `Qwen-Image-2.1-Turbo` (GGUF).
- **Deployment** — nginx serves the built frontend and proxies `/api/` to the backend (`infra/nginx.conf`).

## Application views

| Tab | What it does |
|-----|--------------|
| **Clothes** | Wardrobe grid with search and category filters, add/edit form (inline creation of colors, categories, subcategories, brands), image upload, deactivate with undo, and deep-linkable item details. |
| **Outfits** | Calendar outfit logger. Today is preselected; the item picker is recency-sorted. Supports outfit photos and a virtual try-on preview, plus Discord sync of logged outfits. |
| **Insights** | Analytics dashboard with 19 configurable widget types (usage, wardrobe, value, sustainability, relationships, callouts). Widgets are reorderable by drag or keyboard, globally filterable (date range, category, color, brand, active-only), and the layout persists per browser. |
| **Tables** | Developer-facing PostgreSQL schema explorer. |
| **Settings** | Try-on configuration: provider selection (Gemini BYOK or local Qwen), API key, full-body reference photo, connection test, and Discord account linking. |

## Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/tables` | GET | List public tables with columns and types |
| `/db/:table` | GET / POST | Read rows (`?limit=`, `?offset=`; up to 10,000 rows; total reported via `X-Total-Count` header) / insert a row |
| `/db/:table/:id` | PATCH / DELETE | Update / delete a row (user-scoped tables only) |
| `/colors` | POST | Create a color (reference data) |
| `/clothing-categories` | POST | Create a category (reference data) |
| `/clothing-subcategories` | POST | Create a subcategory (reference data) |
| `/outfits` | GET / POST | List outfits (`?limit=`, `?offset=`, `X-Total-Count`) / create an outfit (transactionally increments wear counts) |
| `/outfits/:id` | DELETE | Delete an outfit (transactionally decrements wear counts) |
| `/outfits/generate-preview` | POST | Generate a virtual try-on image for the selected items |
| `/outfits/discord-sync` | POST | Import outfits logged via Discord |
| `/settings` | GET / PUT | Read / update the user's settings (API key stored AES-256-GCM encrypted; provider, local Qwen URL, reference photo, Discord link) |
| `/settings/test-qwen` | GET | Probe the configured local Qwen server |
| `/settings/discord-code` | POST | Generate a Discord account link code |
| `/settings/discord-unlink` | POST | Unlink the Discord account |
| `/upload` | POST | Upload an image to Supabase Storage, returns public URL |
| `/files/sign` | GET | Issue the caller a short-lived (10 min) signed token for referencing their own images from `<img>` tags |
| `/files/*path` | GET | Serve the user's own stored images, with optional `?width=` resizing (capped at 4096px). Auth via Bearer JWT, or `?token=<signed>&exp=<unix>` from `/files/sign` — JWTs are never placed in URLs |

Tables with a `user_id` column are scoped to the authenticated user; tables without one are shared reference data (read-only). The `user-settings` table is explicitly blocked from the generic `/db/:table` endpoints and only reachable through `/settings`.

## Environment variables

Backend (`.env`):

```
DATABASE_URL                 # Supabase Postgres connection string (required)
SUPABASE_JWT_SECRET          # JWT verification secret (required)
SUPABASE_URL                 # Supabase project URL (required)
SUPABASE_SERVICE_ROLE_KEY    # service-role key (server-side storage access, required)
SUPABASE_STORAGE_BUCKET      # bucket name for uploaded files (required)
SETTINGS_ENCRYPTION_KEY      # 64-char hex string for AES-256-GCM (openssl rand -hex 32, required)
ALLOW_LAN_INFERENCE_URLS     # "true" to allow non-loopback local Qwen URLs (default: false)
QWEN_WIDTH                   # local try-on output width (default: 832)
QWEN_HEIGHT                  # local try-on output height (default: 1248)
QWEN_TIMEOUT_SECONDS         # local try-on request timeout (default: 300)
DISCORD_BOT_TOKEN            # Discord bot token (optional; enables Discord import)
DISCORD_CHANNEL_ID           # Discord channel to import outfits from (optional)
DISCORD_CREATOR_ID           # Discord user id auto-linked on first message (optional)
DISCORD_DEFAULT_USER_ID      # app user id the startup sweep imports for (optional)
```

Frontend (`frontend/.env`):

```
VITE_SUPABASE_URL            # Supabase project URL
VITE_SUPABASE_PUBLISHABLE_KEY# public anon key for client authentication
VITE_SUPABASE_BUCKET         # bucket name (default: clothing-wardrobe)
```

## Upload security policy

Client filenames are treated as untrusted input. Every upload is:

1. **Size-capped** — max 10 MiB (`413` above that).
2. **Filename-sanitized** — path components stripped (no traversal), only `[A-Za-z0-9-_]` kept in the name, extension must be one of `jpg`, `jpeg`, `png`, `webp`, `gif`.
3. **Content-validated** — magic-byte sniffing rejects payloads that don't actually start with a JPEG/PNG/GIF/WEBP signature (catches renamed executables/scripts).
4. **Stored per-user, collision-free** — objects go to `users/{user_id}/{uuid}-{name}`, so uploads can never overwrite or collide between users (or within one user's own uploads).

Server-side processing: EXIF auto-orientation, downsampling to a 1600px max dimension (Lanczos3), re-encoding to progressive JPEG (quality 80; alpha-channel images left untouched), and thumbnail derivatives written at upload time. The `/files` proxy enforces the same per-user path ownership and applies a 4096px ceiling to resize requests. See `src/upload.rs` for the implementation and tests.

## Virtual try-on

The Outfits tab can generate a composite "wearing this outfit" image, with the provider chosen in Settings:

- **Gemini (BYOK)** — the user's own API key, encrypted at rest (AES-256-GCM, never returned to the client). The backend downloads the reference photo and garment images, builds a metadata-driven multimodal prompt, and calls `gemini-2.5-flash-image`.
- **Local Qwen** — dispatches to the user's configured `local_inference/` FastAPI worker (SSRF-validated; loopback-only unless `ALLOW_LAN_INFERENCE_URLS=true`), running `Qwen-Image-2.1-Turbo` GGUF via `stable-diffusion.cpp`.

Generated images pass through the same sanitization/normalization pipeline as uploads and are stored under `users/{user_id}/{uuid}-tryon.jpg`, then attachable to the outfit.

## Discord integration (optional)

With a bot token and channel configured, users can link their Discord account in Settings (6-digit code, valid 15 minutes) and import outfit logs posted in the channel. After five failed `!link` attempts a Discord user is locked out of the link flow for one hour. A startup sweep also imports any new messages.

## Database migrations

Migrations live in `supabase/migrations/` (apply them when setting up the Supabase project):

| Migration | What it does |
|-----------|--------------|
| `20260922000000_constrain_clothing_item_select_fields.sql` | CHECK constraints on `origin` and `laundry_impact` |
| `20261018000000_create_user_settings.sql` | `user-settings` table (encrypted API key, reference photo) with RLS |
| `20261018000001_add_tryon_provider_columns.sql` | try-on provider + local Qwen URL columns |
| `20261026000000_add_discord_integration.sql` | Discord link columns on `user-settings` |
| `20261105000000_index_user_scoped_tables.sql` | `(user_id)` and `(user_id, created_at DESC)` indexes on all user-scoped tables |
| `20261119000000_discord_link_rate_limit.sql` | `discord-link-attempts` table: failed `!link` counts and 1-hour lockout (blocked from the generic API) |

## Build and run

- **Backend development**: `cargo run` (starts on `http://localhost:3000`)
- **Backend testing**: `cargo test`
- **Frontend development**: `npm run dev` in `frontend/` (Vite on `http://localhost:5173`, proxies `/api/` to the backend)
- **Frontend testing**: `npm test` in `frontend/` (Vitest)
- **Frontend typecheck**: `npm run typecheck` in `frontend/` (`tsc --noEmit`)
- **Frontend production build**: `npm run build` in `frontend/` (outputs to `frontend/dist/`)
- **Local inference worker (optional)**:
  ```bash
  cd local_inference
  ./download_models.sh                     # GGUF weights, needs huggingface-cli
  pip install -r requirements.txt
  uvicorn server:app --host 127.0.0.1 --port 8001   # needs sd-cli on PATH
  ```
- **Production**: nginx serves `frontend/dist/` and proxies `/api/` to the backend — see `infra/nginx.conf`.

## Testing

- **Backend** (`cargo test`): unit tests covering filename sanitization, magic-byte sniffing, per-user storage keys and path authorization, AES-256-GCM round-trips, SSRF URL validation, image cache, and Discord message parsing.
- **Frontend** (`npm test`): Vitest suites for date/timezone handling, error classification, routing, sanitizers, the undo manager, insights metrics, layout persistence, widget smoke tests, and picker recency ordering.
