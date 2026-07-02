#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

cargo_feature_args=(--all-features)
if ! pkg-config --exists webkitgtk-6.0; then
    cargo_feature_args=(--no-default-features)
fi

printf '\n==> Carousel compile diagnostic\n'
cargo check \
    --quiet \
    --message-format short \
    --locked \
    --all-targets \
    "${cargo_feature_args[@]}"
