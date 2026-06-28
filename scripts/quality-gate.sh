#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

printf '\n==> Phase 9 formatting diagnostic\n'
rustfmt --version
cargo fmt --all -- --check
printf '\nPhase 9 formatting diagnostic passed.\n'
