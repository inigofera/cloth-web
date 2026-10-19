CREATE TABLE IF NOT EXISTS public."user-settings" (
    user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    encrypted_api_key bytea,
    api_key_nonce bytea,
    reference_image_path text,
    tryon_provider text NOT NULL DEFAULT 'gemini' CHECK (tryon_provider IN ('gemini', 'qwen_local')),
    local_qwen_url text NOT NULL DEFAULT 'http://127.0.0.1:8001',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE public."user-settings" ENABLE ROW LEVEL SECURITY;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies 
        WHERE schemaname = 'public' 
          AND tablename = 'user-settings' 
          AND policyname = 'Users can manage own settings'
    ) THEN
        CREATE POLICY "Users can manage own settings"
            ON public."user-settings"
            FOR ALL
            USING (auth.uid() = user_id)
            WITH CHECK (auth.uid() = user_id);
    END IF;
END $$;

