#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
MAIN = ROOT / "src/main.rs"
BROWSER = ROOT / "src/browser.rs"
RANKING = ROOT / "src/search_ranking.rs"
DOC = ROOT / "docs/SEARCH_RANKING.md"

if not MAIN.is_file():
    raise SystemExit("Run this script from the Nocky repository root.")

changed: list[Path] = []
main = MAIN.read_text(encoding="utf-8")
new_main = main.replace("mod search_ranking;\n", "")
if new_main != main:
    MAIN.write_text(new_main, encoding="utf-8")
    changed.append(MAIN)

if BROWSER.is_file():
    browser = BROWSER.read_text(encoding="utf-8")
    if "search_ranking::{" in browser or "rank_search_document(" in browser:
        raise SystemExit(
            "Mixed-ranking logic is present in src/browser.rs, not just the module hook.\n"
            "Stop here and restore the accidental mixed-search patch before continuing:\n\n"
            "git restore src/browser.rs\n\n"
            "Then rerun this script. No files were written beyond src/main.rs if it needed cleanup."
        )

for path in [RANKING, DOC]:
    if path.exists():
        path.unlink()
        changed.append(path)

if changed:
    print("Removed accidental mixed-search ranking remnants:")
    for path in changed:
        print(f"  {path.relative_to(ROOT)}")
else:
    print("No accidental mixed-search ranking remnants found.")
