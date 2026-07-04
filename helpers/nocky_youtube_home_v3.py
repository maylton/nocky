from __future__ import annotations

from typing import Any


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

    return {
        "version": 3,
        "selected_chip_params": selected_chip_params,
        "sections": sections,
        "chips": _chips_from_response(response),
        "continuation": _continuation_from_response(response),
    }


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


def _item_from_renderer(renderer: dict[str, Any]) -> dict[str, Any]:
    endpoint = _first_endpoint(renderer)
    watch = endpoint.get("watchEndpoint", {}) if isinstance(endpoint, dict) else {}
    browse = endpoint.get("browseEndpoint", {}) if isinstance(endpoint, dict) else {}

    video_id = _str(watch.get("videoId"))
    browse_id = _str(browse.get("browseId"))
    params = _str(browse.get("params"))

    title = _item_title(renderer)
    subtitle = _item_subtitle(renderer)
    album, artist = _album_artist_from_subtitle(subtitle)

    return {
        "result_type": _result_type(video_id, browse_id, renderer),
        "title": title,
        "subtitle": subtitle,
        "video_id": video_id,
        "browse_id": browse_id,
        "album": album,
        "artist": artist,
        "playlist_kind": "",
        "params": params,
        "duration_seconds": 0,
        "thumbnail_url": _thumbnail_url(renderer),
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


def _thumbnail_url(renderer: dict[str, Any]) -> str:
    for node in _walk(renderer):
        if not isinstance(node, dict):
            continue

        thumbnails = node.get("thumbnails")
        if not isinstance(thumbnails, list) or not thumbnails:
            continue

        urls = [_str(thumbnail.get("url")) for thumbnail in thumbnails if isinstance(thumbnail, dict)]
        urls = [url for url in urls if url]
        if urls:
            return urls[-1]

    return ""


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
