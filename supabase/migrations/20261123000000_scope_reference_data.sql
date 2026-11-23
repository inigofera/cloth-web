-- Scope the shared reference tables per user (M5, the §9.5 multi-user decision).
--
-- `brands` already uses the shared-or-owned shape: `user_id` nullable, and the
-- RLS policy `user_id IS NULL OR auth.uid() = user_id`. Colors, categories and
-- subcategories join the same pattern, so each user defines their own
-- reference data and sees their own rows plus the legacy shared ones. New rows
-- always carry the creating user's id; `user_id IS NULL` remains readable by
-- everyone (existing shared data) but is managed only through the dedicated
-- endpoints in src/routes.rs.

ALTER TABLE public.colors
    ADD COLUMN IF NOT EXISTS user_id uuid REFERENCES auth.users(id) ON DELETE CASCADE;
ALTER TABLE public."clothing-categories"
    ADD COLUMN IF NOT EXISTS user_id uuid REFERENCES auth.users(id) ON DELETE CASCADE;
ALTER TABLE public."clothing-subcategories"
    ADD COLUMN IF NOT EXISTS user_id uuid REFERENCES auth.users(id) ON DELETE CASCADE;

-- colors: the slug is no longer unique on its own - one slug can exist per
-- user. `user_id` stays nullable (legacy shared rows). A primary key
-- (user_id, id) would make user_id NOT NULL by definition, so the pair gets
-- a plain UNIQUE constraint instead: NULLs are distinct in it, which keeps
-- the shared rows legal and still backs `ON CONFLICT (user_id, id)`.
--
-- The foreign key from "clothing-items".color_id to colors(id) cannot survive
-- this change: an item's color reference is scoped by the item's own
-- user_id, and a composite (user_id, color_id) -> (user_id, id) FK would
-- forbid referencing the legacy shared rows. The key is dropped and integrity
-- is carried by the app instead - items only ever receive slugs from the
-- owner's visible list, and the rename/delete endpoints re-point or null the
-- references in a transaction.
ALTER TABLE public."clothing-items" DROP CONSTRAINT IF EXISTS "clothing-items_color_id_fkey";
ALTER TABLE public.colors DROP CONSTRAINT IF EXISTS colors_pkey;
ALTER TABLE public.colors ADD CONSTRAINT colors_user_id_id_key UNIQUE (user_id, id);

-- One category name per user. NULL again means "legacy shared", where the
-- constraint cannot dedupe - fine, because shared rows are created only by
-- hand and the dedicated endpoint upserts on (user_id, name) for owned rows.
ALTER TABLE public."clothing-categories"
    ADD CONSTRAINT categories_user_name_key UNIQUE (user_id, name);

CREATE INDEX IF NOT EXISTS colors_user_id_idx ON public.colors (user_id);
CREATE INDEX IF NOT EXISTS clothing_categories_user_id_idx ON public."clothing-categories" (user_id);
CREATE INDEX IF NOT EXISTS clothing_subcategories_user_id_idx ON public."clothing-subcategories" (user_id);

-- RLS, same policy shape as brands: shared rows are visible to everyone,
-- owned rows only to their owner. Defense in depth: the backend connects
-- directly, but the Supabase client role must not see other users' data.
ALTER TABLE public.colors ENABLE ROW LEVEL SECURITY;
ALTER TABLE public."clothing-categories" ENABLE ROW LEVEL SECURITY;
ALTER TABLE public."clothing-subcategories" ENABLE ROW LEVEL SECURITY;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'colors' AND policyname = 'Colors are shared or own') THEN
        CREATE POLICY "Colors are shared or own" ON public.colors
            FOR ALL USING (user_id IS NULL OR auth.uid() = user_id) WITH CHECK (user_id IS NULL OR auth.uid() = user_id);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'clothing-categories' AND policyname = 'Categories are shared or own') THEN
        CREATE POLICY "Categories are shared or own" ON public."clothing-categories"
            FOR ALL USING (user_id IS NULL OR auth.uid() = user_id) WITH CHECK (user_id IS NULL OR auth.uid() = user_id);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE schemaname = 'public' AND tablename = 'clothing-subcategories' AND policyname = 'Subcategories are shared or own') THEN
        CREATE POLICY "Subcategories are shared or own" ON public."clothing-subcategories"
            FOR ALL USING (user_id IS NULL OR auth.uid() = user_id) WITH CHECK (user_id IS NULL OR auth.uid() = user_id);
    END IF;
END $$;

-- Adopt the legacy shared rows: where exactly one user references a row, make
-- that user its owner. Without this, pre-existing data (for example the
-- "Navay" typo this feature exists to fix) would be frozen as read-only
-- shared data. Rows referenced by several users, or by none, stay shared.
-- (DISTINCT is not allowed inside window functions, so the "exactly one
-- referencing user" test is a plain GROUP BY join: when n = 1 every row of
-- refs carries the same user_id, so the pair below is unique per row.)
WITH color_refs AS (
    SELECT ci.color_id, ci.user_id
    FROM public."clothing-items" ci
    WHERE ci.color_id IS NOT NULL
),
color_owners AS (
    SELECT color_id, count(DISTINCT user_id) AS n
    FROM color_refs
    GROUP BY color_id
)
UPDATE public.colors c
SET user_id = r.user_id
FROM (
    SELECT color_refs.color_id, color_refs.user_id
    FROM color_refs
    JOIN color_owners o ON o.color_id = color_refs.color_id AND o.n = 1
) r
WHERE c.id = r.color_id AND c.user_id IS NULL;

WITH category_refs AS (
    SELECT ci.category_id, ci.user_id
    FROM public."clothing-items" ci
    WHERE ci.category_id IS NOT NULL
),
category_owners AS (
    SELECT category_id, count(DISTINCT user_id) AS n
    FROM category_refs
    GROUP BY category_id
)
UPDATE public."clothing-categories" cc
SET user_id = r.user_id
FROM (
    SELECT category_refs.category_id, category_refs.user_id
    FROM category_refs
    JOIN category_owners o ON o.category_id = category_refs.category_id AND o.n = 1
) r
WHERE cc.id = r.category_id AND cc.user_id IS NULL;

WITH subcategory_refs AS (
    SELECT ci.subcategory_id, ci.user_id
    FROM public."clothing-items" ci
    WHERE ci.subcategory_id IS NOT NULL
),
subcategory_owners AS (
    SELECT subcategory_id, count(DISTINCT user_id) AS n
    FROM subcategory_refs
    GROUP BY subcategory_id
)
UPDATE public."clothing-subcategories" cs
SET user_id = r.user_id
FROM (
    SELECT subcategory_refs.subcategory_id, subcategory_refs.user_id
    FROM subcategory_refs
    JOIN subcategory_owners o ON o.subcategory_id = subcategory_refs.subcategory_id AND o.n = 1
) r
WHERE cs.id = r.subcategory_id AND cs.user_id IS NULL;
