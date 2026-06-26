#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME="${IMAGE_NAME:-log-platform}"
IMAGE_TAG="${IMAGE_TAG:-local}"
CONTAINER_NAME="${CONTAINER_NAME:-nuwax-log-platform}"
HOST_PORT="${HOST_PORT:-8097}"
APP_PORT="${LOG_PLATFORM_PORT:-8097}"
QUICKWIT_URL="${QUICKWIT_URL:-http://host.docker.internal:7280}"
LOG_DIR="${LOG_DIR:-/Users/atan/Desktop/work/vs_code_nuwax/nuwax_deploy/docker/logs/log_platform}"
DATA_DIR="${DATA_DIR:-/Users/atan/Desktop/work/vs_code_nuwax/nuwax_deploy/docker/data/log_platform}"
WAIT_TIMEOUT_SECONDS="${WAIT_TIMEOUT_SECONDS:-120}"
PULL_IMAGE="${PULL_IMAGE:-false}"
IMAGE="${IMAGE_NAME}:${IMAGE_TAG}"

if [[ "$PULL_IMAGE" == "true" ]]; then
  docker pull "$IMAGE"
fi

mkdir -p "$LOG_DIR" "$DATA_DIR"

docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true

docker run -d \
  --name "$CONTAINER_NAME" \
  --restart=always \
  -p "${HOST_PORT}:${APP_PORT}" \
  -e RUST_LOG="${RUST_LOG:-info}" \
  -e RUST_BACKTRACE="${RUST_BACKTRACE:-1}" \
  -e LOG_PLATFORM_PORT="$APP_PORT" \
  -e LOG_PLATFORM_LOG_PATH=/app/logs \
  -e QUICKWIT_URL="$QUICKWIT_URL" \
  -v "$LOG_DIR:/app/logs" \
  -v "$DATA_DIR:/app/data" \
  "$IMAGE"

HEALTH_URL="${HEALTH_URL:-http://localhost:${HOST_PORT}/health}"
deadline=$((SECONDS + WAIT_TIMEOUT_SECONDS))
until curl -fsS "$HEALTH_URL" >/dev/null; do
  if (( SECONDS >= deadline )); then
    docker logs --tail=200 "$CONTAINER_NAME" || true
    printf 'Container %s failed health check: %s\n' "$CONTAINER_NAME" "$HEALTH_URL" >&2
    exit 1
  fi
  sleep 2
done

printf 'Deployed %s as %s\n' "$IMAGE" "$CONTAINER_NAME"
