from __future__ import annotations

import hashlib
import os
import re
from pathlib import Path
import urllib.request

import argparse
import json
import sys
from typing import Any


VIDEO_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]{11}$")


def build(
    response: dict[str, Any],
    *,
    selected_chip_params: str = "",
    section_limit: int = 6,
) -> dict[str, Any]:
    """Build the native Home V3 source payload from a YouTube Music browse response.

    The output shape intentionally matches the Rust HomeV3SourcePage contract.
    Missing or unknown structures are ignored instead of falling back to Home V2.
    """

    sections = []
    for renderer, layout in _section_renderers(response):
        section = _section_from_renderer(renderer, layout)
        if section["items"]:
            sections.append(section)
        if len(sections) >= section_limit:
            break

    page = {
        "version": 3,
        "selected_chip_params": selected_chip_params,
        "sections": sections,
        "chips": _home_v3_chips(response),
        "continuation": _continuation_from_response(response),
    }
    return _attach_home_v3_cover_paths(page)



def _home_v3_cache_dir() -> Path:
    root = Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache")
    path = root / "nocky" / "youtube" / "home-v3-covers"
    path.mkdir(parents=True, exist_ok=True)
    return path


def _home_v3_cover_cacheable(url: str) -> bool:
    lowered = url.lower()
    return (
        lowered.startswith("https://")
        and (
            "ytimg.com" in lowered
            or "googleusercontent.com" in lowered
            or "ggpht.com" in lowered
        )
    )


def _home_v3_cached_cover_path(url: str) -> str:
    url = (url or "").strip()
    if not _home_v3_cover_cacheable(url):
        return ""

    digest = hashlib.sha1(url.encode("utf-8")).hexdigest()[:24]
    path = _home_v3_cache_dir() / f"{digest}.cover"

    if path.is_file() and path.stat().st_size > 0:
        return str(path)

    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": "Mozilla/5.0",
            "Accept": "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
        },
    )

    try:
        with urllib.request.urlopen(request, timeout=8) as response:
            data = response.read(4 * 1024 * 1024)
    except Exception:
        return ""

    if not data:
        return ""

    temporary = path.with_suffix(".tmp")
    temporary.write_bytes(data)
    temporary.replace(path)
    return str(path)


def _attach_home_v3_cover_paths(page: dict[str, Any], *, limit: int = 48) -> dict[str, Any]:
    remaining = max(0, limit)

    for section in page.get("sections") or []:
        if not isinstance(section, dict):
            continue

        for item in section.get("items") or []:
            if not isinstance(item, dict):
                continue

            if item.get("cover_path"):
                continue

            thumbnail_url = _str(item.get("thumbnail_url"))
            if not thumbnail_url:
                continue

            if remaining <= 0:
                return page

            remaining -= 1
            item["cover_path"] = _home_v3_cached_cover_path(thumbnail_url)

    return page



def _section_renderers(value: Any):
    for node in _walk(value):
        if not isinstance(node, dict):
            continue
        renderer = node.get("musicCarouselShelfRenderer")
        if isinstance(renderer, dict):
            yield renderer, "carousel"
        renderer = node.get("musicShelfRenderer")
        if isinstance(renderer, dict):
            yield renderer, "list"


def _section_from_renderer(renderer: dict[str, Any], layout: str) -> dict[str, Any]:
    items = []
    seen = set()

    for item_renderer in _item_renderers(renderer):
        item = _item_from_renderer(item_renderer)
        key = _item_key(item)
        if not key or key in seen:
            continue
        seen.add(key)
        items.append(item)

    return {
        "title": _section_title(renderer),
        "layout": layout,
        "items": items,
    }


def _item_renderers(value: Any):
    names = (
        "musicTwoRowItemRenderer",
        "musicResponsiveListItemRenderer",
        "musicMultiRowListItemRenderer",
    )

    for node in _walk(value):
        if not isinstance(node, dict):
            continue
        for name in names:
            renderer = node.get(name)
            if isinstance(renderer, dict):
                yield renderer


def _first_watch_endpoint(value: Any) -> dict[str, Any]:
    direct = _dig(value, "navigationEndpoint", "watchEndpoint")
    if isinstance(direct, dict):
        return direct

    for node in _walk(value):
        if not isinstance(node, dict):
            continue
        watch = node.get("watchEndpoint")
        if isinstance(watch, dict):
            return watch

    return {}


def _first_browse_endpoint(value: Any) -> dict[str, Any]:
    direct = _dig(value, "navigationEndpoint", "browseEndpoint")
    if isinstance(direct, dict):
        return direct

    for node in _walk(value):
        if not isinstance(node, dict):
            continue
        browse = node.get("browseEndpoint")
        if isinstance(browse, dict):
            return browse

    return {}


def _video_id_from_renderer(value: Any) -> str:
    for node in _walk(value):
        if not isinstance(node, dict):
            continue

        for key in ("videoId", "video_id"):
            candidate = _str(node.get(key)).strip()
            if VIDEO_ID_PATTERN.fullmatch(candidate):
                return candidate

    return ""


def _item_from_renderer(renderer: dict[str, Any]) -> dict[str, Any]:
    watch = _first_watch_endpoint(renderer)
    browse = _first_browse_endpoint(renderer)

    video_id = _str(watch.get("videoId")).strip()
    if not video_id:
        video_id = _video_id_from_renderer(renderer)

    browse_id = _str(browse.get("browseId"))
    params = _str(browse.get("params"))

    title = _item_title(renderer)
    subtitle = _item_subtitle(renderer)
    album, artist = _album_artist_from_subtitle(subtitle)

    result_type = _result_type(video_id, browse_id, renderer)

    return {
        "result_type": result_type,
        "title": title,
        "subtitle": subtitle,
        "video_id": video_id,
        "browse_id": browse_id,
        "album": album,
        "artist": artist,
        "playlist_kind": "",
        "params": params,
        "duration_seconds": 0,
        "thumbnail_url": _thumbnail_url(renderer, prefer_square=result_type == "song"),
        "cover_path": "",
    }


def _item_title(renderer: dict[str, Any]) -> str:
    direct = _text(renderer.get("title"))
    if direct:
        return direct

    columns = renderer.get("flexColumns")
    if isinstance(columns, list):
        for column in columns:
            column_renderer = _dig(column, "musicResponsiveListItemFlexColumnRenderer")
            text = _text(_dig(column_renderer, "text"))
            if text:
                return text

    return ""


def _item_subtitle(renderer: dict[str, Any]) -> str:
    direct = _text(renderer.get("subtitle"))
    if direct:
        return direct

    columns = renderer.get("flexColumns")
    if isinstance(columns, list):
        texts = []
        for column in columns[1:]:
            column_renderer = _dig(column, "musicResponsiveListItemFlexColumnRenderer")
            text = _text(_dig(column_renderer, "text"))
            if text:
                texts.append(text)
        if texts:
            return " • ".join(texts)

    runs = []
    for run in _runs(renderer):
        text = _str(run.get("text")).strip()
        if text and text != _item_title(renderer):
            runs.append(text)

    return " • ".join(_dedupe(runs[:4]))


def _album_artist_from_subtitle(subtitle: str) -> tuple[str, str]:
    parts = [part.strip() for part in subtitle.split("•") if part.strip()]
    if not parts:
        return "", ""
    if len(parts) == 1:
        return "", parts[0]
    return parts[-1], parts[0]


def _section_title(renderer: dict[str, Any]) -> str:
    candidates = (
        _dig(renderer, "header", "musicCarouselShelfBasicHeaderRenderer", "title"),
        _dig(renderer, "header", "musicShelfBasicHeaderRenderer", "title"),
        renderer.get("title"),
    )

    for candidate in candidates:
        text = _text(candidate)
        if text:
            return text

    return ""



def _home_v3_chips(response: dict[str, Any]) -> list[dict[str, str]]:
    chips = [{"title": "Início", "params": ""}]
    seen = {("", "")}

    for chip in _chips_from_response(response):
        title = _str(chip.get("title")).strip()
        params = _str(chip.get("params"))
        key = (title, params)

        if not title or key in seen:
            continue

        seen.add(key)
        chips.append({"title": title, "params": params})

    return chips


def _chips_from_response(response: dict[str, Any]) -> list[dict[str, str]]:
    chips = []
    seen = set()

    for node in _walk(response):
        if not isinstance(node, dict):
            continue

        renderer = node.get("chipCloudChipRenderer")
        if not isinstance(renderer, dict):
            continue

        title = _text(renderer.get("text"))
        endpoint = _first_endpoint(renderer)
        browse = endpoint.get("browseEndpoint", {}) if isinstance(endpoint, dict) else {}
        params = _str(browse.get("params"))

        key = (title, params)
        if not title or key in seen:
            continue

        seen.add(key)
        chips.append({"title": title, "params": params})

    return chips


def _continuation_from_response(response: dict[str, Any]) -> str:
    continuation_keys = (
        "nextContinuationData",
        "reloadContinuationData",
        "continuationCommand",
    )

    for node in _walk(response):
        if not isinstance(node, dict):
            continue

        for key in continuation_keys:
            candidate = node.get(key)
            if not isinstance(candidate, dict):
                continue

            token = _str(candidate.get("continuation") or candidate.get("token"))
            if token:
                return token

    return ""


def _result_type(video_id: str, browse_id: str, renderer: dict[str, Any]) -> str:
    if video_id:
        return "song"

    lowered = str(renderer).lower()
    if browse_id.startswith("VL"):
        return "playlist"
    if browse_id.startswith("MPRE"):
        return "album"
    if browse_id.startswith("UC") or "artist" in lowered:
        return "artist"
    if browse_id:
        return "collection"

    return ""


def _thumbnail_url(renderer: dict[str, Any], *, prefer_square: bool = False) -> str:
    # Prefer the explicit YouTube Music artwork node. A blind walk can pick
    # nested watch/video thumbnails before the album/song artwork.
    candidates = (
        _dig(renderer, "thumbnailRenderer", "musicThumbnailRenderer", "thumbnail"),
        _dig(renderer, "thumbnail", "musicThumbnailRenderer", "thumbnail"),
        _dig(renderer, "thumbnail"),
    )

    for candidate in candidates:
        url = _thumbnail_from_node(candidate, prefer_square=prefer_square)
        if url:
            return url

    for node in _walk(renderer):
        url = _thumbnail_from_node(node, prefer_square=prefer_square)
        if url:
            return url

    return ""


def _thumbnail_from_node(value: Any, *, prefer_square: bool = False) -> str:
    if not isinstance(value, dict):
        return ""

    thumbnails = value.get("thumbnails")
    if not isinstance(thumbnails, list) or not thumbnails:
        return ""

    valid = [thumbnail for thumbnail in thumbnails if isinstance(thumbnail, dict) and _str(thumbnail.get("url"))]
    if not valid:
        return ""

    if prefer_square:
        squareish = []
        for thumbnail in valid:
            width = thumbnail.get("width")
            height = thumbnail.get("height")
            if isinstance(width, int) and isinstance(height, int) and width > 0 and height > 0:
                ratio = width / height
                if 0.85 <= ratio <= 1.18:
                    squareish.append(thumbnail)

        if squareish:
            return _str(squareish[-1].get("url"))

    return _str(valid[-1].get("url"))


def _first_endpoint(value: Any) -> dict[str, Any]:
    direct = _dig(value, "navigationEndpoint")
    if isinstance(direct, dict) and (
        isinstance(direct.get("watchEndpoint"), dict)
        or isinstance(direct.get("browseEndpoint"), dict)
    ):
        return direct

    for node in _walk(value):
        if not isinstance(node, dict):
            continue
        if isinstance(node.get("watchEndpoint"), dict) or isinstance(node.get("browseEndpoint"), dict):
            return node

    return {}


def _item_key(item: dict[str, Any]) -> str:
    for key in ("video_id", "browse_id", "params", "title"):
        value = _str(item.get(key))
        if value:
            return f"{key}:{value}"
    return ""


def _text(value: Any) -> str:
    if isinstance(value, str):
        return value.strip()

    if isinstance(value, list):
        return " ".join(part for part in (_text(item) for item in value) if part).strip()

    if not isinstance(value, dict):
        return ""

    simple = _str(value.get("simpleText"))
    if simple:
        return simple.strip()

    text = _str(value.get("text"))
    if text:
        return text.strip()

    runs = value.get("runs")
    if isinstance(runs, list):
        return "".join(_str(run.get("text")) for run in runs if isinstance(run, dict)).strip()

    accessibility = _dig(value, "accessibility", "accessibilityData", "label")
    if isinstance(accessibility, str):
        return accessibility.strip()

    return ""


def _runs(value: Any):
    for node in _walk(value):
        if isinstance(node, dict):
            runs = node.get("runs")
            if isinstance(runs, list):
                for run in runs:
                    if isinstance(run, dict):
                        yield run


def _walk(value: Any):
    yield value

    if isinstance(value, dict):
        for child in value.values():
            yield from _walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk(child)


def _dig(value: Any, *keys: str) -> Any:
    current = value
    for key in keys:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _str(value: Any) -> str:
    return value if isinstance(value, str) else ""


def _dedupe(values: list[str]) -> list[str]:
    seen = set()
    result = []

    for value in values:
        normalized = value.strip()
        if not normalized or normalized in seen:
            continue
        seen.add(normalized)
        result.append(normalized)

    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build a native Nocky Home V3 payload.")
    parser.add_argument("--selected-chip-params", default="")
    parser.add_argument("--section-limit", type=int, default=6)
    args = parser.parse_args(argv)

    try:
        response = json.load(sys.stdin)
        result = build(
            response,
            selected_chip_params=args.selected_chip_params,
            section_limit=args.section_limit,
        )
        print(json.dumps({"ok": True, "result": result, "error": None}, ensure_ascii=False))
        return 0
    except Exception as error:
        print(
            json.dumps(
                {"ok": False, "result": None, "error": str(error)},
                ensure_ascii=False,
            )
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
