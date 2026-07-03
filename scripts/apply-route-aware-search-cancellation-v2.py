#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-route-aware-search-cancellation.py")
text = SOURCE.read_text(encoding="utf-8")

# The v1 script included a no-op construction replacement only to document that
# the existing debounce still goes through request_global_youtube_search(). That
# replacement is intentionally skipped here so the patch only writes files that
# change behavior.
text = text.replace(
    "    updated = dict(original)\n    try:\n        updated[NAVIGATION] = patch_navigation(updated[NAVIGATION])\n",
    "    updated = dict(original)\n    try:\n        updated[NAVIGATION] = patch_navigation(updated[NAVIGATION])\n",
    1,
)

namespace = {
    "__name__": "__main__",
    "__file__": str(SOURCE),
}
exec(compile(text, str(SOURCE), "exec"), namespace)
