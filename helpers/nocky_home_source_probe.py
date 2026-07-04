#!/usr/bin/env python3
"""Probe YouTube Music Home source freshness for Nocky.

This is a diagnostic script. It does not write Nocky cache files unless
``command_home_v2`` is explicitly used outside this script.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path
from typing import Any

from nocky_youtube import (
    _create_client,
    _feed_cache_key,
    _home_feed_cache_path,
    _inner_tube_home_response,
    _inner_tube_home_rows,
    _load_session,
    _text,
)
from nocky_youtube_feed import (
    build_structured_home,
    extract_inner_tube_home_chips,
    find_inner_tube_home_section_list,
)

PREFIX = "[YT_HOME_SOURCE_TRACE]"


def stable_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def digest(value: Any, length: int = 16) -> str:
    return hashlib.sha256(stable_json(value).encode("utf-8")).hexdigest()[:length]


def compact(value: Any, limit: int = 160) -> str:
    text = str(value or "").replace("\n", " ").replace("\r", " ").replace("\t", " ")
    text = " ".join(text.split())
    return text if len(text) <= limit else text[: limit - 1] + "…"


def log(event: str, **fields: Any) -> None:
    parts = [f"event={json.dumps(event, ensure_ascii=False)}"]
    for key, value in fields.items():
        if isinstance(value, (int, float, bool)) or value is None:
            parts.append(f"{key}={json.dumps(value, ensure_ascii=False)}")
        else:
            parts.append(f"{key}={json.dumps(compact(value), ensure_ascii=False)}")
    print(f"{PREFIX} " + " ".join(parts), file=sys.stderr)


def cache_state(cache_key: str) -> dict[str, Any]:
    path = _home_feed_cache_path()
    result: dict[str, Any] = {
        "path": str(path),
        "exists": path.is_file(),
        "entry_exists": False,
        "entry_saved_at": 0,
        "entry_age_seconds": None,
        "entry_page_hash": "",
        "entry_sections": 0,
    }
    if not path.is_file():
        return result
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except Exception as error:
        result["error"] = str(error)
        return result
    pages = payload.get("pages") if isinstance(payload, dict) else {}
    entry = pages.get(cache_key) if isinstance(pages, dict) else None
    if not isinstance(entry, dict) or not isinstance(entry.get("page"), dict):
        return result
    page = entry["page"]
    saved_at = int(entry.get("saved_at") or 0)
    result.update(
        {
            "entry_exists": True,
            "entry_saved_at": saved_at,
            "entry_age_seconds": max(0, int(time.time()) - saved_at) if saved_at else None,
            "entry_page_hash": digest(page),
            "entry_sections": len(page.get("sections") or []),
        }
    )
    return result


def raw_section_summaries(response: dict[str, Any], max_sections: int, max_items: int) -> list[dict[str, Any]]:
    section_list = find_inner_tube_home_section_list(response)
    contents = section_list.get("contents") if isinstance(section_list.get("contents"), list) else []
    output: list[dict[str, Any]] = []
    for index, content in enumerate(contents[:max_sections]):
        if not isinstance(content, dict):
            continue
        carousel = content.get("musicCarouselShelfRenderer")
        if not isinstance(carousel, dict):
            output.append({"index": index, "renderer": next(iter(content.keys()), "unknown"), "items": []})
            continue
        header = carousel.get("header") if isinstance(carousel.get("header"), dict) else {}
        basic = header.get("musicCarouselShelfBasicHeaderRenderer") if isinstance(header.get("musicCarouselShelfBasicHeaderRenderer"), dict) else {}
        title = _text(basic.get("title")) or ""
        items = []
        for item_index, item in enumerate((carousel.get("contents") or [])[:max_items]):
            renderer_name = next((key for key in item.keys() if key.endswith("Renderer")), "unknown") if isinstance(item, dict) else "unknown"
            renderer = item.get(renderer_name) if isinstance(item, dict) else None
            if not isinstance(renderer, dict):
                items.append({"index": item_index, "renderer": renderer_name})
                continue
            items.append(
                {
                    "index": item_index,
                    "renderer": renderer_name,
                    "hash": digest(renderer, 12),
                    "title": _text(renderer.get("title")) or _text(renderer),
                }
            )
        output.append({"index": index, "title": title, "items": items})
    return output


def structured_section_summaries(page: dict[str, Any], max_sections: int, max_items: int) -> list[dict[str, Any]]:
    sections = page.get("sections") if isinstance(page.get("sections"), list) else []
    output: list[dict[str, Any]] = []
    for section_index, section in enumerate(sections[:max_sections]):
        if not isinstance(section, dict):
            continue
        items = []
        for item_index, item in enumerate((section.get("items") or [])[:max_items]):
            if not isinstance(item, dict):
                continue
            items.append(
                {
                    "index": item_index,
                    "type": item.get("result_type", ""),
                    "title": item.get("title", ""),
                    "artist": item.get("artist", ""),
                    "video_id": item.get("video_id", ""),
                    "browse_id": item.get("browse_id", ""),
                    "thumbnail_hash": hashlib.sha256(str(item.get("thumbnail_url") or "").encode("utf-8")).hexdigest()[:12],
                    "thumbnail_url": item.get("thumbnail_url", ""),
                }
            )
        output.append(
            {
                "index": section_index,
                "id": section.get("id", ""),
                "title": section.get("title", ""),
                "layout": section.get("layout", ""),
                "items": items,
            }
        )
    return output


def emit_json_block(label: str, value: Any) -> None:
    print(f"{PREFIX}_JSON_BEGIN {label}", file=sys.stderr)
    print(json.dumps(value, ensure_ascii=False, indent=2), file=sys.stderr)
    print(f"{PREFIX}_JSON_END {label}", file=sys.stderr)


def probe_once(index: int, args: argparse.Namespace) -> dict[str, Any]:
    continuation = str(args.continuation or "").strip()
    params = str(args.params or "").strip()
    offset = max(0, int(continuation or 0))
    section_limit = max(1, min(12, int(args.section_limit or 6)))
    fetch_limit = max(12, min(36, offset + section_limit + 1))
    cache_key = _feed_cache_key("home", continuation, section_limit, params)

    before_cache = cache_state(cache_key)
    log(
        "probe_start",
        run=index,
        continuation=continuation,
        params_hash=hashlib.sha1(params.encode("utf-8")).hexdigest()[:12] if params else "root",
        section_limit=section_limit,
        fetch_limit=fetch_limit,
        cache_key=cache_key,
        cache_exists=before_cache["exists"],
        cache_entry_exists=before_cache["entry_exists"],
        cache_entry_age_seconds=before_cache["entry_age_seconds"],
        cache_entry_page_hash=before_cache["entry_page_hash"],
    )

    client = _create_client(authenticated=True)

    body, raw_response = _inner_tube_home_response(client, params)
    raw_hash = digest(raw_response)
    section_list = find_inner_tube_home_section_list(raw_response)
    raw_contents = section_list.get("contents") if isinstance(section_list.get("contents"), list) else []
    raw_continuations = section_list.get("continuations") if isinstance(section_list.get("continuations"), list) else []
    chips = extract_inner_tube_home_chips(raw_response)
    log(
        "raw_browse_response",
        run=index,
        request_body_hash=digest(body),
        raw_hash=raw_hash,
        raw_sections=len(raw_contents),
        raw_continuations=len(raw_continuations),
        chip_count=len(chips),
        first_sections=" | ".join(_text(section.get("title")) for section in raw_section_summaries(raw_response, args.max_sections, 0)),
    )
    emit_json_block(f"raw_sections_run_{index}", raw_section_summaries(raw_response, args.max_sections, args.max_items))

    rows, raw_response_from_rows = _inner_tube_home_rows(client, params, fetch_limit)
    rows_hash = digest(rows)
    raw_rows_hash = digest(raw_response_from_rows)
    log(
        "parsed_rows",
        run=index,
        rows_hash=rows_hash,
        raw_response_hash_from_rows=raw_rows_hash,
        rows=len(rows),
        row_titles=" | ".join(_text(row.get("title")) for row in rows[: args.max_sections] if isinstance(row, dict)),
    )

    page = build_structured_home(
        rows,
        offset=offset,
        section_limit=section_limit,
        selected_chip_params=params,
    )
    if chips:
        page["chips"] = chips
    page_hash = digest(page)
    log(
        "structured_page",
        run=index,
        page_hash=page_hash,
        page_sections=len(page.get("sections") or []),
        page_continuation=page.get("continuation", ""),
        generated_at=page.get("generated_at", 0),
    )
    emit_json_block(f"structured_sections_run_{index}", structured_section_summaries(page, args.max_sections, args.max_items))

    if args.compare_ytmusicapi:
        try:
            home_rows = client.get_home(limit=fetch_limit)
            log(
                "ytmusicapi_get_home",
                run=index,
                ytmusicapi_hash=digest(home_rows),
                ytmusicapi_rows=len(home_rows or []),
                ytmusicapi_titles=" | ".join(_text(row.get("title")) for row in (home_rows or [])[: args.max_sections] if isinstance(row, dict)),
            )
        except Exception as error:
            log("ytmusicapi_get_home_failed", run=index, error=error)

    return {
        "run": index,
        "raw_hash": raw_hash,
        "rows_hash": rows_hash,
        "page_hash": page_hash,
        "sections": [section.get("title", "") for section in page.get("sections") or [] if isinstance(section, dict)],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Trace Nocky YouTube Home source freshness.")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--sleep", type=float, default=2.0)
    parser.add_argument("--continuation", default="")
    parser.add_argument("--params", default="")
    parser.add_argument("--section-limit", type=int, default=12)
    parser.add_argument("--max-sections", type=int, default=12)
    parser.add_argument("--max-items", type=int, default=6)
    parser.add_argument("--compare-ytmusicapi", action="store_true")
    args = parser.parse_args()

    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")

    summaries = []
    runs = max(1, args.runs)
    for index in range(1, runs + 1):
        summaries.append(probe_once(index, args))
        if index < runs and args.sleep > 0:
            time.sleep(args.sleep)

    log(
        "probe_summary",
        runs=len(summaries),
        raw_hashes=" | ".join(summary["raw_hash"] for summary in summaries),
        rows_hashes=" | ".join(summary["rows_hash"] for summary in summaries),
        page_hashes=" | ".join(summary["page_hash"] for summary in summaries),
        section_sets=" || ".join(" | ".join(summary["sections"]) for summary in summaries),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
