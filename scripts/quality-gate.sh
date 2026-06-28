#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! cargo clippy --locked --all-targets --all-features -- -D warnings \
    > /tmp/nocky-clippy.log 2>&1; then
    tail -n 160 /tmp/nocky-clippy.log
    exit 1
fi

tail -n 40 /tmp/nocky-clippy.log
