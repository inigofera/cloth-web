-- Base schema for Cloth Web (V1.0 report §5.1).
--
-- The seven core tables below predate the migration ledger: they were created
-- manually in the live Supabase project, and every later migration assumes
-- they exist (20260922 constrains columns on `clothing-items`, 20261018
-- references `auth.users`, ...). This file makes a fresh project
-- bootstrappable from the repo alone, and is idempotent so it re-runs
-- cleanly against the live one.
--
-- Shapes are reconstructed from code usage: `frontend/src/api/types.ts`,
-- the insert/update statements in `src/routes.rs` and `src/discord.rs`, and
-- the constraint expectations in the 20260922 migration.

-- colors: shared reference data keyed by a lowercased text slug.
CREATE TABLE IF NOT EXISTS public.colors (
    id text PRIMARY KEY,
    hex_value text
);

-- brands: shared-or-owned (discord.rs queries `user_id = $1 OR user_id IS NULL`).
CREATE TABLE IF NOT EXISTS public.brands (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid,
    name text NOT NULL
);

-- Shared integer-keyed reference tables for the generic API's read path.
CREATE TABLE IF NOT EXISTS public."clothing-categories" (
    id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name text NOT NULL
);

CREATE TABLE IF NOT EXISTS public."clothing-subcategories" (
    id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name text NOT NULL,
    category_id integer REFERENCES public."clothing-categories"(id)
);

-- User-scoped garment rows. `json_populate_record` inserts name columns from
-- the client body; the id, user_id and timestamps come from the server.
CREATE TABLE IF NOT EXISTS public."clothing-items" (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    name text NOT NULL,
    brand_id uuid REFERENCES public.brands(id),
    category_id integer REFERENCES public."clothing-categories"(id),
    subcategory_id integer REFERENCES public."clothing-subcategories"(id),
    color_id text REFERENCES public.colors(id),
    purchase_price numeric,
    owned_since date,
    origin text,
    laundry_impact text,
    repairable boolean,
    notes text,
    image_path text,
    -- Nullable on purpose: the generic insert builds rows with
    -- `json_populate_record`, which does not apply column defaults, so a body
    -- that omits `is_active` must not violate a NOT NULL constraint.
    is_active boolean DEFAULT true,
    wear_count integer,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- User-scoped outfit logs; `date` is bound `$2::timestamptz` and rendered
-- `to_char(o.date, 'YYYY-MM-DD')` by the list endpoint. The id is generated
-- by the database: `create_outfit` never supplies one.
CREATE TABLE IF NOT EXISTS public.outfits (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    date timestamptz NOT NULL,
    notes text,
    image_path text,
    is_active boolean DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Outfit <-> garment join. The dedupe logic in create_outfit and the wear
-- count updates assume the pair is unique.
CREATE TABLE IF NOT EXISTS public."outfit-items" (
    outfit_id uuid NOT NULL REFERENCES public.outfits(id) ON DELETE CASCADE,
    clothing_item_id uuid NOT NULL REFERENCES public."clothing-items"(id) ON DELETE CASCADE,
    PRIMARY KEY (outfit_id, clothing_item_id)
);

-- RLS on every user-scoped table (defense in depth: the backend connects
-- directly and the frontend never queries Postgres, but the Supabase client
-- role must not be able to read another user's wardrobe).
ALTER TABLE public."clothing-items" ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.outfits ENABLE ROW LEVEL SECURITY;
ALTER TABLE public."outfit-items" ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.brands ENABLE ROW LEVEL SECURITY;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'clothing-items' AND policyname = 'Users manage own items') THEN
        CREATE POLICY "Users manage own items" ON public."clothing-items"
            FOR ALL USING (auth.uid() = user_id) WITH CHECK (auth.uid() = user_id);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'outfits' AND policyname = 'Users manage own outfits') THEN
        CREATE POLICY "Users manage own outfits" ON public.outfits
            FOR ALL USING (auth.uid() = user_id) WITH CHECK (auth.uid() = user_id);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'outfit-items' AND policyname = 'Users manage own outfit links') THEN
        CREATE POLICY "Users manage own outfit links" ON public."outfit-items"
            FOR ALL
            USING (EXISTS (SELECT 1 FROM public.outfits o WHERE o.id = outfit_id AND o.user_id = auth.uid()))
            WITH CHECK (EXISTS (SELECT 1 FROM public.outfits o WHERE o.id = outfit_id AND o.user_id = auth.uid()));
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'brands' AND policyname = 'Brands are shared or own') THEN
        CREATE POLICY "Brands are shared or own" ON public.brands
            FOR ALL USING (user_id IS NULL OR auth.uid() = user_id) WITH CHECK (user_id IS NULL OR auth.uid() = user_id);
    END IF;
END $$;
