"""Privacy-safe parsing for YouTube Music account/profile discovery.

The parser accepts a raw ``account/accounts_list`` response and emits only a
small allowlisted contract. It never returns cookies, headers, sign-in URLs,
tracking parameters, continuation tokens, or opaque service-endpoint payloads.
"""

from __future__ import annotations

import re
from typing import Any, Iterator
from urllib.parse import urlparse

_BRAND_ID_RE = re.compile(r"^[0-9]{10,30}$")


def _text(value: Any) -> str:
    if isinstance(value, str):
        return value.strip()
    if not isinstance(value, dict):
        return ""

    content = value.get("content")
    if isinstance(content, str) and content.strip():
        return content.strip()

    runs = value.get("runs")
    if not isinstance(runs, list):
        return ""
    return "".join(
        str(run.get("text") or "")
        for run in runs
        if isinstance(run, dict)
    ).strip()


def _https_url(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    candidate = value.strip()
    try:
        parsed = urlparse(candidate)
    except ValueError:
        return ""
    if parsed.scheme != "https" or not parsed.netloc:
        return ""
    return candidate


def _photo_url(item: dict[str, Any]) -> str:
    photo = item.get("accountPhoto")
    if not isinstance(photo, dict):
        return ""
    thumbnails = photo.get("thumbnails")
    if not isinstance(thumbnails, list):
        return ""

    for thumbnail in reversed(thumbnails):
        if isinstance(thumbnail, dict):
            url = _https_url(thumbnail.get("url"))
            if url:
                return url
    return ""


def _identity_selector(item: dict[str, Any]) -> dict[str, Any] | None:
    endpoint = item.get("serviceEndpoint")
    if not isinstance(endpoint, dict):
        return None
    selector = endpoint.get("selectActiveIdentityEndpoint")
    return selector if isinstance(selector, dict) else None


def _brand_id(item: dict[str, Any]) -> str:
    selector = _identity_selector(item)
    if selector is None:
        return ""
    tokens = selector.get("supportedTokens")
    if not isinstance(tokens, list):
        return ""

    for token in tokens:
        if not isinstance(token, dict):
            continue
        page_token = token.get("pageIdToken")
        if not isinstance(page_token, dict):
            continue
        page_id = str(page_token.get("pageId") or "").strip()
        if _BRAND_ID_RE.fullmatch(page_id):
            return page_id
    return ""


def _renderers(value: Any, renderer_name: str) -> Iterator[dict[str, Any]]:
    if isinstance(value, dict):
        renderer = value.get(renderer_name)
        if isinstance(renderer, dict):
            yield renderer
        for nested in value.values():
            yield from _renderers(nested, renderer_name)
    elif isinstance(value, list):
        for nested in value:
            yield from _renderers(nested, renderer_name)


def _account_item(wrapper: Any) -> dict[str, Any] | None:
    if not isinstance(wrapper, dict):
        return None
    for key in ("accountItem", "accountItemRenderer"):
        item = wrapper.get(key)
        if isinstance(item, dict):
            return item
    return None


def discover_account_profiles(payload: Any) -> dict[str, Any]:
    """Return a deterministic, display-only account discovery contract."""

    parsed: list[dict[str, Any]] = []
    seen_brand_ids: set[str] = set()
    seen_fallbacks: set[tuple[str, str]] = set()

    for renderer in _renderers(payload, "accountItemSectionRenderer"):
        contents = renderer.get("contents")
        if not isinstance(contents, list):
            continue

        for wrapper in contents:
            item = _account_item(wrapper)
            if item is None:
                continue

            name = _text(item.get("accountName"))
            if not name:
                continue
            handle = _text(item.get("channelHandle"))
            brand_id = _brand_id(item)
            has_selector = _identity_selector(item) is not None

            if brand_id:
                if brand_id in seen_brand_ids:
                    continue
                seen_brand_ids.add(brand_id)
            else:
                fallback = (name.casefold(), handle.casefold())
                if fallback in seen_fallbacks:
                    continue
                seen_fallbacks.add(fallback)

            parsed.append(
                {
                    "profile_id": brand_id,
                    "name": name,
                    "channel_handle": handle,
                    "photo_url": _photo_url(item),
                    "kind": "brand" if brand_id else "unknown",
                    "is_selected": bool(item.get("isSelected")),
                    "switchable": bool(brand_id),
                    "_has_identity_selector": has_selector,
                }
            )

    primary_candidates = [
        profile
        for profile in parsed
        if not profile["profile_id"] and not profile["_has_identity_selector"]
    ]
    if len(primary_candidates) == 1:
        primary = primary_candidates[0]
        primary["profile_id"] = "primary"
        primary["kind"] = "primary"
        primary["switchable"] = True

    for profile in parsed:
        profile.pop("_has_identity_selector", None)

    profile_ids = [profile["profile_id"] for profile in parsed]
    selected_count = sum(bool(profile["is_selected"]) for profile in parsed)
    deterministic = bool(parsed) and all(profile_ids) and len(set(profile_ids)) == len(profile_ids)
    if len(parsed) > 1:
        deterministic = deterministic and selected_count == 1

    if not parsed:
        state = "unavailable"
    elif len(parsed) == 1 and deterministic:
        state = "single"
    elif deterministic:
        state = "multiple"
    else:
        state = "ambiguous"

    return {
        "state": state,
        "deterministic": deterministic,
        "profiles": parsed,
    }
