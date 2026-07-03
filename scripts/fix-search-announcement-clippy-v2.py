#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"

if not BROWSER.is_file():
    raise SystemExit("Run this script from the Nocky repository root.")

text = BROWSER.read_text(encoding="utf-8")

if "struct SearchResultCounts" in text:
    print("Search announcement helpers already use SearchResultCounts; no clippy expectation needed.")
    raise SystemExit(0)

changes = 0
replacements = [
    (
        "fn search_results_announcement(\n",
        '''#[expect(
    clippy::too_many_arguments,
    reason = "Search announcement copy keeps localized category counts explicit"
)]
fn search_results_announcement(\n''',
    ),
    (
        "fn update_search_results_accessible_summary(\n",
        '''#[expect(
    clippy::too_many_arguments,
    reason = "Search accessible summary mirrors the categorized result counts"
)]
fn update_search_results_accessible_summary(\n''',
    ),
]

for old, new in replacements:
    if new in text:
        continue
    if old not in text:
        raise SystemExit(f"Could not find helper signature: {old.strip()} No files were written.")
    text = text.replace(old, new, 1)
    changes += 1

if changes == 0:
    print("Search announcement helper clippy expectations were already present.")
else:
    BROWSER.write_text(text, encoding="utf-8")
    print("Search announcement helper clippy expectations added successfully.")
