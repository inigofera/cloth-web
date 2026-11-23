#!/usr/bin/env bash
# Deploy a backend image tag on inibaby (plan §2.4), until CI-based CD exists.
#
#   ./deploy.sh            # deploy `latest` (every main push retags it)
#   ./deploy.sh <sha>      # pin — or roll back — to a specific commit's image
#
# The image is built and pushed by CI (GitHub Container Registry); this only
# pulls and recreates the container.
set -euo pipefail
cd "$(dirname "$0")"

export CLOTHWEB_TAG="${1:-latest}"
TAG="ghcr.io/inigofera/cloth-web-backend:${CLOTHWEB_TAG}"

echo "pulling $TAG"
docker compose pull backend
docker compose up -d
docker compose ps
echo "deployed $TAG"
