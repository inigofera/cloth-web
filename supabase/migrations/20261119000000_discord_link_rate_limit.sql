-- Rate-limiting state for the Discord `!link <code>` flow.
--
-- Link codes are 6 digits with a 15-minute window; without attempt limits a
-- Discord-side attacker could brute-force them and each wrong guess gets a
-- confirming reply (an oracle). This table counts failed attempts per Discord
-- user and locks the flow out for one hour after five failures.
--
-- The table is internal bookkeeping: it is blocked from the generic table API
-- (src/routes.rs) and carries no RLS policy, so neither the app API nor
-- authenticated database users can read or write it directly.

CREATE TABLE IF NOT EXISTS public."discord-link-attempts" (
    discord_user_id text PRIMARY KEY,
    fail_count integer NOT NULL DEFAULT 0,
    locked_until timestamptz
);

ALTER TABLE public."discord-link-attempts" ENABLE ROW LEVEL SECURITY;
