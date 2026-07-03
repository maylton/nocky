#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"

if not BROWSER.is_file():
    raise SystemExit("Run this script from the Nocky repository root.")

text = BROWSER.read_text(encoding="utf-8")
old = '''fn update_search_results_accessible_summary(
    widget: &impl IsA<gtk::Widget>,
    language: AppLanguage,
'''
new = '''fn update_search_results_accessible_summary(
    widget: &gtk::Box,
    language: AppLanguage,
'''

if old in text:
    text = text.replace(old, new, 1)
    BROWSER.write_text(text, encoding="utf-8")
    print("Search result announcement target made concrete successfully.")
elif new in text:
    print("Search result announcement target is already concrete.")
else:
    raise SystemExit(
        "Could not find the search announcement helper signature. No files were written."
    )
