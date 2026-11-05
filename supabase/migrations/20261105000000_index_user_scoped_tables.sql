-- The generic table endpoints now answer two questions with columns rather than with a walk:
-- they filter user-scoped tables on user_id, and they page them newest first. A foreign key
-- towards auth.users buys no index of its own, so each of those reads walked the whole table.
-- Index the columns that answer, generically: every public table carrying user_id gets the
-- lookup index, and one that also carries created_at gets the composite index its paging
-- reads through. Written this way so tables that exist now and tables a later migration adds
-- are both covered, and so re-running it costs nothing.

BEGIN;

DO $$
DECLARE
  candidate text;
  index_name text;
BEGIN
  FOR candidate IN
    SELECT candidate_table.table_name
    FROM information_schema.tables AS candidate_table
    WHERE candidate_table.table_schema = 'public'
      AND candidate_table.table_name NOT IN ('user-settings')
      AND EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = candidate_table.table_name
          AND column_name = 'user_id'
      )
  LOOP
    index_name := 'idx_' || candidate || '_user_id';

    IF NOT EXISTS (
      SELECT 1
      FROM information_schema.indices
      WHERE table_schema = 'public'
        AND name = index_name
    ) THEN
      EXECUTE format('CREATE INDEX %1I ON public.%1I (user_id)', candidate);
    END IF;

    IF EXISTS (
      SELECT 1
      FROM information_schema.columns
      WHERE table_schema = 'public'
        AND table_name = candidate
        AND column_name = 'created_at'
    ) THEN
      index_name := 'idx_' || candidate || '_user_created';

      IF NOT EXISTS (
        SELECT 1
        FROM information_schema.indices
        WHERE table_schema = 'public'
          AND name = index_name
      ) THEN
        EXECUTE format('CREATE INDEX %1I ON public.%1I (user_id, created_at DESC NULLS LAST)', candidate);
      END IF;
    END IF;
  END LOOP;
END $$;

COMMIT;
