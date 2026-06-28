#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER="${NOCKY_YOUTUBE_HELPER:-${ROOT_DIR}/helpers/nocky_youtube.py}"
CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/.cache}/nocky/youtube"
FEED_CACHE="${CACHE_ROOT}/home-feed-v2.json"

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
    local output_path="$1"
    shift
    printf '%s\n' '{"section_limit":6}' \
        | "$@" "${PYTHON}" "${HELPER}" home_v2 \
        > "${output_path}"
}

online_path="${TMP_DIR}/online.json"
invoke "${online_path}" env

[[ -f "${FEED_CACHE}" ]] || {
    echo "Structured feed cache was not created at ${FEED_CACHE}" >&2
    exit 1
}

mode="$(${PYTHON} - "${FEED_CACHE}" <<'PY'
import pathlib
import stat
import sys
path = pathlib.Path(sys.argv[1])
print(f"{stat.S_IMODE(path.stat().st_mode):04o}")
PY
)"

if [[ "${mode}" != "0600" ]]; then
    echo "Structured feed cache permissions are ${mode}, expected 0600" >&2
    exit 1
fi

offline_path="${TMP_DIR}/offline.json"
invoke "${offline_path}" env \
    HTTP_PROXY=http://127.0.0.1:9 \
    HTTPS_PROXY=http://127.0.0.1:9 \
    ALL_PROXY=http://127.0.0.1:9 \
    http_proxy=http://127.0.0.1:9 \
    https_proxy=http://127.0.0.1:9 \
    all_proxy=http://127.0.0.1:9 \
    NO_PROXY= \
    no_proxy=

"${PYTHON}" - "${online_path}" "${offline_path}" <<'PY'
import json
import pathlib
import sys

online = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
offline = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))

if not online.get("ok"):
    raise SystemExit(online.get("error") or "online home_v2 request failed")
if not offline.get("ok"):
    raise SystemExit(offline.get("error") or "offline home_v2 request failed")

online_page = online.get("result") or {}
offline_page = offline.get("result") or {}

if not online_page.get("sections"):
    raise SystemExit("online home_v2 returned no sections")
if not offline_page.get("sections"):
    raise SystemExit("stale fallback returned no sections")
if not offline_page.get("stale"):
    raise SystemExit("offline request did not mark the cached page as stale")

print("Online structured page: OK")
print("Offline stale fallback: OK")
print("Returned sections:", len(offline_page.get("sections") or []))
PY

printf 'Structured stale-cache smoke test passed.\n'
