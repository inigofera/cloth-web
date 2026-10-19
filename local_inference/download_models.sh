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

