-- Constrain clothing item Origin and Laundry Impact to the values exposed by the UI.
-- Both fields remain nullable so existing unset values stay unset.

BEGIN;

UPDATE public."clothing-items"
SET laundry_impact = CASE lower(trim(laundry_impact))
  WHEN 'nothing' THEN 'Nothing'
  WHEN 'low' THEN 'Low'
  WHEN 'medium' THEN 'Medium'
  WHEN 'high' THEN 'High'
  ELSE laundry_impact
END
WHERE laundry_impact IS NOT NULL;

UPDATE public."clothing-items"
SET origin = NULL
WHERE origin IS NOT NULL AND trim(origin) = '';

DO $$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM public."clothing-items"
    WHERE origin IS NOT NULL
      AND origin NOT IN ('Bought New', '2nd Hand', 'Gift', 'Borrowed', 'Made Myself')
  ) THEN
    RAISE EXCEPTION 'Cannot add origin constraint: unknown existing clothing item origin values exist';
  END IF;

  IF EXISTS (
    SELECT 1
    FROM public."clothing-items"
    WHERE laundry_impact IS NOT NULL
      AND laundry_impact NOT IN ('Nothing', 'Low', 'Medium', 'High')
  ) THEN
    RAISE EXCEPTION 'Cannot add laundry impact constraint: unknown existing clothing item values exist';
  END IF;
END $$;

ALTER TABLE public."clothing-items"
  DROP CONSTRAINT IF EXISTS clothing_items_origin_check,
  DROP CONSTRAINT IF EXISTS clothing_items_laundry_impact_check;

ALTER TABLE public."clothing-items"
  ADD CONSTRAINT clothing_items_origin_check
    CHECK (origin IS NULL OR origin IN ('Bought New', '2nd Hand', 'Gift', 'Borrowed', 'Made Myself')),
  ADD CONSTRAINT clothing_items_laundry_impact_check
    CHECK (laundry_impact IS NULL OR laundry_impact IN ('Nothing', 'Low', 'Medium', 'High'));

COMMIT;
