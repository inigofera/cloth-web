# cloth-web

A web app for managing clothing items and outfits, backed by Supabase (Postgres + Storage).

## Stack

- **Backend** — Rust / Axum on port 3000 (`src/`): generic table CRUD, outfit management, file uploads. Auth via Supabase JWTs (HS256, `authenticated` role only).
- **Frontend** — Vue 3 + TypeScript + Vite (`frontend/`), using the Supabase JS client for auth and API calls.
- **Deployment** — nginx serves the built frontend and proxies `/api/` to the backend (`infra/nginx.conf`).

## Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/tables` | GET | List public tables with columns and types |
| `/db/:table` | GET / POST | Read rows (max 100) / insert a row |
| `/db/:table/:id` | PATCH / DELETE | Update / delete a row (user-scoped tables only) |
| `/outfits` | GET / POST | List outfits with items / create an outfit |
| `/upload` | POST | Upload an image to Supabase Storage, returns public URL |

Tables with a `user_id` column are scoped to the authenticated user; tables without one are shared reference data (read-only).

## Environment variables

```
DATABASE_URL                 # Supabase Postgres connection string
SUPABASE_JWT_SECRET          # JWT verification secret
SUPABASE_URL                 # Supabase project URL
SUPABASE_SERVICE_ROLE_KEY    # service-role key (server-side storage uploads)
SUPABASE_STORAGE_BUCKET      # bucket name for uploaded files
```

## Upload security policy

Client filenames are treated as untrusted input. Every upload is:

1. **Size-capped** — max 10 MiB (`413` above that).
2. **Filename-sanitized** — path components stripped (no traversal), only `[A-Za-z0-9-_]` kept in the name, extension must be one of `jpg`, `jpeg`, `png`, `webp`, `gif`.
3. **Content-validated** — magic-byte sniffing rejects payloads that don't actually start with a JPEG/PNG/GIF/WEBP signature (catches renamed executables/scripts).
4. **Stored per-user, collision-free** — objects go to `users/{user_id}/{uuid}-{name}`, so uploads can never overwrite or collide between users (or within one user's own uploads).

See `src/upload.rs` for the implementation and tests.
