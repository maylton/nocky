#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

path = Path("src/app/controller/construction.rs")
if not path.is_file():
    raise SystemExit(
        "Run this script from the Nocky repository root after applying the recent-search dropdown patches."
    )

text = path.read_text(encoding="utf-8")
old_setup = '''        search_history_revealer.set_hexpand(true);
        search_history_revealer.set_halign(gtk::Align::Fill);
        // Match the SearchEntry's left edge and reserve the SearchBar close
        // button area on the right.
        search_history_revealer.set_margin_start(12);
        search_history_revealer.set_margin_end(56);
        search_history_revealer.add_css_class("search-history-dropdown");
        search_bar.set_child(Some(&search_entry));
'''
new_setup = '''        search_history_revealer.set_hexpand(true);
        search_history_revealer.set_halign(gtk::Align::Fill);
        search_history_revealer.set_margin_start(12);
        search_history_revealer.set_margin_end(12);
        search_history_revealer.add_css_class("search-history-dropdown");

        // Keep the entry and its recent-query surface inside the same SearchBar
        // child. Both now share the exact content width before the close button.
        let search_surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
        search_surface.set_hexpand(true);
        search_surface.add_css_class("search-surface-stack");
        search_surface.append(&search_entry);
        search_surface.append(&search_history_revealer);
        search_bar.set_child(Some(&search_surface));
'''

old_append = '''        shell.append(&search_bar);
        shell.append(&search_history_revealer);
'''
new_append = '''        shell.append(&search_bar);
'''

if new_setup in text and new_append in text and old_append not in text:
    print("Recent-search dropdown is already aligned with the search entry.")
    raise SystemExit(0)

if text.count(old_setup) != 1:
    raise SystemExit(
        f"Expected one recent-search setup block; found {text.count(old_setup)}. No files were written."
    )
if text.count(old_append) != 1:
    raise SystemExit(
        f"Expected one separate dropdown append block; found {text.count(old_append)}. No files were written."
    )

updated = text.replace(old_setup, new_setup, 1).replace(old_append, new_append, 1)
path.write_text(updated, encoding="utf-8")
print("Recent-search dropdown aligned with the global search entry successfully.")
