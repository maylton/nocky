#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-search-pagination.py")
namespace = {
    "__name__": "apply_search_pagination",
    "__file__": str(SOURCE),
}
exec(compile(SOURCE.read_text(encoding="utf-8"), str(SOURCE), "exec"), namespace)

original_patch_browser = namespace["patch_browser"]
original_replace_once = namespace["replace_once"]
search_list_section = namespace["SEARCH_LIST_SECTION"]
remote_more_button = namespace["REMOTE_MORE_BUTTON"]


def patch_browser_v2(text: str) -> str:
    def replace_once_without_early_button(
        current: str,
        old: str,
        new: str,
        label: str,
    ) -> str:
        if label == "Add remote pagination button":
            return current
        return original_replace_once(current, old, new, label)

    namespace["replace_once"] = replace_once_without_early_button
    try:
        updated = original_patch_browser(text)
    finally:
        namespace["replace_once"] = original_replace_once

    return original_replace_once(
        updated,
        search_list_section,
        remote_more_button + search_list_section,
        "Add remote pagination button",
    )


namespace["patch_browser"] = patch_browser_v2
raise SystemExit(namespace["main"]())
