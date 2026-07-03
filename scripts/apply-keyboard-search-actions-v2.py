#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-keyboard-search-actions.py")
text = SOURCE.read_text(encoding="utf-8")

old = '''        "fn search_result_artwork(\n",
        SEARCH_SECTION,
        "Keyboard-first search collection rows",
'''
new = '''        "fn search_result_artwork(",
        SEARCH_SECTION,
        "Keyboard-first search collection rows",
'''

count = text.count(old)
if count != 1:
    raise SystemExit(
        f"Expected one search artwork boundary in {SOURCE}, found {count}."
    )

text = text.replace(old, new, 1)
namespace = {
    "__name__": "__main__",
    "__file__": str(SOURCE),
}
exec(compile(text, str(SOURCE), "exec"), namespace)
