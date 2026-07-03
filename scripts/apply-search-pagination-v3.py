#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-search-pagination.py")
text = SOURCE.read_text(encoding="utf-8")

old_guard = '''    if "search_result_primary_action" not in original[BROWSER]:
        print("ERROR: apply and validate the keyboard-first search actions patch first.", file=sys.stderr)
        return 1
'''
new_guard = '''    keyboard_markers = (
        "fn search_collection_row(",
        "fn install_row_keyboard_activation(",
        'add_css_class("search-result-primary-action")',
    )
    if not all(marker in original[BROWSER] for marker in keyboard_markers):
        print("ERROR: apply and validate the keyboard-first search actions patch first.", file=sys.stderr)
        return 1
'''

if old_guard not in text:
    raise SystemExit("Could not find the outdated keyboard-search prerequisite check.")
text = text.replace(old_guard, new_guard, 1)

namespace = {
    "__name__": "apply_search_pagination",
    "__file__": str(SOURCE),
}
exec(compile(text, str(SOURCE), "exec"), namespace)

original_patch_browser = namespace["patch_browser"]
original_replace_once = namespace["replace_once"]
search_list_section = namespace["SEARCH_LIST_SECTION"]
remote_more_button = namespace["REMOTE_MORE_BUTTON"]


def patch_browser_v3(current: str) -> str:
    def replace_once_without_early_button(
        source: str,
        old: str,
        new: str,
        label: str,
    ) -> str:
        if label == "Add remote pagination button":
            return source
        return original_replace_once(source, old, new, label)

    namespace["replace_once"] = replace_once_without_early_button
    try:
        updated = original_patch_browser(current)
    finally:
        namespace["replace_once"] = original_replace_once

    return original_replace_once(
        updated,
        search_list_section,
        remote_more_button + search_list_section,
        "Add remote pagination button",
    )


namespace["patch_browser"] = patch_browser_v3
raise SystemExit(namespace["main"]())
