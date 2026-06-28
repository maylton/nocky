#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER="${NOCKY_YOUTUBE_HELPER:-${ROOT_DIR}/helpers/nocky_youtube.py}"
CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/.cache}/nocky/youtube"

find_runtime_python() {
    if [[ -n "${NOCKY_PYTHON:-}" && -x "${NOCKY_PYTHON}" ]]; then
        printf '%s\n' "${NOCKY_PYTHON}"
        return
    fi

    for candidate in \
        "${ROOT_DIR}/.nocky-runtime/bin/python3" \
        "${HOME}/.local/share/nocky/runtime/bin/python3" \
        "/usr/local/share/nocky/runtime/bin/python3" \
        "/usr/share/nocky/runtime/bin/python3"; do
        if [[ -x "${candidate}" ]]; then
            printf '%s\n' "${candidate}"
            return
        fi
    done

    command -v python3 || true
}

PYTHON="$(find_runtime_python)"
[[ -n "${PYTHON}" ]] || { echo "Python 3 was not found." >&2; exit 1; }
[[ -f "${HELPER}" ]] || { echo "YouTube helper not found: ${HELPER}" >&2; exit 1; }

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

invoke() {
    local command="$1"
    local payload="$2"
    local output_path="$3"

    printf '%s\n' "${payload}" \
        | "${PYTHON}" "${HELPER}" "${command}" \
        > "${output_path}"
}

status_path="${TMP_DIR}/status.json"
invoke status '{}' "${status_path}"

"${PYTHON}" - "${status_path}" <<'PY'
import json
import pathlib
import sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
if not payload.get("ok"):
    raise SystemExit(payload.get("error") or "YouTube helper status failed")
status = payload.get("result") or {}
if not status.get("connected"):
    raise SystemExit("YouTube Music account session is not connected")
print("Account session: connected")
print("Storage backend:", status.get("storage") or "unknown")
PY

page_commands=(home_v2 library_v2 library_page_v2 liked_v2)
for command in "${page_commands[@]}"; do
    output_path="${TMP_DIR}/${command}.json"
    invoke "${command}" '{"section_limit":6,"limit":160}' "${output_path}"
done

"${PYTHON}" - "${TMP_DIR}" <<'PY'
from __future__ import annotations

import json
import pathlib
import sys
from collections import Counter

root = pathlib.Path(sys.argv[1])
commands = ("home_v2", "library_v2", "library_page_v2", "liked_v2")
card_layouts = {"carousel", "mixed", "quick_picks"}

pages: dict[str, dict] = {}
for command in commands:
    response = json.loads((root / f"{command}.json").read_text(encoding="utf-8"))
    if not response.get("ok"):
        raise SystemExit(f"{command}: {response.get('error') or 'helper failed'}")
    page = response.get("result") or {}
    if page.get("version") != 2:
        raise SystemExit(f"{command}: unexpected contract version {page.get('version')!r}")
    sections = page.get("sections") or []
    if not sections:
        raise SystemExit(f"{command}: no structured sections returned")
    pages[command] = page

home = pages["home_v2"]
home_sections = home.get("sections") or []
if not any((section.get("layout") or "").lower() in card_layouts for section in home_sections):
    raise SystemExit("home_v2: no card-capable section returned")
if not home.get("continuation"):
    raise SystemExit("home_v2: expected a continuation token for the first page")

for command in ("library_v2", "library_page_v2", "liked_v2"):
    sections = pages[command].get("sections") or []
    card_indexes = [
        index
        for index, section in enumerate(sections)
        if (section.get("layout") or "").lower() in card_layouts
    ]
    list_indexes = [
        index
        for index, section in enumerate(sections)
        if (section.get("layout") or "").lower() == "list"
    ]
    if not card_indexes:
        raise SystemExit(f"{command}: no collection card section returned")
    if list_indexes and max(card_indexes) > min(list_indexes):
        raise SystemExit(f"{command}: long list appears before a card section")

for command in commands:
    page = pages[command]
    sections = page.get("sections") or []
    layout_counts = Counter((section.get("layout") or "unknown") for section in sections)
    item_types = Counter(
        item.get("result_type") or "unknown"
        for section in sections
        for item in (section.get("items") or [])
        if isinstance(item, dict)
    )
    print(
        f"{command}: sections={len(sections)} "
        f"layouts={dict(layout_counts)} types={dict(item_types)}"
    )
PY

continuation="$(${PYTHON} - "${TMP_DIR}/home_v2.json" <<'PY'
import json
import pathlib
import sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
print((payload.get("result") or {}).get("continuation") or "")
PY
)"

continuation_path="${TMP_DIR}/home_continuation.json"
invoke home_v2 "{\"continuation\":\"${continuation}\",\"section_limit\":6}" "${continuation_path}"

"${PYTHON}" - "${TMP_DIR}/home_v2.json" "${continuation_path}" <<'PY'
import json
import pathlib
import sys

first = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
second = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
if not second.get("ok"):
    raise SystemExit(second.get("error") or "home continuation failed")

first_sections = (first.get("result") or {}).get("sections") or []
second_sections = (second.get("result") or {}).get("sections") or []
if not second_sections:
    raise SystemExit("home continuation returned no sections")

first_ids = {section.get("id") for section in first_sections if section.get("id")}
second_ids = {section.get("id") for section in second_sections if section.get("id")}
if first_ids & second_ids:
    raise SystemExit("home continuation repeated section IDs from the first page")
print("Continuation: returned new structured sections")
PY

policy_path="${TMP_DIR}/stream_clients.json"
invoke stream_clients '{}' "${policy_path}"

"${PYTHON}" - "${policy_path}" <<'PY'
import json
import pathlib
import sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
if not payload.get("ok"):
    raise SystemExit(payload.get("error") or "stream client policy failed")
result = payload.get("result") or {}
order = result.get("order") or []
if not order:
    raise SystemExit("stream client policy returned an empty order")
print("Stream client order:", " -> ".join(order))
PY

"${PYTHON}" - "${CACHE_ROOT}" <<'PY'
import json
import os
import pathlib
import stat
import sys

root = pathlib.Path(sys.argv[1])
feed = root / "home-feed-cache.json"
if not feed.is_file():
    raise SystemExit("Structured feed cache was not created")
mode = stat.S_IMODE(feed.stat().st_mode)
if mode & 0o077:
    raise SystemExit(f"Structured feed cache permissions are too broad: {mode:04o}")
payload = json.loads(feed.read_text(encoding="utf-8"))
pages = payload.get("pages") or {}
if not pages:
    raise SystemExit("Structured feed cache contains no pages")
print(f"Structured feed cache: {len(pages)} page entries, mode {mode:04o}")
PY

printf '\nStructured YouTube Music smoke check passed.\n'
