# Virtual Try-On (Nano Banana) Implementation Specification

> **Target Repository:** `cloth-web`[cite: 1]
> **Architecture Choice:** Option 2 (Encrypted Database Storage + Dedicated Axum Orchestration)
> **Objective:** Implement a Bring-Your-Own-Key (BYOK) virtual outfit try-on feature using the Gemini API (`gemini-2.5-flash-image` / Nano Banana). Users configure their API key and a full-body reference photo in a new Settings view. Users can then generate composite outfit previews in the Outfits view and save them to `outfits.image_path`[cite: 1].

---

## 1. Architectural Decisions & Constraints

1. **Multi-Device Persistence (Encrypted DB):** User settings (`api_key` and `reference_image_path`) are stored in a dedicated PostgreSQL table (`public."user-settings"`). The API key is encrypted at rest using **AES-256-GCM** on the Rust backend before database insertion.
2. **Generic CRUD Isolation:** The `cloth-web` backend exposes `GET/POST/PATCH/DELETE /db/:table`[cite: 1]. You **must** explicitly block `"user-settings"` in these generic handlers so that encrypted ciphertexts and nonces are never leaked or overwritten via the generic table API.
3. **Native Multi-Image Payload:** Do not stitch or juxtapose images on a canvas. The Axum backend will download the reference image and all selected garment images concurrently from Supabase Storage[cite: 1], encode them as Base64, and send them as discrete `inlineData` parts in a single multimodal prompt to the Gemini API.
4. **Metadata-Driven Prompting:** The backend will query the database for each selected garment's `name`, `category_name`, `subcategory_name`, `color_id`, and `notes`[cite: 1], injecting this structured context into the prompt to prevent garment ambiguity.
5. **Output Persistence:** Generated images returned by the Gemini API are passed through the existing `upload.rs` sanitization/normalization pipeline (Lanczos3 resize to max 1600px, progressive JPEG Q80)[cite: 1], stored under `users/{user_id}/generated-{uuid}.jpg`[cite: 1], and return the storage key so the frontend can attach it to `outfits.image_path`[cite: 1].

---

## 2. Database Migration

Create a new migration file: `supabase/migrations/20261018000000_create_user_settings.sql`[cite: 1].

### SQL Specification
```sql
CREATE TABLE IF NOT EXISTS public."user-settings" (
    user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    encrypted_api_key bytea,
    api_key_nonce bytea,
    reference_image_path text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE public."user-settings" ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage own settings"
    ON public."user-settings"
    FOR ALL
    USING (auth.uid() = user_id)
    WITH CHECK (auth.uid() = user_id);
```

---

## 3. Environment & Dependencies

### 3.1 Environment Variables (`.env`)[cite: 1]
Add the following required variable to the backend `.env` file[cite: 1]:
```env
# 64-character hex string (32 bytes) used for AES-256-GCM encryption of user API keys.
# Generate via: openssl rand -hex 32
SETTINGS_ENCRYPTION_KEY=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
```

### 3.2 Rust Dependencies (`Cargo.toml`)[cite: 1]
Ensure the following crates are present in `Cargo.toml`[cite: 1]:
```toml
aes-gcm = "0.10"
rand = "0.8"
hex = "0.4"
base64 = "0.22"
```

---

## 4. Backend Implementation (Rust / Axum)[cite: 1]

### 4.1 Update `AppState` (`src/main.rs`)[cite: 1]
1. Read `SETTINGS_ENCRYPTION_KEY` from the environment on startup.
2. Decode the hex string into a `[u8; 32]` byte array. Panic on startup with a clear error message if the key is missing or not exactly 32 bytes (64 hex chars).
3. Add `encryption_key: [u8; 32]` to `AppState`[cite: 1].
4. Register the new routes in the Axum `Router`[cite: 1]:
   - `.route("/settings", get(routes::get_user_settings).put(routes::update_user_settings))`
   - `.route("/outfits/generate-preview", post(routes::generate_outfit_preview))`

### 4.2 Create Cryptography Module (`src/crypto.rs`)
Create `src/crypto.rs` and declare `mod crypto;` in `src/main.rs`[cite: 1].
Implement two pure functions using `aes_gcm::{Aes256Gcm, KeyInit, Nonce}` and `aes_gcm::aead::Aead`:
1. `pub fn encrypt_api_key(plaintext: &str, key: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>), String>`:
   - Generate a random 12-byte nonce (`[u8; 12]`) using `rand::RngCore::fill_bytes`.
   - Encrypt `plaintext.as_bytes()` using `Aes256Gcm::new(key.into())`.
   - Return `(ciphertext, nonce.to_vec())`.
2. `pub fn decrypt_api_key(ciphertext: &[u8], nonce: &[u8], key: &[u8; 32]) -> Result<String, String>`:
   - Verify `nonce.len() == 12`.
   - Decrypt `ciphertext` and convert the resulting bytes to a UTF-8 `String`.

### 4.3 Reuse Image Processing Pipeline (`src/upload.rs`)[cite: 1]
Ensure the image normalization function in `src/upload.rs` (which decodes images, downscales to max 1600px with `Lanczos3`, and encodes to progressive JPEG at quality 80) is exposed as a public helper function[cite: 1]:
```rust
pub fn process_and_encode_image(raw_bytes: &[u8]) -> Result<(Vec<u8>, &'static str), (StatusCode, String)>
```
This allows both `POST /upload` and the Gemini output handler to use the exact same sanitization and compression logic[cite: 1].

### 4.4 Block Generic CRUD Access to Settings (`src/routes.rs`)[cite: 1]
1. In `GET /tables` (schema introspection): Filter out `"user-settings"` so the encrypted columns do not appear in the Tables explorer[cite: 1].
2. In `GET /db/:table`, `POST /db/:table`, `PATCH /db/:table/:id`, and `DELETE /db/:table/:id`[cite: 1]:
   Add an explicit guard at the top of each handler:
   ```rust
   if table == "user-settings" {
       return Err((StatusCode::FORBIDDEN, "Access user settings via /settings endpoint".to_string()));
   }
   ```

### 4.5 Implement Settings Endpoints (`src/routes.rs`)[cite: 1]

#### `GET /settings` (`pub async fn get_user_settings`)
- **Extractor:** `AuthUser(user_id)`[cite: 1], `State(state)`[cite: 1].
- **Behavior:** Query `public."user-settings"` for `user_id`[cite: 1].
- **Response Payload (`200 OK`):**
  ```json
  {
    "has_api_key": true,
    "api_key_hint": "...a1b2",
    "reference_image_path": "users/<user_id>/uuid-reference.jpg"
  }
  ```
  *Note:* Never return the full decrypted API key to the client. If `encrypted_api_key` is present, decrypt it in memory, extract the last 4 characters to populate `api_key_hint`, and immediately drop the plaintext string. If no row exists for the user, return `has_api_key: false`, `api_key_hint: null`, `reference_image_path: null`.

#### `PUT /settings` (`pub async fn update_user_settings`)
- **Extractor:** `AuthUser(user_id)`[cite: 1], `State(state)`[cite: 1], `Json(payload)`.
- **Request Payload:**
  ```rust
  #[derive(Deserialize)]
  pub struct UpdateSettingsRequest {
      pub api_key: Option<String>, // Some("") clears the key; Some("AIza...") updates it; None leaves it unchanged
      pub reference_image_path: Option<String>, // Some("") clears it; Some("users/...") updates it; None leaves it unchanged
  }
  ```
- **Validation:**
  - If `reference_image_path` is `Some(path)` and not empty, validate ownership using the existing `validate_image_path(&path, user_id)` helper in `routes.rs`[cite: 1].
  - If `api_key` is `Some(key)` and not empty, trim whitespace, validate length (`10..=256`), and encrypt via `crypto::encrypt_api_key`.
- **Database Operation:** Perform an `INSERT INTO public."user-settings" ... ON CONFLICT (user_id) DO UPDATE` updating only the provided fields and setting `updated_at = now()`[cite: 1].
- **Response:** Call and return the output of `get_user_settings`.

### 4.6 Implement Outfit Generation Endpoint (`src/routes.rs`)[cite: 1]

#### `POST /outfits/generate-preview` (`pub async fn generate_outfit_preview`)
- **Request Payload:**
  ```rust
  #[derive(Deserialize)]
  pub struct GeneratePreviewRequest {
      pub item_ids: Vec<Uuid>,
  }
  ```
- **Step 1: Input Validation**
  - Verify `!payload.item_ids.is_empty()` and `payload.item_ids.len() <= 8`. Return `400 Bad Request` if outside bounds.
- **Step 2: Fetch & Verify User Settings**
  - Query `public."user-settings"` for `user_id`[cite: 1].
  - If `encrypted_api_key` is null, return `(StatusCode::BAD_REQUEST, "Gemini API key is not configured in Settings.")`.
  - If `reference_image_path` is null, return `(StatusCode::BAD_REQUEST, "Full-body reference photo is not configured in Settings.")`.
  - Decrypt the API key in memory.
- **Step 3: Query Selected Garments & Metadata**
  - Execute a SQL query fetching the selected items along with their human-readable category and subcategory names[cite: 1]:
    ```sql
    SELECT 
        ci.id,
        ci.name,
        ci.color_id,
        ci.image_path,
        ci.notes,
        cc.name AS category_name,
        cs.name AS subcategory_name
    FROM public."clothing-items" ci
    LEFT JOIN public."clothing-categories" cc ON ci.category_id = cc.id
    LEFT JOIN public."clothing-subcategories" cs ON ci.subcategory_id = cs.id
    WHERE ci.id = ANY($1) AND ci.user_id = $2
    ```
  - Verify that all returned items have a non-null, non-empty `image_path`[cite: 1]. If any selected item lacks a photo, return `400 Bad Request` naming the specific item (e.g., `"Item 'Navy Blazer' does not have an uploaded photo."`).
- **Step 4: Concurrent Image Download from Supabase Storage**[cite: 1]
  - Extract the storage object keys from `reference_image_path` and each item's `image_path` using the existing `extract_object_path` helper[cite: 1].
  - Use `futures::future::try_join_all` (or `tokio::spawn` tasks) to download the reference image and all garment images concurrently via `state.storage.download_file(key)`[cite: 1].
  - Encode each downloaded image byte vector into a Base64 string using `base64::engine::general_purpose::STANDARD.encode(&bytes)`.
- **Step 5: Construct the Multimodal Gemini API Request**
  - Build a structured prompt string:
    ```text
    You are an expert virtual try-on image generator.
    Image 1 is the full-body reference photo of the user.
    Preserve the user's identity, face, hair, body proportions, pose, lighting, and background from Image 1.
    Replace the clothing currently worn by the person in Image 1 with the following garments provided in the subsequent images:
    - Image 2: [Category: <category_name> / <subcategory_name>, Color: <color_id>, Name: "<name>"]
    - Image 3: [Category: <category_name> / <subcategory_name>, Color: <color_id>, Name: "<name>"]
    ...
    Completely remove any existing clothing on the user in Image 1 that conflicts with the new outfit. Ensure realistic fabric drape, accurate garment colors, and natural proportions. Output only the final generated image.
    ```
  - Construct the JSON body for the Gemini API (`POST https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent`):
    - Pass the decrypted API key in the `x-goog-api-key` HTTP header (never in the URL query string, to avoid URL logging leaks).
    - Set request timeout to 60 seconds.
    - Body structure:
      ```json
      {
        "contents": [{
          "parts": [
            { "text": "<constructed_prompt>" },
            { "inlineData": { "mimeType": "<ref_mime>", "data": "<ref_base64>" } },
            { "inlineData": { "mimeType": "<item1_mime>", "data": "<item1_base64>" } }
          ]
        }],
        "generationConfig": {
          "responseModalities": ["IMAGE"]
        }
      }
      ```
- **Step 6: Parse Response & Save Generated Image**
  - Inspect the Gemini response JSON at `candidates[0].content.parts`. Locate the part containing `inlineData.data` (or `inline_data.data`).
  - If the API returns an error (e.g., 400 Invalid API Key, 429 Quota Exceeded, or safety block), map it to a clear error message and return an appropriate status code (`400` or `502`) so the frontend `ErrorDisplay.vue` can render actionable troubleshooting hints[cite: 1].
  - Decode the Base64 image data into raw bytes.
  - Pass the raw bytes through `upload::process_and_encode_image` to ensure max dimensions (1600px) and JPEG encoding[cite: 1].
  - Generate a collision-free storage key: `users/{user_id}/{uuid_v4}-tryon.jpg`[cite: 1].
  - Upload to Supabase Storage via `state.storage.upload_file(&key, processed_bytes).await`[cite: 1].
  - Return `200 OK`:
    ```json
    {
      "image_path": "users/<user_id>/<uuid>-tryon.jpg"
    }
    ```

---

## 5. Frontend Implementation (Vue 3 + TypeScript)[cite: 1]

### 5.1 TypeScript Interfaces (`frontend/src/api/types.ts`)[cite: 1]
Add the following interfaces to `types.ts`[cite: 1]:
```typescript
export interface UserSettings {
  has_api_key: boolean;
  api_key_hint: string | null;
  reference_image_path: string | null;
}

export interface UpdateUserSettingsPayload {
  api_key?: string;
  reference_image_path?: string;
}

export interface GeneratePreviewResponse {
  image_path: string;
}
```
Verify that `OutfitCreatePayload` (or the body passed to `POST /outfits`) includes `image_path?: string | null`[cite: 1]. (**Note:** Also double-check `POST /outfits` in `src/routes.rs` to ensure that if `image_path` is provided in the outfit creation payload, it is validated via `validate_image_path` and included in the `INSERT INTO outfits` statement[cite: 1]!).

### 5.2 API Client Methods (`frontend/src/api/client.ts`)[cite: 1]
Add three methods to the API client[cite: 1]:
1. `getSettings(): Promise<UserSettings>` -> calls `GET /api/settings`[cite: 1]
2. `updateSettings(payload: UpdateUserSettingsPayload): Promise<UserSettings>` -> calls `PUT /api/settings`[cite: 1]
3. `generateOutfitPreview(itemIds: string[]): Promise<GeneratePreviewResponse>` -> calls `POST /api/outfits/generate-preview` with `{ item_ids: itemIds }`[cite: 1].

### 5.3 Update Outfit Creation in Backend (`src/routes.rs`)[cite: 1]
In `src/routes.rs`, inspect the existing `POST /outfits` handler[cite: 1]. Currently, it inserts `outfits (user_id, date, notes, is_active, created_at, updated_at)`[cite: 1].
Update `POST /outfits` so that:
1. The request struct accepts `pub image_path: Option<String>`.
2. If `image_path` is `Some(ref path)`, it calls `validate_image_path(path, user_id)?`[cite: 1].
3. The `INSERT INTO outfits` statement includes `image_path`[cite: 1].

### 5.4 SVG Icons (`frontend/src/ui/components/icons.ts`)[cite: 1]
Add two new 24x24 SVG path entries to the icon map in `icons.ts`[cite: 1]:
- `settings`: Standard Material Design gear/cog icon.
- `sparkles` (or `auto_awesome`): Material Design stars/magic wand icon for the AI generation button.

### 5.5 Create Settings View (`frontend/src/components/SettingsView.vue`)
Create a new view component following the existing Material Design 3 token system (`tokens.css`)[cite: 1]:
1. **Section 1: Nano Banana (Gemini) API Key**
   - Displays current status: Badge showing `"Configured (ends in ...XXXX)"` or `"Not configured"`.
   - Password input field (`type="password"`) to enter a new API key, sanitized via `sanitizeText`[cite: 1].
   - `MdButton` (`variant="filled"`) to save the key, and `MdButton` (`variant="outlined"`) to clear an existing key[cite: 1].
   - Collapsible or clean instructional box explaining how to obtain a key:
     1. Visit Google AI Studio (`https://aistudio.google.com/apikey`).
     2. Create an API key associated with a project that has billing enabled for image generation (`gemini-2.5-flash-image`).
     3. Paste the key above. Explain that the key is encrypted with AES-256-GCM on the server.
2. **Section 2: Full-Body Reference Photo**
   - Uses `ImagePreview.vue` (or a dedicated file input + preview card) to display the current `reference_image_path` via the authenticated `/api/files/...` proxy[cite: 1].
   - File upload handler that validates the selected file using `validateImageFile(file)` from `lib/sanitize.ts`[cite: 1], uploads it via `POST /api/upload`[cite: 1], and immediately persists the returned `image_path` via `updateSettings({ reference_image_path })`.
   - Photo Guidelines Checklist (rendered as a clean M3 list or callout):
     - Wear form-fitting base clothing (e.g., plain t-shirt and shorts/leggings). Avoid bulky coats or loose dresses in the reference photo.
     - Stand facing the camera with arms relaxed at your sides (not crossed or in pockets).
     - Ensure your full body is visible from head to shoes against a plain, well-lit background.

### 5.6 Register Settings Tab (`App.vue` & `router.ts`)[cite: 1]
1. In `frontend/src/lib/router.ts`, add `'settings'` to the valid tab route union type[cite: 1].
2. In `frontend/src/App.vue`, add `{ id: 'settings', label: 'Settings', icon: 'settings' }` to the navigation drawer items (`MdNavDrawer`) and map it to `SettingsView.vue` inside the `<KeepAlive>` dynamic component switcher[cite: 1].

### 5.7 Integrate Try-On in `OutfitsView.vue`[cite: 1]
Modify the Outfit Logging Sidebar in `frontend/src/components/OutfitsView.vue`[cite: 1]:
1. **State:**
   - `isGeneratingPreview = ref(false)`
   - `generatedImagePath = ref<string | null>(null)`
   - `generationError = ref<unknown | null>(null)`
2. **UI Controls (Inside the "Add Outfit" form, below the Garment Picker)[cite: 1]:**
   - Render an `MdButton` (`variant="tonal"`, icon `sparkles`) labeled **"Generate Try-On Preview"**[cite: 1].
   - Disable the button when `selectedItemIds.length === 0` or `isGeneratingPreview === true`[cite: 1].
   - While `isGeneratingPreview` is true, show a loading state (`"Generating outfit preview (10–20s)..."`).
   - If `generationError` is populated, display it using `ErrorDisplay.vue`[cite: 1]. If the error indicates missing settings (no API key or reference photo), provide a quick action button to navigate to `#settings` (`router.navigate('settings')`)[cite: 1].
3. **Preview & Persistence:**
   - When `generateOutfitPreview(selectedItemIds)` succeeds, store the returned path in `generatedImagePath.value`.
   - Render a preview of the generated outfit image right in the form (using the `/api/files/{path}?width=720&token={jwt}` image proxy helper) with a button to **"Discard Preview"** or **"Regenerate"**[cite: 1].
   - When the user clicks **"Save Outfit"**, pass `image_path: generatedImagePath.value` in the `POST /api/outfits` payload[cite: 1].
4. **Display in Logged Outfit Cards & Calendar:**
   - In the sidebar list of logged outfits for the selected day, if `outfit.image_path` is present, display the generated outfit photo at the top of the outfit card (clickable to view full-size at 720px/1600px)[cite: 1].

---

## 6. Testing & Verification Requirements[cite: 1]

### 6.1 Backend Unit Tests (`cargo test`)[cite: 1]
Add unit tests in `src/crypto.rs` and `src/routes.rs`[cite: 1]:
1. **`crypto::tests::test_encrypt_decrypt_roundtrip`**: Verify that encrypting an API key and decrypting it with the same key and nonce yields the original plaintext.
2. **`crypto::tests::test_decrypt_wrong_key_fails`**: Verify that decrypting with a different 32-byte key returns an error.
3. **`crypto::tests::test_unique_nonces`**: Verify that encrypting the same plaintext twice produces different nonces and different ciphertexts.
4. **`routes::tests::test_user_settings_blocked_in_generic_crud`**: Verify that `user-settings` is rejected by the generic table sanitizer/validator.

### 6.2 Frontend Unit Tests (`npm test` & `npm run typecheck`)[cite: 1]
1. Update `frontend/src/lib/router.spec.ts` to test navigating to `#settings`[cite: 1].
2. Run `npm run typecheck` (`tsc --noEmit`) to verify strict TypeScript compliance across all modified components and API types[cite: 1].
3. Run `npm test` (Vitest) and ensure all existing 165 tests plus new tests pass with zero regressions[cite: 1].