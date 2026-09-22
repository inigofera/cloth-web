#!/usr/bin/env python3
"""Import a cloth export JSON into the Supabase database.

Usage:
  python3 scripts/import_export.py               # dry run: report + write SQL file
  python3 scripts/import_export.py --execute     # additionally run the SQL via psql
  python3 scripts/import_export.py --user <uuid> # override target user

The generated SQL is a single transaction, idempotent (safe to re-run), and
preserves the export's UUIDs.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import uuid

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXPORT_FILE = os.path.join(REPO_ROOT, "cloth_export_1789763982199.json")
SQL_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "import_export.generated.sql")
DEFAULT_USER = "a64f5034-b30c-4f19-8acc-6812f74929ff"  # inigofera@proton.me

# raw category -> normalized category
CATEGORY_MAP = {
    "base layer": "base-layer",
    "base-layer": "base-layer",
    "footwear": "shoes",
    "shoes": "shoes",
    "outerwear": "outerwear",
    "bottoms": "bottoms",
    "midlayer": "midlayer",
    "accessories": "accessories",
}
# legacy values that were used as categories but are really subcategories
LEGACY_CATEGORY_AS_SUBCATEGORY = {
    "t-shirt": ("base-layer", "t-shirt"),
    "shorts": ("bottoms", "shorts"),
    "jeans": ("bottoms", "jeans"),
}

ORIGIN_VALUES = {
    "bought new": "Bought New",
    "2nd hand": "2nd Hand",
    "gift": "Gift",
    "borrowed": "Borrowed",
    "made myself": "Made Myself",
}
LAUNDRY_IMPACT_VALUES = {
    "nothing": "Nothing",
    "low": "Low",
    "medium": "Medium",
    "high": "High",
}


def load_env():
    env = {}
    with open(os.path.join(REPO_ROOT, ".env")) as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                env[k.strip()] = v.strip()
    return env


def psql_query(db_url, sql):
    result = subprocess.run(
        ["psql", db_url, "-tA", "-F", "\t", "-v", "ON_ERROR_STOP=1", "-c", sql],
        capture_output=True,
        text=True,
        check=True,
    )
    rows = []
    for line in result.stdout.splitlines():
        if line:
            rows.append(line.split("\t"))
    return rows


def sql_val(v):
    if v is None:
        return "NULL"
    if isinstance(v, bool):
        return "TRUE" if v else "FALSE"
    if isinstance(v, (int, float)):
        return repr(v)
    if isinstance(v, str):
        return "'" + v.replace("'", "''") + "'"
    raise TypeError(f"unsupported SQL value type: {type(v)}")


BATCH_SIZE = 200


def batched_insert(lines, table, columns, rows, conflict):
    """Emit INSERT statements in multi-row batches to keep round-trips low."""
    cols = ", ".join(columns)
    for start in range(0, len(rows), BATCH_SIZE):
        chunk = rows[start : start + BATCH_SIZE]
        values = ",\n  ".join("(" + ", ".join(sql_val(v) for v in row) + ")" for row in chunk)
        lines.append(f"INSERT INTO {table} ({cols}) VALUES\n  {values}\nON CONFLICT {conflict} DO NOTHING;")


def clean_str(v):
    if v is None:
        return ""
    return str(v).strip()


def normalize_laundry(v):
    s = clean_str(v)
    if not s:
        return None
    normalized = LAUNDRY_IMPACT_VALUES.get(s.casefold())
    if normalized is None:
        raise ValueError(f"unknown laundry impact {v!r}; expected one of {sorted(LAUNDRY_IMPACT_VALUES.values())}")
    return normalized


def normalize_origin(v):
    s = clean_str(v)
    if not s:
        return None
    normalized = ORIGIN_VALUES.get(s.casefold())
    if normalized is None:
        raise ValueError(f"unknown origin {v!r}; expected one of {sorted(ORIGIN_VALUES.values())}")
    return normalized


UUID_RE = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", re.I)


def map_id(kind, raw_id):
    """DB id columns are uuid; the export mixes real UUIDs with slugs.

    Slugs are mapped to deterministic UUIDv5s so re-runs are stable and
    outfit references (which use the same slugs) keep resolving.
    """
    if UUID_RE.match(raw_id):
        return raw_id
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"cloth-{kind}:{raw_id}"))


def ts_to_timestamptz(v):
    """Export timestamps are naive (UTC) or carry a Z/offset; normalize to explicit UTC."""
    s = clean_str(v)
    if not s:
        return None
    if s.endswith("Z") or re.search(r"[+-]\d{2}:?\d{2}$", s):
        return s
    return s + "Z"


def date_only(v):
    s = clean_str(v)
    if not s:
        return None
    return s.split("T")[0]


def normalize_item(raw):
    item_id = map_id("item", raw["id"])

    cat_raw = clean_str(raw.get("category")).lower()
    sub_forced = None
    if cat_raw in LEGACY_CATEGORY_AS_SUBCATEGORY:
        category, sub_forced = LEGACY_CATEGORY_AS_SUBCATEGORY[cat_raw]
    elif cat_raw in CATEGORY_MAP:
        category = CATEGORY_MAP[cat_raw]
    else:
        raise ValueError(f"item {item_id}: unknown category {raw.get('category')!r}")

    subcategory = sub_forced or (clean_str(raw.get("subcategory")).lower() or None)

    brand = clean_str(raw.get("brand"))
    if brand.lower() in ("", "no brand"):
        brand = None

    color = clean_str(raw.get("color")).lower() or None
    season = clean_str(raw.get("season")).lower() or None

    materials = []
    if raw.get("materials") is not None:
        materials = sorted({m.strip().lower() for m in str(raw["materials"]).split(",") if m.strip()})

    if raw.get("imageData") is not None:
        raise ValueError(f"item {item_id}: unexpected imageData present; image import not supported")

    return {
        "id": item_id,
        "name": clean_str(raw.get("name")) or None,
        "category": category,
        "subcategory": subcategory,
        "brand": brand,
        "color": color,
        "season": season,
        "materials": materials,
        "purchase_price": raw.get("purchasePrice"),
        "owned_since": date_only(raw.get("ownedSince")),
        "origin": normalize_origin(raw.get("origin")),
        "laundry_impact": normalize_laundry(raw.get("laundryImpact")),
        "repairable": raw.get("repairable"),
        "notes": raw.get("notes"),
        "created_at": ts_to_timestamptz(raw.get("createdAt")),
        "updated_at": ts_to_timestamptz(raw.get("updatedAt")),
        "is_active": bool(raw.get("isActive", True)),
        "wear_count": raw.get("wearCount"),
    }


def normalize_outfit(raw, item_ids):
    outfit_id = map_id("outfit", raw["id"])
    if raw.get("imageData") is not None:
        raise ValueError(f"outfit {outfit_id}: unexpected imageData present; image import not supported")

    seen = set()
    links = []
    for ref in raw.get("clothingItemIds", []):
        mapped = map_id("item", ref)
        if mapped not in item_ids:
            raise ValueError(f"outfit {outfit_id}: references unknown item {ref}")
        if mapped not in seen:
            seen.add(mapped)
            links.append(mapped)

    return {
        "id": outfit_id,
        "date": ts_to_timestamptz(raw.get("date")),
        "notes": raw.get("notes"),
        "created_at": ts_to_timestamptz(raw.get("createdAt")),
        "updated_at": ts_to_timestamptz(raw.get("updatedAt")),
        "is_active": bool(raw.get("isActive", True)),
        "item_ids": links,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true", help="run the generated SQL via psql")
    parser.add_argument("--user", default=DEFAULT_USER, help="target user UUID")
    args = parser.parse_args()

    user_id = args.user
    uuid.UUID(user_id)

    env = load_env()
    db_url = env["DATABASE_URL"]

    with open(EXPORT_FILE) as f:
        export = json.load(f)

    raw_items = export["clothingItems"]
    raw_outfits = export["outfits"]

    mapped_item_ids = [map_id("item", i["id"]) for i in raw_items]
    if len(set(mapped_item_ids)) != len(mapped_item_ids):
        sys.exit("duplicate item ids in export")
    mapped_outfit_ids = [map_id("outfit", o["id"]) for o in raw_outfits]
    if len(set(mapped_outfit_ids)) != len(mapped_outfit_ids):
        sys.exit("duplicate outfit ids in export")
    item_id_set = set(mapped_item_ids)

    items = [normalize_item(i) for i in raw_items]
    outfits = [normalize_outfit(o, item_id_set) for o in raw_outfits]
    total_links = sum(len(o["item_ids"]) for o in outfits)
    slug_items = sum(1 for i in raw_items if not UUID_RE.match(i["id"]))
    slug_outfits = sum(1 for o in raw_outfits if not UUID_RE.match(o["id"]))

    # ---- current reference data in the DB ----
    cats = psql_query(db_url, 'SELECT id, name FROM "clothing-categories"')
    subs = psql_query(
        db_url,
        'SELECT s.id, s.name, c.name FROM "clothing-subcategories" s '
        'JOIN "clothing-categories" c ON c.id = s.category_id',
    )
    brands = psql_query(db_url, "SELECT id, name FROM brands")
    colors = psql_query(db_url, "SELECT id FROM colors")
    seasons = psql_query(db_url, "SELECT id FROM seasons")
    materials = psql_query(db_url, "SELECT id FROM materials")

    cat_by_name = {name.lower(): int(cid) for cid, name in cats}
    sub_by_name = {(cat.lower(), name.lower()): int(sid) for sid, name, cat in subs}
    brand_by_name = {name.lower(): bid for bid, name in brands}
    existing_colors = {cid for (cid,) in colors}
    existing_seasons = {sid for (sid,) in seasons}
    existing_materials = {mid for (mid,) in materials}

    # ---- plan reference data ----
    new_cats = sorted({i["category"] for i in items} - {n for n in cat_by_name})
    next_cat_id = max((int(cid) for cid, _ in cats), default=0) + 1
    for name in new_cats:
        cat_by_name[name.lower()] = next_cat_id
        next_cat_id += 1

    new_subs = sorted(
        {(i["category"], i["subcategory"]) for i in items if i["subcategory"]}
        - {(c, s) for (c, s) in sub_by_name}
    )
    next_sub_id = max((int(sid) for sid, _, _ in subs), default=0) + 1
    for cat, name in new_subs:
        sub_by_name[(cat.lower(), name.lower())] = next_sub_id
        next_sub_id += 1

    new_brands = sorted({i["brand"] for i in items if i["brand"]} - {n for n in brand_by_name})
    brand_ids = {}
    for name in new_brands:
        brand_ids[name.lower()] = str(uuid.uuid5(uuid.NAMESPACE_URL, f"cloth-brand:{name}"))
        brand_by_name[name.lower()] = brand_ids[name.lower()]

    new_colors = sorted({i["color"] for i in items if i["color"]} - existing_colors)
    new_seasons = sorted({i["season"] for i in items if i["season"]} - existing_seasons)
    all_materials = sorted({m for i in items for m in i["materials"]})
    new_materials = [m for m in all_materials if m not in existing_materials]

    # ---- emit SQL ----
    lines = []
    ap = lines.append
    ap("-- Generated by scripts/import_export.py from cloth_export_1789763982199.json")
    ap(f"-- Target user: {user_id}")
    ap(f"-- Items: {len(items)}  Outfits: {len(outfits)}  Outfit links: {total_links}")
    ap("BEGIN;")
    ap("")
    ap("-- reference data: categories")
    if new_cats:
        rows = ",\n  ".join(f"({cat_by_name[n.lower()]}, {sql_val(n)})" for n in new_cats)
        ap(f'INSERT INTO "clothing-categories" (id, name) VALUES\n  {rows}\nON CONFLICT (id) DO NOTHING;')
    ap("-- reference data: subcategories")
    if new_subs:
        rows = ",\n  ".join(
            f"({sub_by_name[(c.lower(), s.lower())]}, {sql_val(s)}, {cat_by_name[c.lower()]})" for c, s in new_subs
        )
        ap(
            'INSERT INTO "clothing-subcategories" (id, name, category_id) VALUES\n  '
            + rows
            + "\nON CONFLICT (id) DO NOTHING;"
        )
    ap("-- reference data: brands")
    if new_brands:
        rows = ",\n  ".join(f"({sql_val(brand_ids[n.lower()])}, {sql_val(n)}, {sql_val(user_id)})" for n in new_brands)
        ap(f"INSERT INTO brands (id, name, user_id) VALUES\n  {rows}\nON CONFLICT (name) DO NOTHING;")
    ap("-- reference data: colors")
    if new_colors:
        rows = ",\n  ".join(f"({sql_val(c)}, NULL)" for c in new_colors)
        ap(f"INSERT INTO colors (id, hex_value) VALUES\n  {rows}\nON CONFLICT (id) DO NOTHING;")
    ap("-- reference data: seasons")
    if new_seasons:
        rows = ",\n  ".join(f"({sql_val(s)})" for s in new_seasons)
        ap(f"INSERT INTO seasons (id) VALUES\n  {rows}\nON CONFLICT (id) DO NOTHING;")
    ap("-- reference data: materials")
    if new_materials:
        rows = ",\n  ".join(f"({sql_val(m)}, {sql_val(user_id)})" for m in new_materials)
        ap(f"INSERT INTO materials (id, user_id) VALUES\n  {rows}\nON CONFLICT (id) DO NOTHING;")
    ap("")
    ap("-- clothing items")
    item_cols = [
        "id", "user_id", "name", "category_id", "subcategory_id", "color_id", "brand_id",
        "purchase_price", "owned_since", "origin", "laundry_impact", "repairable", "notes",
        "image_path", "created_at", "updated_at", "is_active", "wear_count",
    ]
    item_rows = [
        (
            i["id"],
            user_id,
            i["name"],
            cat_by_name[i["category"].lower()],
            sub_by_name.get((i["category"].lower(), i["subcategory"].lower())) if i["subcategory"] else None,
            i["color"],
            brand_by_name.get(i["brand"].lower()) if i["brand"] else None,
            i["purchase_price"],
            i["owned_since"],
            i["origin"],
            i["laundry_impact"],
            i["repairable"],
            i["notes"],
            None,
            i["created_at"],
            i["updated_at"],
            i["is_active"],
            i["wear_count"],
        )
        for i in items
    ]
    batched_insert(lines, '"clothing-items"', item_cols, item_rows, "(id)")
    ap("")
    ap("-- outfits")
    batched_insert(
        lines,
        "outfits",
        ["id", "user_id", "date", "notes", "image_path", "created_at", "updated_at", "is_active"],
        [(o["id"], user_id, o["date"], o["notes"], None, o["created_at"], o["updated_at"], o["is_active"]) for o in outfits],
        "(id)",
    )
    ap("")
    ap("-- outfit items")
    batched_insert(
        lines,
        '"outfit-items"',
        ["outfit_id", "clothing_item_id", "user_id"],
        [(o["id"], item_id, user_id) for o in outfits for item_id in o["item_ids"]],
        "(outfit_id, clothing_item_id)",
    )
    ap("")
    ap("-- item materials")
    batched_insert(
        lines,
        '"clothing-item-materials"',
        ["clothing_item_id", "material_id", "user_id"],
        [(i["id"], m, user_id) for i in items for m in i["materials"]],
        "(clothing_item_id, material_id)",
    )
    ap("")
    ap("-- item seasons")
    batched_insert(
        lines,
        '"clothing-item-seasons"',
        ["clothing_item_id", "season_id", "user_id"],
        [(i["id"], i["season"], user_id) for i in items if i["season"]],
        "(clothing_item_id, season_id)",
    )
    ap("")
    ap("-- keep identity sequences ahead of the explicit ids inserted above")
    ap('SELECT setval(\'clothing_categories_id_seq\', (SELECT max(id) FROM "clothing-categories"));')
    ap('SELECT setval(\'"clothing-subcategories_id_seq"\', (SELECT max(id) FROM "clothing-subcategories"));')
    ap("COMMIT;")

    with open(SQL_FILE, "w") as f:
        f.write("\n".join(lines) + "\n")

    # ---- report ----
    from collections import Counter

    print(f"source:   {os.path.basename(EXPORT_FILE)}")
    print(f"target:   user {user_id}")
    print(f"items:    {len(items)} ({slug_items} slug ids -> deterministic UUIDv5)")
    print(f"outfits:  {len(outfits)} ({slug_outfits} slug ids -> deterministic UUIDv5)")
    print(f"links:    {total_links}")
    print()
    print("category mapping (raw -> normalized):")
    raw_counts = Counter(clean_str(r.get("category")).lower() for r in raw_items)
    for raw, count in sorted(raw_counts.items()):
        if raw in LEGACY_CATEGORY_AS_SUBCATEGORY:
            final = f"{LEGACY_CATEGORY_AS_SUBCATEGORY[raw][0]} / {LEGACY_CATEGORY_AS_SUBCATEGORY[raw][1]} (subcategory)"
        else:
            final = CATEGORY_MAP.get(raw, raw)
        print(f"  {raw or '(empty)'} ({count}) -> {final}")
    print()
    print(f"categories:   reuse {len({i['category'] for i in items} - set(new_cats))}, create {len(new_cats)}: {', '.join(new_cats)}")
    print(f"subcategories: reuse {len({(i['category'], i['subcategory']) for i in items if i['subcategory']} - set(new_subs))}, create {len(new_subs)}")
    for c, s in new_subs:
        print(f"  + {c}/{s}")
    print(f"brands:       reuse {len({i['brand'] for i in items if i['brand']} - set(new_brands))}, create {len(new_brands)}: {', '.join(new_brands)}")
    print(f"colors:       reuse {len({i['color'] for i in items if i['color']} - set(new_colors))}, create {len(new_colors)}: {', '.join(new_colors)}")
    print(f"seasons:      reuse {len({i['season'] for i in items if i['season']} - set(new_seasons))}, create {len(new_seasons)}: {', '.join(new_seasons)}")
    print(f"materials:    reuse {len(all_materials) - len(new_materials)}, create {len(new_materials)}: {', '.join(new_materials)}")
    print()
    nulls = Counter()
    for i in items:
        for k in ("name", "subcategory", "brand", "color", "season", "purchase_price", "owned_since", "origin", "laundry_impact", "repairable", "notes", "wear_count"):
            if i[k] is None:
                nulls[k] += 1
    print("item fields left NULL: " + (", ".join(f"{k}={v}" for k, v in sorted(nulls.items())) or "none"))
    print()
    print(f"SQL written to {SQL_FILE} ({os.path.getsize(SQL_FILE)} bytes)")

    if args.execute:
        print("\nexecuting via psql...")
        result = subprocess.run(
            ["psql", db_url, "-v", "ON_ERROR_STOP=1", "-f", SQL_FILE],
            capture_output=True,
            text=True,
        )
        sys.stdout.write(result.stdout)
        sys.stderr.write(result.stderr)
        if result.returncode != 0:
            sys.exit("psql failed")
        print("done.")


if __name__ == "__main__":
    main()
