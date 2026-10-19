# Companion Specification: Hybrid Provider Support (Nano Banana + Local `Qwen-Image-2.1-Turbo`)

> **Target Repository:** `cloth-web`[cite: 1]
> **Prerequisite:** Virtual Try-On (Nano Banana) Implementation Specification
> **Objective:** Extend the Virtual Try-On feature to support a provider toggle between **Nano Banana** (Gemini Cloud API) and a locally hosted **`Qwen/Qwen-Image-2.1-Turbo`** instance.

---

## 1. Architectural Overview & Model Mechanics

### 1.1 Why `Qwen/Qwen-Image-2.1-Turbo` Fits This Architecture
`Qwen-Image-2.1-Turbo` is an 8-step accelerated checkpoint of `Qwen-Image-2.1` that pairs a 7B visual Single-Stream DiT generator with a `Qwen3-VL` (8B) vision-language encoder. 
1. **Native Multi-Image Editing:** Like Gemini, `QwenImage21Pipeline` natively accepts a Python list of multiple reference images (`image=[ref_img, item1_img, item2_img, ...]`, up to 10 images) and encodes them alongside the text prompt into a single joint sequence using block-causal attention.
2. **8-Step Turbo Schedule:** Unlike the base model (40 steps), the Turbo checkpoint embeds a pre-configured 8-step sigma schedule (`sample_sigmas` in `model_index.json`) and uses classifier-free guidance off by default (`true_cfg_scale=1.0`, 1 forward pass per step) with prefix KV cache reuse (`use_kv_cache=True`).
3. **Deterministic Image Indexing:** The `Qwen3-VL` processor labels input images sequentially in the prompt space (`Picture 1`, `Picture 2`, `Picture 3`, etc.).

### 1.2 Service Decoupling (Python Sidecar Microservice)
Because `Qwen-Image-2.1-Turbo` requires PyTorch (`torch>=2.4.0`), `transformers>=5.17.0`, and `diffusers` (built with PR #14950 pipeline-configured sampling sigmas support), it cannot run natively inside the Rust/Axum binary[cite: 1]. 
We will add a lightweight **FastAPI Python microservice** (`local_inference/`) that wraps `QwenImage21Pipeline` and exposes a local HTTP endpoint (`POST /generate`). The Rust Axum backend acts as a unified router that dispatches either to Google's Gemini API or to the user's configured local Qwen URL[cite: 1].

---

## 2. Database Migration Update

Update `supabase/migrations/20261018000000_create_user_settings.sql` (or create a subsequent migration `20261018000001_add_tryon_provider_columns.sql`) to store the user's selected provider and local endpoint URL[cite: 1]:

```sql
ALTER TABLE public."user-settings"
    ADD COLUMN IF NOT EXISTS tryon_provider text NOT NULL DEFAULT 'gemini',
    ADD COLUMN IF NOT EXISTS local_qwen_url text NOT NULL DEFAULT '[http://127.0.0.1:8001](http://127.0.0.1:8001)',
    ADD CONSTRAINT user_settings_tryon_provider_check
        CHECK (tryon_provider IN ('gemini', 'qwen_local'));
```

---
# Updated Section 3: Local Inference Server (`unsloth/Qwen-Image-2.1-Turbo-GGUF` — `Q6_K_XL`)

> **Replaces:** Section 3 of the Hybrid Provider Companion Specification
> **Target Quantization:** `qwen-image-2.1-turbo-Q6_K_XL.gguf` (6.72 GB Unsloth Dynamic 2.0 quantization)

---

## 3.1 Architecture of the GGUF Pipeline

A `.gguf` diffusion checkpoint contains **only the Diffusion Transformer (DiT) denoiser**. To run `qwen-image-2.1-turbo-Q6_K_XL.gguf` without blowing up VRAM on the 8B vision-language encoder, the local worker uses **`stable-diffusion.cpp`** (`sd-cli`, build `>= 2026-10-06` with commit `c150a6b` for custom sigma support) wrapped in a lightweight FastAPI server.

### Required Model Files (Total Disk/Memory Footprint: ~13.2 GB)
1. **Diffusion Model (DiT):**
   - Repo: `unsloth/Qwen-Image-2.1-Turbo-GGUF`
   - File: `qwen-image-2.1-turbo-Q6_K_XL.gguf` (6.72 GB)
2. **Vision-Language Text Encoder (`Qwen3-VL-8B`):**
   - Repo: `unsloth/Qwen3-VL-8B-Instruct-GGUF`
   - File 1 (LLM): `Qwen3-VL-8B-Instruct-UD-Q4_K_XL.gguf` (5.15 GB)
   - File 2 (Vision Projector for Multi-Image Edit): `mmproj-F16.gguf` (~0.7 GB)
3. **VAE:**
   - Repo: `unsloth/Qwen-Image-2.1-FP8`
   - File: `vae/qwen_image_2.1_vae_bf16.safetensors` (0.68 GB)

---

## 3.2 Directory Structure & Model Bootstrap Script

Create the following files in `local_inference/`:
```text
cloth-web/
└── local_inference/
    ├── download_models.sh
    ├── requirements.txt
    └── server.py
```

### 1. `local_inference/download_models.sh`
Provide a deterministic download script using the Hugging Face CLI (`hf download`):
```bash
#!/usr/bin/env bash
set -euo pipefail

mkdir -p models

echo "1/4 Downloading Qwen-Image-2.1-Turbo Q6_K_XL Denoiser (6.72 GB)..."
hf download unsloth/Qwen-Image-2.1-Turbo-GGUF \
  qwen-image-2.1-turbo-Q6_K_XL.gguf \
  --local-dir ./models

echo "2/4 Downloading Qwen3-VL-8B-Instruct UD-Q4_K_XL Text Encoder (5.15 GB)..."
hf download unsloth/Qwen3-VL-8B-Instruct-GGUF \
  Qwen3-VL-8B-Instruct-UD-Q4_K_XL.gguf \
  --local-dir ./models

echo "3/4 Downloading Qwen3-VL Vision Projector (mmproj)..."
hf download unsloth/Qwen3-VL-8B-Instruct-GGUF \
  mmproj-F16.gguf \
  --local-dir ./models

echo "4/4 Downloading Qwen-Image-2.1 BF16 VAE (0.68 GB)..."
hf download unsloth/Qwen-Image-2.1-FP8 \
  vae/qwen_image_2.1_vae_bf16.safetensors \
  --local-dir ./models

echo "All GGUF models downloaded to ./local_inference/models/"
```

### 2. `local_inference/requirements.txt`
Because inference is handled by the compiled `sd-cli` binary (or `diffusers` GGUF loader if configured), the Python wrapper dependencies are minimal:
```txt
fastapi>=0.115.0
uvicorn>=0.30.0
pydantic>=2.8.0
pillow>=10.0.0
huggingface_hub>=0.25.0
```

---

## 3.3 FastAPI GGUF Wrapper (`local_inference/server.py`)

Implement `local_inference/server.py`. This service supports two execution backends via `QWEN_BACKEND`:
- **`sd_cpp` (Default & Recommended for GGUF):** Invokes `sd-cli` (from `stable-diffusion.cpp`) with the quantized GGUF DiT, GGUF Qwen3-VL encoder, `mmproj`, and the explicit 8-step Turbo sigma schedule.
- **`diffusers_gguf` (Alternative Pure-Python Route):** Uses `QwenImageTransformer2DModel.from_single_file(..., quantization_config=GGUFQuantizationConfig(compute_dtype=torch.bfloat16))` injected into `QwenImage21Pipeline.from_pretrained("Qwen/Qwen-Image-2.1-Turbo", transformer=transformer)`.

```python
import base64
import io
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from PIL import Image
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

app = FastAPI(title="Cloth-Web Qwen-Image-2.1-Turbo Q6_K_XL GGUF Worker")

MODELS_DIR = Path(os.getenv("QWEN_MODELS_DIR", "./models"))
SD_CLI_BIN = os.getenv("SD_CLI_BIN", "sd-cli")

DIFFUSION_GGUF = MODELS_DIR / os.getenv(
    "QWEN_DIFFUSION_GGUF", "qwen-image-2.1-turbo-Q6_K_XL.gguf"
)
LLM_GGUF = MODELS_DIR / os.getenv(
    "QWEN_LLM_GGUF", "Qwen3-VL-8B-Instruct-UD-Q4_K_XL.gguf"
)
MMPROJ_GGUF = MODELS_DIR / os.getenv(
    "QWEN_MMPROJ_GGUF", "mmproj-F16.gguf"
)
VAE_PATH = MODELS_DIR / os.getenv(
    "QWEN_VAE_PATH", "vae/qwen_image_2.1_vae_bf16.safetensors"
)

# Exact 8-step Turbo schedule from Qwen/Qwen-Image-2.1-Turbo model_index.json + terminal 0.0
TURBO_SIGMAS = "1.0,0.978453,0.95418,0.926626,0.89508,0.845148,0.704534,0.414568,0.0"


class GenerateRequest(BaseModel):
    prompt: str
    images_base64: list[str] = Field(..., min_length=2, max_length=10)
    width: int = 1024
    height: int = 1536
    seed: int = 42


@app.get("/health")
def health_check():
    missing = [
        str(p)
        for p in (DIFFUSION_GGUF, LLM_GGUF, MMPROJ_GGUF, VAE_PATH)
        if not p.exists()
    ]
    sd_cli_found = shutil.which(SD_CLI_BIN) is not None or Path(SD_CLI_BIN).exists()
    ready = len(missing) == 0 and sd_cli_found
    return {
        "status": "ok" if ready else "degraded",
        "model": DIFFUSION_GGUF.name,
        "ready": ready,
        "sd_cli_found": sd_cli_found,
        "missing_files": missing,
    }


@app.post("/generate")
def generate_image(req: GenerateRequest):
    health = health_check()
    if not health["ready"]:
        raise HTTPException(
            status_code=503,
            detail=f"Local GGUF worker not ready: {health}",
        )

    with tempfile.TemporaryDirectory(prefix="cloth_tryon_") as tmpdir:
        tmp_path = Path(tmpdir)
        ref_paths: list[Path] = []

        # Write reference image (Picture 1) and garment images (Picture 2..N) to disk
        for idx, b64_str in enumerate(req.images_base64, start=1):
            try:
                raw_bytes = base64.b64decode(b64_str)
                img = Image.open(io.BytesIO(raw_bytes)).convert("RGB")
                img_file = tmp_path / f"input_{idx}.png"
                img.save(img_file, format="PNG")
                ref_paths.append(img_file)
            except Exception as exc:
                raise HTTPException(
                    status_code=400,
                    detail=f"Invalid image at index {idx}: {exc}",
                )

        output_file = tmp_path / "output.png"

        # Construct stable-diffusion.cpp command for Qwen-Image-2.1-Turbo GGUF
        cmd = [
            SD_CLI_BIN,
            "--diffusion-model", str(DIFFUSION_GGUF),
            "--vae", str(VAE_PATH),
            "--llm", str(LLM_GGUF),
            "--llm_vision", str(MMPROJ_GGUF),
            "-p", req.prompt,
            "--steps", "8",
            "--cfg-scale", "1.0",
            "--sampling-method", "euler",
            "--sigmas", TURBO_SIGMAS,
            "-W", str(req.width),
            "-H", str(req.height),
            "--seed", str(req.seed),
            "--diffusion-fa",
            "-o", str(output_file),
        ]

        # Attach each reference image in slot order (-r)
        for ref_path in ref_paths:
            cmd.extend(["-r", str(ref_path)])

        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=115,
        )

        if proc.returncode != 0 or not output_file.exists():
            raise HTTPException(
                status_code=500,
                detail=f"sd-cli execution failed (code {proc.returncode}): {proc.stderr[-500:]}",
            )

        out_bytes = output_file.read_bytes()
        out_b64 = base64.b64encode(out_bytes).decode("ascii")

        return {
            "image_base64": out_b64,
            "mime_type": "image/png",
        }
```

---

## 3.4 Pure-Python Diffusers Alternative (Optional If Not Using `sd-cli`)

If you prefer to run `qwen-image-2.1-turbo-Q6_K_XL.gguf` inside pure Python `diffusers` without compiling `stable-diffusion.cpp`, install `gguf>=0.10.0` in `requirements.txt` and load the single-file GGUF transformer into `QwenImage21Pipeline`:

```python
import torch
from diffusers import (
    QwenImage21Pipeline,
    QwenImageTransformer2DModel,
    GGUFQuantizationConfig,
)

transformer = QwenImageTransformer2DModel.from_single_file(
    "[https://huggingface.co/unsloth/Qwen-Image-2.1-Turbo-GGUF/blob/main/qwen-image-2.1-turbo-Q6_K_XL.gguf](https://huggingface.co/unsloth/Qwen-Image-2.1-Turbo-GGUF/blob/main/qwen-image-2.1-turbo-Q6_K_XL.gguf)",
    quantization_config=GGUFQuantizationConfig(compute_dtype=torch.bfloat16),
    torch_dtype=torch.bfloat16,
)

pipe = QwenImage21Pipeline.from_pretrained(
    "Qwen/Qwen-Image-2.1-Turbo",
    transformer=transformer,
    torch_dtype=torch.bfloat16,
)
pipe.enable_model_cpu_offload()
```
*Note on the Diffusers route:* While this loads the `6.72 GB` `Q6_K_XL` GGUF for the DiT, `QwenImage21Pipeline.from_pretrained("Qwen/Qwen-Image-2.1-Turbo")` will still download the `17.5 GB` `BF16` `Qwen3-VL-8B` text encoder and swap it into VRAM during prompt encoding via `enable_model_cpu_offload()`. If you want **both** the DiT and the `Qwen3-VL-8B` encoder to stay quantized (`6.72 GB` + `5.15 GB`), use the `sd-cli` implementation in **Section 3.3**.
---

## 4. Backend Updates (Rust / Axum)[cite: 1]

### 4.1 Environment Variables (`.env`)[cite: 1]
Add a security toggle to `.env` to control whether non-loopback URLs are permitted for the local Qwen endpoint[cite: 1]:
```env
# Set to "true" only if the Qwen inference server runs on another LAN machine (e.g., 192.168.x.x).
# When false (default), only 127.0.0.1 and localhost are permitted to prevent SSRF attacks.
ALLOW_LAN_INFERENCE_URLS=false
```

### 4.2 SSRF Validation Helper (`src/routes.rs`)[cite: 1]
Because users can input `local_qwen_url` in Settings, you **must** validate the URL to prevent Server-Side Request Forgery (SSRF) against internal cloud metadata endpoints (e.g., `169.254.169.254`) or internal Supabase services:
1. Parse `local_qwen_url` using `reqwest::Url::parse`.
2. Enforce scheme is `http` or `https`.
3. By default (`ALLOW_LAN_INFERENCE_URLS != "true"`), verify `url.host_str()` is strictly `"127.0.0.1"` or `"localhost"`, and the port is not `3000` (Axum itself) or `5432` (Postgres)[cite: 1].
4. If `ALLOW_LAN_INFERENCE_URLS == "true"`, additionally allow RFC 1918 private LAN addresses (`192.168.x.x`, `10.x.x.x`), while still blocking link-local (`169.254.x.x`).

### 4.3 Update Settings Endpoints (`src/routes.rs`)[cite: 1]

#### `GET /settings`
Update the response JSON to include the new columns:
```json
{
  "tryon_provider": "gemini",
  "has_api_key": true,
  "api_key_hint": "...a1b2",
  "local_qwen_url": "[http://127.0.0.1:8001](http://127.0.0.1:8001)",
  "reference_image_path": "users/<user_id>/uuid-reference.jpg"
}
```

#### `PUT /settings`
Update `UpdateSettingsRequest`:
```rust
#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub tryon_provider: Option<String>, // "gemini" | "qwen_local"
    pub api_key: Option<String>,
    pub local_qwen_url: Option<String>,
    pub reference_image_path: Option<String>,
}
```
- Validate `tryon_provider` is either `"gemini"` or `"qwen_local"`.
- Validate `local_qwen_url` using the SSRF validator before writing to PostgreSQL[cite: 1].

#### `GET /settings/test-qwen` (New Endpoint)
Add `.route("/settings/test-qwen", get(routes::test_qwen_connection))` in `src/main.rs`[cite: 1]:
- Reads the user's configured `local_qwen_url` (or accepts `?url=...` query param, validated via the SSRF helper).
- Sends a `GET {local_qwen_url}/health` request with a 3-second timeout.
- Returns `200 OK` with the upstream health status if reachable, or `502 Bad Gateway` with troubleshooting details if the local Python server is offline.

### 4.4 Provider Dispatch in `POST /outfits/generate-preview` (`src/routes.rs`)[cite: 1]
Refactor Step 5 of `generate_outfit_preview` to branch based on `user_settings.tryon_provider`:

#### Branch A: `tryon_provider == "gemini"`
- Executes the Gemini `gemini-2.5-flash-image` API call as specified in the primary manual.

#### Branch B: `tryon_provider == "qwen_local"`
1. **No API Key Required:** Skip the `encrypted_api_key` check. Verify `local_qwen_url` passes SSRF validation.
2. **Construct Qwen-Specific Prompt:**
   `Qwen3-VL` inside `QwenImage21Pipeline` indexes condition images as `Picture 1`, `Picture 2`, `Picture 3`, etc., in the exact order they are passed in the `images_base64` array. Construct the prompt using Qwen's native naming convention:
   ```text
   Keep the person's identity, face, hair, pose, body proportions, and background from Picture 1 unchanged. Replace the clothes worn by the person in Picture 1 with the garments shown in the following pictures:
   - Picture 2: <category_name> (<subcategory_name>) in <color_id>, "<name>"
   - Picture 3: <category_name> (<subcategory_name>) in <color_id>, "<name>"
   Remove any existing clothing in Picture 1 that conflicts with these garments. Ensure natural fabric drape, realistic lighting, and accurate garment details.
   ```
3. **Determine Aspect Ratio / Resolution:**
   - Standard portrait try-on resolution for `Qwen-Image-2.1-Turbo`: `width: 1024, height: 1536` (2:3 aspect ratio, which fits within 16GB–24GB VRAM much faster than 2K `1696x2528` while matching the 1600px ceiling in `upload.rs`)[cite: 1].
4. **Dispatch Request to Local Server:**
   - Send `POST {local_qwen_url}/generate` with a **120-second timeout** (local consumer GPUs with CPU offload take ~15–35 seconds for 8 steps):
     ```json
     {
       "prompt": "<qwen_constructed_prompt>",
       "images_base64": ["<ref_b64>", "<item1_b64>", "<item2_b64>"],
       "width": 1024,
       "height": 1536,
       "seed": 42
     }
     ```
5. **Process Output:**
   - Parse `image_base64` from the response, decode from Base64, and feed the raw bytes into the exact same `upload::process_and_encode_image` and `state.storage.upload_file` pipeline used by the Gemini branch[cite: 1].

---

## 5. Frontend Updates (Vue 3 + TypeScript)[cite: 1]

### 5.1 Update TypeScript Types (`frontend/src/api/types.ts`)[cite: 1]
```typescript
export type TryOnProvider = 'gemini' | 'qwen_local';

export interface UserSettings {
  tryon_provider: TryOnProvider;
  has_api_key: boolean;
  api_key_hint: string | null;
  local_qwen_url: string;
  reference_image_path: string | null;
}

export interface UpdateUserSettingsPayload {
  tryon_provider?: TryOnProvider;
  api_key?: string;
  local_qwen_url?: string;
  reference_image_path?: string;
}
```

### 5.2 Update API Client (`frontend/src/api/client.ts`)[cite: 1]
Add a helper to test the local Qwen server:
```typescript
testQwenConnection(): Promise<{ status: string; model: string; ready: boolean }>
```
Calling `GET /api/settings/test-qwen`.

### 5.3 Update `SettingsView.vue`
In `frontend/src/components/SettingsView.vue`, update **Section 1** to include a Material Design 3 segmented button or radio card selector for **Try-On Engine**:

1. **Option A: Nano Banana (Google Gemini API)**
   - Shows the API Key input field, status badge, and AI Studio instructions.
2. **Option B: Local `Qwen-Image-2.1-Turbo` (Self-Hosted)**
   - Shows a text input for **Local Server URL** (default `http://127.0.0.1:8001`).
   - Includes a **"Test Connection"** `MdButton` (`variant="tonal"`)[cite: 1] that calls `testQwenConnection()` and displays a green `"Connected to Qwen/Qwen-Image-2.1-Turbo"` badge or a red error banner via `ErrorDisplay.vue`[cite: 1].
   - Displays a concise setup guide for the user:
     ```bash
     cd local_inference
     pip install -r requirements.txt
     uvicorn server:app --host 127.0.0.1 --port 8001
     ```
   - Includes a hardware note: *"Requires an NVIDIA GPU with ≥16GB VRAM (using CPU offload) or ≥32GB VRAM (native BF16). Uses the 8-step Turbo schedule."*

---

## 6. Testing & Verification Additions[cite: 1]

1. **Backend SSRF Unit Tests (`cargo test`)**[cite: 1]:
   - `routes::tests::test_ssrf_blocks_metadata_ip`: Verify `http://169.254.169.254/latest/meta-data` is rejected.
   - `routes::tests::test_ssrf_blocks_internal_ports`: Verify `http://127.0.0.1:3000` and `http://127.0.0.1:5432` are rejected[cite: 1].
   - `routes::tests::test_ssrf_allows_local_qwen`: Verify `http://127.0.0.1:8001` is accepted.
2. **Prompt Formatting Unit Test (`cargo test`)**[cite: 1]:
   - Verify that `build_qwen_prompt` maps the reference image to `Picture 1` and N clothing items to `Picture 2` through `Picture N+1`.

---

## 7. Hardware Addendum: AMD Radeon RX 9070 XT (16 GB VRAM, RDNA 4)

> **Target GPU:** AMD Radeon RX 9070 XT (16 GB GDDR6, `gfx1201`)
> **Applies To:** `local_inference/` in the Hybrid Provider Companion Specification

### 7.1 Compiling `stable-diffusion.cpp` for RX 9070 XT
Compile `stable-diffusion.cpp` using the **Vulkan** backend (cross-platform Linux/Windows, zero ROCm driver fragility) or **HIP/ROCm** backend:

#### Option A: Vulkan Backend (Recommended for RX 9070 XT)
```bash
git clone --recursive https://github.com/leejet/stable-diffusion.cpp
cd stable-diffusion.cpp
cmake -B build -DSD_VULKAN=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j $(nproc)
cp build/bin/sd-cli ../local_inference/
```

#### Option B: ROCm / hipBLAS Backend (Linux Only, Native `gfx1201`)
```bash
git clone --recursive https://github.com/leejet/stable-diffusion.cpp
cd stable-diffusion.cpp
cmake -B build -DSD_HIPBLAS=ON -DAMDGPU_TARGETS="gfx1201" -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j $(nproc)
cp build/bin/sd-cli ../local_inference/
```

### 7.2 Updated `cmd` Flags in `local_inference/server.py`
To prevent 16 GB VRAM OOM, the command includes `--offload-to-cpu` and `--vae-tiling`.