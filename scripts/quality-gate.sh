#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

python3 -m py_compile \
    helpers/nocky_youtube.py \
    helpers/nocky_youtube_feed.py \
    helpers/nocky_stream_clients.py \
    scripts/smoke_youtube_stream_preferences.py

python3 -m unittest discover -s tests -p 'test_youtube_*.py' -v
python3 scripts/smoke_youtube_stream_preferences.py
