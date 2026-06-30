#!/usr/bin/env python3
"""Sanitized diagnostics for real YouTube Music Home renderer payloads."""

from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit, urlunsplit


SENSITIVE_KEYS = {
    "trackingParams",
    "clickTrackingParams",
    "continuation",
    "continuations",
    "continuationCommand",
    "continuationEndpoint",
    "params",
    "visitorData",
    "responseContext",
    "frameworkUpdates",
    "serviceTrackingParams",
    "adSignalsInfo",
    "loggingDirectives",
    "serializedShareEntity",
}


def _text(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, dict):
        for key in ("text", "simpleText", "content", "title", "name", "label"):
            text = _text(value.get(key))
            if text:
                return text
        runs = value.get("runs")
        if isinstance(runs, list):
            return "".join(_text(run) for run in runs if isinstance(run, dict)).strip()
    return ""


def _sanitize_url(value: str) -> str:
    try:
        parts = urlsplit(value)
    except ValueError:
        return value[:1000]
    if not parts.scheme or not parts.netloc:
        return value[:1000]
    return urlunsplit((parts.scheme, parts.netloc, parts.path, "", ""))


def sanitize_home_payload(value: Any) -> Any:
    """Remove tracking/session-shaped fields while preserving renderer structure."""

    if isinstance(value, dict):
        output: dict[str, Any] = {}
        for key, child in value.items():
            if key in SENSITIVE_KEYS:
                continue
            if key == "url" and isinstance(child, str):
                output[key] = _sanitize_url(child)
                continue
            output[key] = sanitize_home_payload(child)
        return output
    if isinstance(value, list):
        return [sanitize_home_payload(child) for child in value]
    if isinstance(value, str):
        return value if len(value) <= 2000 else value[:2000] + "…"
    return value


def _walk(value: Any, path: tuple[str, ...] = ()):
    if isinstance(value, dict):
        yield path, value
        for key, child in value.items():
            yield from _walk(child, path + (str(key),))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from _walk(child, path + (str(index),))


def _renderer_counts(pages: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for page in pages:
        response = page.get("response")
        for _path, node in _walk(response):
            for key, child in node.items():
                if not isinstance(child, dict):
                    continue
                if key.endswith("Renderer") or key.endswith("ViewModel"):
                    counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items(), key=lambda item: (-item[1], item[0])))


def _thumbnail_paths(pages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    output: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for page_index, page in enumerate(pages):
        response = page.get("response")
        for path, node in _walk(response):
            url = node.get("url") if isinstance(node, dict) else None
            if not isinstance(url, str) or not url.strip():
                continue
            path_text = ".".join(path)
            context = path_text.casefold()
            if not any(token in context for token in ("thumbnail", "image", "avatar", "source")):
                continue
            cleaned = _sanitize_url(url)
            key = (path_text, cleaned)
            if key in seen:
                continue
            seen.add(key)
            output.append(
                {
                    "page": page_index,
                    "path": path_text,
                    "url": cleaned,
                    "width": node.get("width"),
                    "height": node.get("height"),
                }
            )
    return output


def _parsed_summary(rows: Any) -> list[dict[str, Any]]:
    sections: list[dict[str, Any]] = []
    for row in rows or []:
        if not isinstance(row, dict):
            continue
        items = []
        for item in row.get("contents") or []:
            if not isinstance(item, dict):
                continue
            thumbnails = item.get("thumbnails") or item.get("rawRendererThumbnails") or []
            urls = [
                _sanitize_url(candidate.get("url", ""))
                for candidate in thumbnails
                if isinstance(candidate, dict) and candidate.get("url")
            ]
            items.append(
                {
                    "title": _text(item.get("title")),
                    "resultType": _text(item.get("resultType") or item.get("result_type")),
                    "rendererType": _text(item.get("rendererType")),
                    "videoId": _text(item.get("videoId") or item.get("video_id")),
                    "browseId": _text(item.get("browseId") or item.get("browse_id")),
                    "playlistId": _text(item.get("playlistId") or item.get("playlist_id")),
                    "thumbnailUrls": urls,
                }
            )
        sections.append({"title": _text(row.get("title")), "items": items})
    return sections


def write_home_debug_dump(
    destination: str | Path,
    *,
    pages: list[dict[str, Any]],
    rows: Any,
    selected_params: str = "",
) -> Path:
    """Write a sanitized renderer dump suitable for attaching to a bug report."""

    path = Path(destination).expanduser()
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "version": 1,
        "generatedAt": int(time.time()),
        "selectedChip": bool(selected_params),
        "rendererCounts": _renderer_counts(pages),
        "thumbnailPaths": _thumbnail_paths(pages),
        "parsedSections": _parsed_summary(rows),
        "pages": [
            {
                "kind": _text(page.get("kind")) or "unknown",
                "response": sanitize_home_payload(page.get("response")),
            }
            for page in pages
        ],
    }
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    temporary.replace(path)
    return path
