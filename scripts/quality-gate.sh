#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

section() {
    printf '\n033[1;34m==> %s\033[0m\n' "$1"
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
    python3 -m py_compile \
        helpers/nocky_youtube.py \
        helpers/nocky_youtube_feed.py \
        helpers/nocky_stream_clients.py \
        scripts/smoke_youtube_stream_preferences.py
    python3 -m unittest discover -s tests -p 'test_youtube_*.py' -v
    python3 scripts/smoke_youtube_stream_preferences.py
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

section "Release metadata"
bash ./scripts/verify-release.sh

section "Quality gate passed"
