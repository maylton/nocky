#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

section() {
    printf '\n\033[1;34m==> %s\033[0m\n' "$1"
}

section "Toolchain"
rustc --version
cargo --version
rustfmt --version
cargo clippy --version
pkg-config --modversion gtk4
pkg-config --modversion libadwaita-1
pkg-config --modversion gstreamer-1.0

section "Formatting"
cargo fmt --all -- --check

section "Compilation"
cargo check \
    --locked \
    --all-targets \
    --all-features

section "Tests"
cargo test \
    --locked \
    --all-targets \
    --all-features

section "Clippy"
cargo clippy \
    --locked \
    --all-targets \
    --all-features \
    -- \
    -D warnings

if [[ -f helpers/nocky_youtube.py ]]; then
    section "Python helper"
    python3 -m py_compile helpers/nocky_youtube.py
fi

section "Shell syntax"
while IFS= read -r -d '' script; do
    bash -n "$script"
done < <(
    find . \
        -path './.git' -prune -o \
        -path './target' -prune -o \
        -type f -name '*.sh' -print0
)

section "Quality gate passed"
