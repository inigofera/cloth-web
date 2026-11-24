#!/usr/bin/env bash
# Applies supabase/migrations/*.sql not yet recorded in the `schema_migrations`
# ledger table (plan §0.4/§1.6). Runs on inibaby against the live Supabase
# database via a throwaway postgres container — no psql or Supabase CLI is
# needed on the NUC itself.
#
#   ./migrate.sh            # read DATABASE_URL from .env in this directory
#
# First run only: the ledger table does not exist yet, so it is created and
# seeded from seeded-migrations.txt — the migrations the live database
# already has (it has been serving the app since before the ledger existed).
# If you believe a listed migration was NOT actually applied, remove its line
# from the seed file and re-run: the runner will apply it, and fails loudly
# (ON_ERROR_STOP) if the database turns out to already have it.
set -euo pipefail
cd "$(dirname "$0")"

if [ ! -f .env ]; then
    echo "error: .env not found in $(pwd) (copy .env.example and fill it in)" >&2
    exit 1
fi
# shellcheck disable=SC1091
set -a; . ./.env; set +a
: "${DATABASE_URL:?DATABASE_URL must be set in .env}"

PSQL() {
    docker run --rm postgres:16-alpine psql "$DATABASE_URL" "$@"
}

REPO_ROOT="$(cd ../.. && pwd)"
MIGRATIONS="$REPO_ROOT/supabase/migrations"
SEED_FILE="$REPO_ROOT/infra/compose/seeded-migrations.txt"

if [ "$(PSQL -tAc "SELECT to_regclass('public.schema_migrations') IS NOT NULL")" != "t" ]; then
    echo "schema_migrations does not exist: creating and seeding from $(basename "$SEED_FILE")"
    PSQL -c "CREATE TABLE public.schema_migrations (name text PRIMARY KEY, applied_at timestamptz DEFAULT now());"
    while IFS= read -r line; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        PSQL -c "INSERT INTO public.schema_migrations (name) VALUES ('$line');"
        echo "seeded $line"
    done < "$SEED_FILE"
fi

applied=0
for f in "$MIGRATIONS"/*.sql; do
    name="$(basename "$f" .sql)"
    if [ "$(PSQL -tAc "SELECT 1 FROM public.schema_migrations WHERE name='$name'")" = "1" ]; then
        continue
    fi
    echo "applying $name"
    # Migration files are checked-in repo content, not request data: the
    # filename-derived name is safe to inline in the SQL below.
    docker run --rm -v "$MIGRATIONS:/migrations:ro" \
        postgres:16-alpine psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "/migrations/$name"
    PSQL -c "INSERT INTO public.schema_migrations (name) VALUES ('$name');"
    applied=$((applied + 1))
done

echo "done: $applied new migration(s) applied"
