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

SCRIPT_DIR = Path(__file__).resolve().parent
MODELS_DIR = Path(os.getenv("QWEN_MODELS_DIR", SCRIPT_DIR / "models")).resolve()

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


def find_sd_cli() -> Path | None:
    env_bin = os.getenv("SD_CLI_BIN")
    if env_bin:
        resolved = shutil.which(env_bin)
        if resolved:
            return Path(resolved).resolve()
        p = Path(env_bin)
        if p.exists():
            return p.resolve()

    candidates = [
        SCRIPT_DIR / "sd-cli.exe",
        SCRIPT_DIR / "sd.exe",
        SCRIPT_DIR / "sd-cli",
        SCRIPT_DIR / "sd",
        Path.cwd() / "sd-cli.exe",
        Path.cwd() / "sd.exe",
        Path.cwd() / "sd-cli",
        Path.cwd() / "sd",
        "sd-cli.exe",
        "sd-cli",
        "sd.exe",
        "sd",
    ]
    for c in candidates:
        if isinstance(c, Path) and c.exists():
            return c.resolve()
        resolved = shutil.which(str(c))
        if resolved:
            return Path(resolved).resolve()
    return None


DEFAULT_WIDTH = int(os.getenv("QWEN_DEFAULT_WIDTH", "832"))
DEFAULT_HEIGHT = int(os.getenv("QWEN_DEFAULT_HEIGHT", "1248"))


class GenerateRequest(BaseModel):
    prompt: str
    images_base64: list[str] = Field(..., min_length=2, max_length=10)
    width: int = DEFAULT_WIDTH
    height: int = DEFAULT_HEIGHT
    seed: int = 42


@app.get("/health")
def health_check():
    missing = [
        str(p)
        for p in (DIFFUSION_GGUF, LLM_GGUF, MMPROJ_GGUF, VAE_PATH)
        if not p.exists()
    ]
    bin_path = find_sd_cli()
    sd_cli_found = bin_path is not None
    ready = len(missing) == 0 and sd_cli_found
    return {
        "status": "ok" if ready else "degraded",
        "model": DIFFUSION_GGUF.name,
        "ready": ready,
        "sd_cli_found": sd_cli_found,
        "sd_cli_path": str(bin_path) if bin_path else None,
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

    bin_path = find_sd_cli()
    if not bin_path or not bin_path.exists():
        raise HTTPException(
            status_code=503,
            detail="sd-cli executable not found.",
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

        # Construct stable-diffusion.cpp command for RX 9070 XT (16 GB VRAM)
        cmd = [
            str(bin_path),
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
            "--flash-attn", "off,diffusion=on",
            "--vae-tiling",
            "-o", str(output_file),
        ]

        # Attach each reference image in slot order (-r)
        for ref_path in ref_paths:
            cmd.extend(["-r", str(ref_path)])

        timeout_sec = int(os.getenv("QWEN_TIMEOUT_SECONDS", "300"))
        try:
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=timeout_sec,
                cwd=str(bin_path.parent),
            )
        except subprocess.TimeoutExpired:
            raise HTTPException(
                status_code=504,
                detail=f"sd-cli execution timed out after {timeout_sec} seconds",
            )
        except Exception as exc:
            raise HTTPException(
                status_code=500,
                detail=f"Failed to execute sd-cli process: {exc}",
            )

        if proc.returncode != 0 or not output_file.exists():
            err_msg = proc.stderr[-1000:] if proc.stderr else "(no stderr output recorded)"
            raise HTTPException(
                status_code=500,
                detail=f"sd-cli execution failed (code {proc.returncode}): {err_msg}",
            )

        out_bytes = output_file.read_bytes()
        out_b64 = base64.b64encode(out_bytes).decode("ascii")

        return {
            "image_base64": out_b64,
            "mime_type": "image/png",
        }

