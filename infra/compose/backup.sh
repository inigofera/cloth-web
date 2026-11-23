#!/usr/bin/env bash
# Backups for inibaby (plan §2.6): a Postgres dump plus a full export of the
# Supabase Storage bucket — the wardrobe photos have no independent backup
# anywhere else, so this is their restore path.
#
#   ./backup.sh          # database + storage
#   ./backup.sh db       # database only
#   ./backup.sh storage  # storage only
#
# Reads DATABASE_URL, SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY and
# SUPABASE_STORAGE_BUCKET from .env in this directory (or the environment).
# Writes into $BACKUP_DIR (default /var/backups/cloth-web) and keeps the
# newest $KEEP (default 4) of each kind.
#
# The DB dump runs in a throwaway postgres container (no pg_dump on the NUC);
# the storage export needs python3 (stdlib only) — the NUC already has it for
# the local_inference worker.
set -euo pipefail
cd "$(dirname "$0")"

if [ -f .env ]; then
    # shellcheck disable=SC1091
    set -a; . ./.env; set +a
fi
: "${DATABASE_URL:?DATABASE_URL must be set (in .env or the environment)}"

BACKUP_DIR="${BACKUP_DIR:-/var/backups/cloth-web}"
KEEP="${KEEP:-4}"
STAMP="$(date +%F)"
TARGET="${1:-all}"
mkdir -p "$BACKUP_DIR"

if [ "$TARGET" = "all" ] || [ "$TARGET" = "db" ]; then
    OUT="$BACKUP_DIR/db-$STAMP.sql.gz"
    echo "pg_dump → $OUT"
    docker run --rm postgres:16-alpine pg_dump "$DATABASE_URL" | gzip > "$OUT"
fi

if [ "$TARGET" = "all" ] || [ "$TARGET" = "storage" ]; then
    : "${SUPABASE_URL:?SUPABASE_URL must be set}"
    : "${SUPABASE_SERVICE_ROLE_KEY:?SUPABASE_SERVICE_ROLE_KEY must be set}"
    : "${SUPABASE_STORAGE_BUCKET:?SUPABASE_STORAGE_BUCKET must be set}"
    OUT="$BACKUP_DIR/storage-$STAMP.tar.gz"
    echo "storage export → $OUT"
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT
    python3 - "$TMP" <<'PY'
import json
import os
import sys
import urllib.parse
import urllib.request

tmp = sys.argv[1]
base = os.environ["SUPABASE_URL"].rstrip("/")
key = os.environ["SUPABASE_SERVICE_ROLE_KEY"]
bucket = os.environ["SUPABASE_STORAGE_BUCKET"]
limit = 1000

def get(url):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {key}"})
    with urllib.request.urlopen(req) as r:
        return r.read()

# List the whole bucket (the list endpoint caps at 1000 per page).
offset, names = 0, []
while True:
    url = f"{base}/storage/v1/object/list/{urllib.parse.quote(bucket)}?limit={limit}&offset={offset}"
    page = json.loads(get(url))
    names += [o["name"] for o in page]
    if len(page) < limit:
        break
    offset += limit

# Download each object, preserving its path so the tar restores as-is.
for i, name in enumerate(names, 1):
    url = f"{base}/storage/v1/object/{urllib.parse.quote(bucket)}/{urllib.parse.quote(name, safe='/')}"
    dest = os.path.join(tmp, name)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with open(dest, "wb") as f:
        f.write(get(url))
    if i % 100 == 0 or i == len(names):
        print(f"  {i}/{len(names)} objects", flush=True)
PY
    tar -czf "$OUT" -C "$TMP" .
fi

# Retention: keep the newest $KEEP of each kind, oldest-first beyond that.
for kind in db storage; do
    ls -1t "$BACKUP_DIR"/"$kind"-* 2>/dev/null \
        | tail -n +"$((KEEP + 1))" \
        | while read -r old; do
            rm -f "$old"
          done
done
echo "done (keeping $KEEP of each kind in $BACKUP_DIR)"
