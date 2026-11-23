# inibaby deployment runbook (M6)

Machine-side half of [docs/home-infra-plan.md](../../docs/home-infra-plan.md):
Cloudflare Tunnel, Squarespace CNAMEs, the NUC `.env`, first migration run,
backups, uptime monitoring, and the cutover. The repo-side pieces (CORS,
`VITE_API_BASE_URL`, Dockerfile, CI image/pages jobs, this directory) ship in
the M6 commit; everything below runs on the NUC or in a web console.

**Files in this directory**

| File | Purpose |
|------|---------|
| `docker-compose.yml` | the backend service (loopback-only port, healthcheck on `/healthz`) |
| `.env.example` | the production env template — copy to `.env` and fill in |
| `migrate.sh` | applies unapplied `supabase/migrations/*.sql` via the `schema_migrations` ledger |
| `seeded-migrations.txt` | what the live DB already has, seeded into the ledger on first run |
| `deploy.sh` | `docker pull` + recreate for a given image tag (or SHA rollback) |
| `backup.sh` | Postgres dump + full Storage-bucket export, with retention |

Suggested order (mirrors the plan): **1–3 in parallel, then 4–9, then cutover.**

---

## 1. NUC prerequisites (one-time)

- Docker + compose v2 (`docker compose version`), python3 (for `backup.sh` and
  the future inference worker).
- Static LAN IP / DHCP reservation for inibaby.
- The repo cloned on the NUC (this directory then lives at
  `<repo>/infra/compose`).

## 2. Cloudflare Tunnel (plan §2.1)

```sh
cloudflared tunnel login          # browser dance; one time per account
cloudflared tunnel create cloth-web
# note the public hostname printed, e.g.
#   1234abcd.cloth-web.<account>.cfargotunnel.com   → step 3's CNAME value
cloudflared tunnel run cloth-web  # test in a terminal
```

With it running, confirm locally:
`curl http://127.0.0.1:3000/healthz` (once the container is up, step 7).
Then register it as a systemd service and stop the manual run:

```sh
sudo cloudflared service install cloth-web
sudo systemctl enable --now cloudflared
```

No inbound firewall rules are added — that's the point. The NUC's public
exposure is whatever the tunnel edge provides.

## 3. DNS: two CNAMEs in Squarespace (plan §3.1)

**inigoartolozaga.com → DNS → Custom records** (existing Squarespace-site and
Google-verification records stay untouched; the apex stays on the website):

| Type | Name | Value |
|------|------|-------|
| `CNAME` | `cloth` | `inigofera.github.io` |
| `CNAME` | `api` | the tunnel's public hostname from step 2 |

Propagation: minutes to a few hours.

## 4. GitHub Pages (plan §3.2)

1. Repo **Settings → Pages → Build and deployment → Source: `GitHub Actions`**
   (required once, before the CI `pages` job can deploy).
2. **Settings → Pages → Custom domains → `cloth.inigoartolozaga.com`** →
   **Enforce HTTPS**. GitHub auto-provisions the Let's Encrypt cert; the
   domain shows "not yet verified" until the CNAME from step 3 resolves
   (~15 min after DNS).
3. The CI `pages` job creates/uses the `github-pages` deployment environment
   automatically — no repo secret needed (`GITHUB_TOKEN` OIDC).

## 5. The NUC `.env` (plan §2.2)

```sh
cd <repo>/infra/compose
cp .env.example .env
openssl rand -hex 32    # candidate SETTINGS_ENCRYPTION_KEY
```

Fill in every `Required` value from the old server's `.env` and the Supabase
dashboard. Notes:

- **Reuse the old server's `SETTINGS_ENCRYPTION_KEY`** if users have stored
  per-user settings (Gemini keys are encrypted under it); a new key forces
  re-entry.
- `CORS_ALLOWED_ORIGINS` must list the exact origin the Pages site is served
  from: `https://cloth.inigoartolozaga.com`.
- Discord: rotate the bot token (Discord Developer Portal → your app → Bot →
  **Reset Token**) and put only the new token here — see step 10.
- Never set `SUPABASE_JWT_SECRET` or the service key in any frontend/CI env —
  they live only in this file.

## 6. First migration run (plan §1.6)

```sh
./migrate.sh
```

First run: creates `schema_migrations` and seeds it from
`seeded-migrations.txt` (the live DB has been serving the M5 codebase, so all
eight migrations should already be applied — the run is a no-op that puts the
ledger in place). If it errors on a migration that the DB turns out to already
have, that migration belongs in `seeded-migrations.txt`: add it, re-run. The
runner fails loudly (`ON_ERROR_STOP`), never half-applies.

## 7. Start the container

```sh
docker compose up -d
docker compose ps          # health: (healthy) once /healthz answers
ss -tlnp | grep 3000       # expect ONLY 127.0.0.1:3000
```

## 8. Smoke test through the tunnel (plan §4.2–4.3)

```sh
curl https://api.inigoartolozaga.com/healthz
# → 200 {"status":"ok"}

# CORS preflight, exactly as the browser will send it:
curl -i -X OPTIONS https://api.inigoartolozaga.com/outfits \
  -H "Origin: https://cloth.inigoartolozaga.com" \
  -H "Access-Control-Request-Method: GET"
# → 204 + Access-Control-Allow-Origin: https://cloth.inigoartolozaga.com
```

## 9. Wait for the Pages deploy, then verify the SPA (plan §4.4)

CI's `pages` job deploys on the main push that carried M6. Open
`https://cloth.inigoartolozaga.com`, log in, and run the core flows: wardrobe
list, add item + upload, outfit log, try-on, settings save. A cross-origin
`X-Total-Count` read (capped-list UI) confirms `Expose-Headers` works.

## 10. Cutover: Discord token + decommission the old server

1. **Discord token rotation:** with the NUC live, reset the bot token in the
   Discord Developer Portal, update the NUC `.env`, and
   `docker compose up -d` to reload it. Then **remove/unset `DISCORD_BOT_TOKEN`
   on the old server** so the token isn't live in two places.
2. **Decommission the old server's nginx for this app** (plan §4.5): drop the
   `cloth-web` server block (keep the box for anything else it serves).
   `infra/nginx.conf` in the repo stays as the historical record until then.
3. Confirm the old origin no longer answers `/api` for this app.

## 11. Backups (plan §2.6)

```sh
./backup.sh            # db + storage, into /var/backups/cloth-web
```

Cron (weekly + a monthly extra; the script keeps the newest 4 of each):

```cron
# /etc/cron.d/cloth-web-backups
0 3 * * 0 root /path/to/infra/compose/backup.sh all >> /var/log/cloth-web-backup.log 2>&1
```

Supabase has its own snapshots, but this is **your** restore path if a bad
migration hits prod, and the only independent copy of the photos. Strongly
prefer a second disk or an off-box rsync for `$BACKUP_DIR`.

## 12. Uptime monitor (plan §4.6)

Point any monitor at `https://api.inigoartolozaga.com/healthz` — it
distinguishes "backend down" from "everything up" without auth. Options:
Uptime Kuma in a second compose service on the NUC, or a free external tier
(Better Stack / UptimeRobot).

## 13. Deploys and rollbacks (plan §2.4/§4)

```sh
./deploy.sh            # latest (retagged on every main push)
./deploy.sh <sha>      # pin or roll back to any past commit's image
```

Images are tagged per-SHA in GHCR, so rollback is one tag change. Frontend
rollback: redeploy Pages from a previous commit (the `pages` job runs on
push-to-main; push a revert, or use Pages' "Restore" if available).
