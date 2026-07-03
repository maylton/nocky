#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-keyboard-search-actions.py")
text = SOURCE.read_text(encoding="utf-8")

old = r'"fn search_result_artwork(\n",'
new = '"fn search_result_artwork(",'

if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit(
        f"Could not find either the old or corrected search artwork boundary in {SOURCE}."
    )

namespace = {
    "__name__": "__main__",
    "__file__": str(SOURCE),
}
exec(compile(text, str(SOURCE), "exec"), namespace)
