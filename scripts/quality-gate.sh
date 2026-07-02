#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

printf '\n\033[1;34m==> Carousel diagnostic: formatting\033[0m\n'
cargo fmt --all -- --check

echo "Formatting diagnostic passed"
