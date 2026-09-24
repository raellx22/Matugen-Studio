#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v docker >/dev/null || { echo "Docker is required for the local Ubuntu 22.04 build" >&2; exit 1; }
docker build --file "$root_dir/Dockerfile.release" --tag matugen-studio:2.0.0-rc.1 "$root_dir"
container_id="$(docker create matugen-studio:2.0.0-rc.1)"
trap 'docker rm "$container_id" >/dev/null' EXIT
mkdir -p "$root_dir/dist"
docker cp "$container_id:/release/." "$root_dir/dist/"
cd "$root_dir/dist"
sha256sum --check SHA256SUMS
