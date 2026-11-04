ALTER TABLE public."user-settings"
    ADD COLUMN IF NOT EXISTS discord_user_id text UNIQUE,
    ADD COLUMN IF NOT EXISTS discord_link_code text,
    ADD COLUMN IF NOT EXISTS discord_link_expires_at timestamptz;

