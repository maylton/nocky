#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

printf '\n==> Phase 9 compilation diagnostic\n'
cargo check --locked --all-targets --all-features
printf '\nPhase 9 compilation diagnostic passed.\n'
