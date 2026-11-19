# Home Infrastructure Plan: GitHub Pages frontend + inibaby backend

**Goal:** serve the frontend from GitHub Pages under the domain `inigoartolozaga.com`, and run the
Dockerized backend on the home NUC (`inibaby`), exposed securely to the internet.

**Current state:** one VPS-style box running nginx (serves `frontend/dist/` + proxies `/api/` to
port 3000). Everything same-origin. Backend not yet containerized. Frontend not yet in CI's
deploy path.

```
Today                              Target
────────                          ──────
Browser                            Browser
  │ HTTPS                          │ HTTPS
  │                                │
inigo's server                     inigoartolozaga.com  (GitHub Pages, custom domain)
┌─────────────┐                    │  static assets only
│ nginx       │                    │  (no server code, no API)
│  / → dist/  │                    │
│  /api → :3000│                   │  VITE_API_BASE_URL ──────┐
└─────────────┘                    │                          │ HTTPS (public origin)
  │                                │                          │
┌─────────────┐                    │        ┌─────────────────┴──────────┐
│ backend :3000│                   │        │  Tunnel (Cloudflare or     │
└─────────────┘                    │        │  Tailscale Funnel)         │
  │                                │        └─────────────────┬──────────┘
┌─────────────┐                    │                          │ 127.0.0.1:3000
│ Supabase    │◄──────────────────┴──────────────────────────┤
└─────────────┘                                              ┌──────────────┐
   (Postgres + Storage, unchanged)                           │ inibaby NUC  │
                                                            │ docker:      │
                                                            │ backend :3000│
                                                            └──────────────┘
```

**Why this works:** the frontend talks to two public APIs — Supabase (auth + no direct DB
access; the publishable key is public by design) and our backend (JWT-protected). The backend
is the only piece that needs to be reachable from the internet, and it stays on
`127.0.0.1` behind a tunnel — no port forwarding, no exposed nginx.

---

## Phase 0 — Decisions (pick before building)

| # | Decision | Options | Recommendation |
|---|----------|---------|----------------|
| 0.1 | **How to expose inibaby publicly** | a) **Cloudflare Tunnel** — `cloudflared` daemon on inibaby, zero open ports, free, custom hostname works with *any* DNS provider (one CNAME); b) **Tailscale Funnel** — `tailscale funnel 3000`, zero open ports, `*.ts.net` URL or custom domain; c) Port forwarding + Caddy/Let's Encrypt — classic, more moving parts, permanently exposed port | **Cloudflare Tunnel.** The domain lives on Squarespace (which also hosts the website) — no nameserver migration needed: the tunnel's public hostname is a CNAME you add in Squarespace's DNS panel. The Squarespace site stays untouched. |
| 0.2 | **API base URL** | `https://api.inigoartolozaga.com` (subdomain, official GitHub Pages support, clean) vs `https://inigoartolozaga.com` (apex — GitHub Pages officially supports apex only via A records to their IPs; it works in practice but is less supported) | **Two subdomains:** `cloth.inigoartolozaga.com` (Pages) + `api.inigoartolozaga.com` (tunnel). The apex is taken by the Squarespace website anyway, so subdomains are the only clean option. |
| 0.3 | **How inibaby gets the backend image** | a) **CI builds the image and pushes to GitHub Container Registry (GHCR)** — free for public repos; inibaby just `docker pull`s; single source of truth; b) inibaby does `git pull && docker compose build` — no registry, but the NUC pays the compile time (~5–10 min per deploy) | **GHCR** — CI already compiles in the release profile; the image job reuses it. |
| 0.4 | **Migrations** | a) small `schema_migrations` ledger table + psql loop over `supabase/migrations/*.sql` in an entrypoint; b) Supabase CLI `db push` on inibaby (needs a linked project + service key) | **a)** — self-contained, no CLI state on the NUC, runs on every container start and is a no-op when up to date. |
| 0.5 | **Local inference (Qwen) on inibaby** | Run the `local_inference` worker in a second compose service so the backend can reach it via `http://qwen:8001` | Decide later — compose makes it a one-file change. Backend needs `ALLOW_LAN_INFERENCE_URLS=true` and the container network to allow it. |

**Resolved (2026-11-16):** `inigoartolozaga.com` is registered and DNS-managed on Squarespace,
which also hosts the website (apex + `www`). Decision: **Cloudflare Tunnel** with a custom
hostname — no nameserver migration; two CNAMEs added in Squarespace's Custom records panel.

---

## Phase 1 — Code changes (this repo)

### 1.1 Frontend: configurable API base

`frontend/src/api/client.ts` line 19:

```ts
const API_BASE = '/api';
```

becomes:

```ts
const API_BASE = (import.meta.env.VITE_API_BASE_URL as string) || '/api';
```

- Dev (`npm run dev`): unset → `/api` → Vite proxy (unchanged behavior).
- Pages build: CI sets `VITE_API_BASE_URL=https://api.inigoartolozaga.com` → all `fetch` and
  `imageUrl()` calls hit the public API origin.

Everything else in `client.ts` already prefixes with `API_BASE`, so this is the only change.
Add `VITE_API_BASE_URL` to the CI workflow env (secret or plain — it's a public URL).

### 1.2 Backend: CORS

Today the browser and API are same-origin, so there is no CORS middleware. Once the frontend is
on `cloth.inigoartolozaga.com` and the API on `api.inigoartolozaga.com`, every cross-origin
request needs:

- `Access-Control-Allow-Origin: https://cloth.inigoartolozaga.com` (exact origin, **not** `*` —
  the API carries `Authorization` headers and `*` is invalid for credentialed-style flows)
- `Access-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS`
- `Access-Control-Allow-Headers: Authorization, Content-Type`
- Preflight (`OPTIONS`) handled before the JWT extractor (OPTIONS must not require auth)

Implementation: `axum-cors` (add to `Cargo.toml`), layered **before** the JWT auth middleware on
the router, with a small env-driven origin allowlist (`CORS_ALLOWED_ORIGINS`, comma-separated)
so the NUC `.env` controls it — keeping prod and any future staging distinct. Add unit/integration
tests for the preflight response and for a disallowed origin.

### 1.3 Backend: `/healthz` endpoint

Tiny unauthenticated route: pings `SELECT 1` on the pool, returns `200 {"status":"ok"}`.
Used by the tunnel's health checks and any uptime monitor later. (Also lets the tunnel/proxy
distinguish "container is down" from "nginx is down".)

### 1.4 Backend: Dockerfile

Multi-stage (new file at repo root):

```dockerfile
# --- build ---
FROM rust:1.100-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# --- run ---
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
USER nobody
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/backend"]
```

Notes:
- No `DATABASE_URL`-time query macros (all runtime SQL), so no DB needed at build time —
  CI can build this image as-is.
- `USER nobody`: the binary only needs env + network. (Verify it doesn't write to disk —
  it doesn't today; images go to Supabase Storage.)
- Runtime base needs no OpenSSL dev headers; linking is static via `rustls`.

### 1.5 CI: two new jobs (extend `.github/workflows/ci.yml`)

Both run **after** `backend` + `frontend` pass, on push to `main` only:

```yaml
  image:                      # Phase 0.3 = GHCR
    needs: backend
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - uses: actions/checkout@v5
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ghcr.io/${{ github.repository_owner }}/cloth-web-backend:${{ github.sha }}
          cache-from: type=gha,scope=build
          cache-to: type=gha,mode=max,scope=build

  pages:                     # Frontend → GitHub Pages
    needs: frontend
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    permissions:
      pages: write
      id-token: write        # for OIDC Pages deployment
    environment:
      name: github-pages
      url: ${{ steps.deploy.outputs.page_url }}
    steps:
      - uses: actions/upload-pages-artifact@v3
        with:
          path: frontend/dist          # already built in the frontend job — see 1.5a
      - uses: actions/deploy-pages@v4
        id: deploy
```

**1.5a — artifact handoff:** the `frontend` job must publish `frontend/dist` as a
**Pages-compatible artifact** (not just the human-download artifact). Cleanest: the `pages` job
checks out the repo, runs the same build (typecheck already ran in CI, this is build-only), and
uploads `frontend/dist`. Slightly redundant build, but keeps jobs independent; ~1 min.

Enable Pages in repo settings first: **Settings → Pages → Build and deployment → Source:
GitHub Pages**, then this job does the rest. (Using `actions/deploy-pages` with OIDC avoids
storing a deploy key.)

**New repo secret:** none required for either job — `GITHUB_TOKEN` is automatic. The existing
`VITE_SUPABASE_*` secrets cover the Pages build env.

### 1.6 Compose file for inibaby (new: `infra/compose/`)

```yaml
# infra/compose/docker-compose.yml — copied to inibaby, or inibaby pulls the repo
services:
  backend:
    image: ghcr.io/inigofera/cloth-web-backend:${CLOTHWEB_TAG:-latest}
    container_name: cloth-web-backend
    env_file: .env                    # DATABASE_URL, SUPABASE_*, SETTINGS_ENCRYPTION_KEY,
                                      # CORS_ALLOWED_ORIGINS, DISCORD_*, ...
    environment:
      CORS_ALLOWED_ORIGINS: https://cloth.inigoartolozaga.com,https://inigoartolozaga.com
    ports:
      - "127.0.0.1:3000:3000"         # loopback ONLY; the tunnel connects here
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://127.0.0.1:3000/healthz"]
      interval: 30s
      retries: 3
```

**Migration runner** (Phase 0.4 = ledger table), `infra/compose/migrate.sh`, run before `up`
(either a compose entrypoint wrapper or a CI-on-inibaby cron):

```bash
#!/usr/bin/env bash
# Applies supabase/migrations/*.sql not yet in schema_migrations.
set -euo pipefail
PSQL="docker run --rm --env-file .env postgres:16-alpine psql \"$DATABASE_URL\""
$PSQL -c "CREATE TABLE IF NOT EXISTS schema_migrations (name text PRIMARY KEY, applied_at timestamptz DEFAULT now());"
for f in supabase/migrations/*.sql; do
  name=$(basename "$f" .sql)
  applied=$($PSQL -tAc "SELECT 1 FROM schema_migrations WHERE name='$name'")
  if [ "$applied" = "1" ]; then continue; fi
  echo "applying $name"
  docker run --rm --env-file .env -v "$PWD/supabase/migrations:/migrations" \
    postgres:16-alpine psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "/migrations/$name"
  $PSQL -c "INSERT INTO schema_migrations (name) VALUES ('$name');"
done
```

(Caveat: Supabase Postgres may already have some of these tables from manual setup — the
ledger starts empty, so on first run it will try to re-apply `CREATE TABLE` statements.
Solution: seed the ledger with the already-applied migration names on first run, and/or make
migrations idempotent where cheap. Call this out explicitly during rollout.)

---

## Phase 2 — inibaby (the NUC)

Assumes: Docker installed (`docker` + `docker compose` v2), and a static LAN IP or DHCP
reservation.

1. **Tunnel agent** (Phase 0.1):
   - *Cloudflare Tunnel:* install `cloudflared`, `cloudflared tunnel login`, create tunnel
     `cloth-web`, route `api.inigoartolozaga.com` (and optionally `cloth.` → handled by Pages,
     not the tunnel) → `http://127.0.0.1:3000`. Enable the tunnel as a systemd service.
   - *Tailscale Funnel:* `tailscale up --hostname=inibaby`, then
     `tailscale funnel 3000` (or a Funnel-enabled MagicDNS name + custom domain mapping).
2. **Backend container:** clone the repo (or copy `infra/compose/` + `.env`), fill in
   `infra/compose/.env` with the production values (same set the old server's `.env` had, plus
   `CORS_ALLOWED_ORIGINS`), run `./migrate.sh`, then `docker compose up -d`.
3. **Firewall posture:** no inbound rules added at all (that's the point of the tunnel).
   Verify `ss -tlnp` shows only `127.0.0.1:3000` for the backend.
4. **Deploy procedure** (until we automate it): on inibaby,
   `docker pull ghcr.io/inigofera/cloth-web-backend:<sha> && docker compose up -d`.
   Later: a small `deploy.sh` triggered from CI over SSH (the CD job from the earlier menu).
5. **Auto-updates hygiene:** `docker` auto-updates on the NUC are fine; pin the image tag per
   deploy (the CI also tags `${{ github.sha }}` so you can always roll back to any past
   commit's image).
6. **Backup (cheap insurance):** weekly cron on inibaby:
   `pg_dump "$DATABASE_URL" | gzip > /var/backups/cloth-web-$(date +%F).sql.gz`
   (keep 4 weekly + 1 monthly), stored on a second disk or rsynced off-box. Supabase has its
   own snapshots, but this is your restore path if a bad migration hits prod.

---

## Phase 3 — Domain & DNS (`inigoartolozaga.com`)

1. **DNS records** — added in **Squarespace → inigoartolozaga.com → DNS → Custom records**
   (existing Squarespace-site and Google-verification records stay untouched; the apex stays
   on the website):

   | Type | Name | Value |
   |------|------|-------|
   | `CNAME` | `cloth` | `inigofera.github.io` |
   | `CNAME` | `api` | the tunnel's public hostname, e.g. `abcd1234.cloth-web.<account>.cfargotunnel.com` (printed by `cloudflared tunnel route dns …`) |

   No nameserver migration, no apex changes.

2. **GitHub Pages custom domain:** repo → **Settings → Pages → Custom domains** →
   `cloth.inigoartolozaga.com` → **Enforce HTTPS**. GitHub auto-provisions a Let's Encrypt
   cert (can take ~15 min; the domain shows "not yet verified" until the CNAME resolves).
3. **CI env:** add `VITE_API_BASE_URL=https://api.inigoartolozaga.com` to the workflow's
   `env:` block (plain value, not a secret).
4. **Tunnel health check:** point it at `/healthz` so Cloudflare/Tailscale reports the real
   backend state, not just "port open".

---

## Phase 4 — Cutover & verification

1. Deploy the new backend image to inibaby **while the old server still runs** (both talk to
   the same Supabase; the app is stateless — no local state to migrate).
2. Smoke test the API directly: `curl https://api.inigoartolozaga.com/healthz` and an
   authenticated call (e.g. `GET /tables` with a user JWT).
3. Check CORS preflight:
   `curl -i -X OPTIONS https://api.inigoartolozaga.com/outfits \
     -H "Origin: https://cloth.inigoartolozaga.com" \
     -H "Access-Control-Request-Method: GET"` → expect `204` + the allow-origin header.
4. Wait for the Pages deploy (CI `pages` job) → open `https://cloth.inigoartolozaga.com`,
   log in, run the core flows: wardrobe list, add item + upload, outfit log, try-on,
   settings save.
5. **Decommission the old server's nginx** for this app (keep the box for anything else it
   serves). Update `infra/nginx.conf` or remove it from the repo with a note pointing here.
6. Optional finish: uptime monitor on `/healthz` (Uptime Kuma on inibaby itself, or a free
   external tier).

**Rollback:** point `VITE_API_BASE_URL` back at the old server (rebuild/redeploy Pages from a
previous commit's tag) or flip the tunnel route back. Images are tagged by SHA, so the
backend rolls back with one `docker compose` tag change.

---

## Security notes

- **Never** put `SUPABASE_JWT_SECRET` in the Pages build env — it would be inlined into the
  public JS bundle. It lives only in inibaby's `.env`.
- CORS allowlist is **exact origins** (`cloth.` + apex if used), never `*`.
- The backend stays on `127.0.0.1`; the only internet path is the tunnel. The NUC's public
  exposure is whatever the tunnel edge provides.
- `SETTINGS_ENCRYPTION_KEY` stays on the server (it's already — the key never leaves the
  backend), so moving to inibaby changes nothing for encrypted settings.
- The publishable key + URL are public by design (Supabase's new API key model) — fine in the
  bundle and fine as CI secrets.
- Discord token: only in inibaby's `.env`. The old server's copy should be rotated or removed
  at cutover so it isn't live in two places.

## Effort estimate

| Phase | Work | ~Time |
|-------|------|-------|
| 1.1–1.3 | API base env, CORS + tests, `/healthz` | half a day |
| 1.4–1.5 | Dockerfile, CI image job, Pages job | half a day |
| 2 | inibaby: tunnel, compose, `.env`, first migration run | half a day |
| 3 | DNS + Pages custom domain + cert | 30 min (+ cert wait) |
| 4 | cutover, smoke tests, decommission | 1 hour |

**Suggested order:** Phase 1 → 3 (DNS/CNAME can be staged in parallel) → 2 → 4.
