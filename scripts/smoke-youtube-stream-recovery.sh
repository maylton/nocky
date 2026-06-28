#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER="${NOCKY_YOUTUBE_HELPER:-${ROOT_DIR}/helpers/nocky_youtube.py}"
CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/.cache}/nocky/youtube"
STREAM_CACHE="${CACHE_ROOT}/stream-cache.json"

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

if pgrep -x nocky >/dev/null 2>&1; then
    echo "Close Nocky before running this smoke test so its stream cache can be restored safely." >&2
    exit 1
fi

TMP_DIR="$(mktemp -d)"
mkdir -p "${CACHE_ROOT}"
chmod 700 "${CACHE_ROOT}" 2>/dev/null || true

had_cache=false
if [[ -f "${STREAM_CACHE}" ]]; then
    cp -p "${STREAM_CACHE}" "${TMP_DIR}/stream-cache.backup.json"
    had_cache=true
fi

restore() {
    if [[ "${had_cache}" == true ]]; then
        cp -p "${TMP_DIR}/stream-cache.backup.json" "${STREAM_CACHE}"
        chmod 600 "${STREAM_CACHE}"
    else
        rm -f "${STREAM_CACHE}"
    fi
    rm -rf "${TMP_DIR}"
}
trap restore EXIT

rm -f "${STREAM_CACHE}"

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
if not (payload.get("result") or {}).get("connected"):
    raise SystemExit("YouTube Music account session is not connected")
print("Account session: connected")
PY

home_path="${TMP_DIR}/home.json"
invoke home_v2 '{"section_limit":6}' "${home_path}"

video_id="$(${PYTHON} - "${home_path}" <<'PY'
import json
import pathlib
import sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
if not payload.get("ok"):
    raise SystemExit(payload.get("error") or "home_v2 failed")
for section in (payload.get("result") or {}).get("sections") or []:
    for item in section.get("items") or []:
        video_id = str(item.get("video_id") or "").strip()
        if len(video_id) == 11:
            print(video_id)
            raise SystemExit(0)
raise SystemExit("No playable track was returned by home_v2")
PY
)"

first_path="${TMP_DIR}/first.json"
invoke resolve "{\"video_id\":\"${video_id}\",\"force\":false}" "${first_path}"

second_path="${TMP_DIR}/second.json"
invoke resolve "{\"video_id\":\"${video_id}\",\"force\":true}" "${second_path}"

"${PYTHON}" - "${first_path}" "${second_path}" <<'PY'
import json
import pathlib
import sys

first = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
second = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))

if not first.get("ok"):
    raise SystemExit(first.get("error") or "initial stream resolution failed")
if not second.get("ok"):
    raise SystemExit(second.get("error") or "forced recovery resolution failed")

initial = first.get("result") or {}
recovery = second.get("result") or {}
initial_client = str(initial.get("stream_client") or "").strip()
recovery_client = str(recovery.get("stream_client") or "").strip()
attempted = [str(value) for value in recovery.get("attempted_clients") or []]

if not initial_client:
    raise SystemExit("initial resolution did not report a stream client")
if not recovery_client:
    raise SystemExit("recovery resolution did not report a stream client")
if not attempted:
    raise SystemExit("recovery resolution reported no attempted clients")
if attempted[0] == initial_client:
    raise SystemExit("forced recovery retried the previously selected client first")

print("Initial client:", initial_client)
print("Recovery first attempt:", attempted[0])
print("Recovery selected client:", recovery_client)
print("Recovery attempts:", len(attempted))
print("Stream client rotation: OK")
PY

printf 'YouTube stream-recovery smoke test passed. Original stream cache restored.\n'
