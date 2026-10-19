ALTER TABLE public."user-settings"
    ADD COLUMN IF NOT EXISTS tryon_provider text NOT NULL DEFAULT 'gemini',
    ADD COLUMN IF NOT EXISTS local_qwen_url text NOT NULL DEFAULT 'http://127.0.0.1:8001';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'user_settings_tryon_provider_check'
    ) THEN
        ALTER TABLE public."user-settings"
            ADD CONSTRAINT user_settings_tryon_provider_check
                CHECK (tryon_provider IN ('gemini', 'qwen_local'));
    END IF;
END $$;

