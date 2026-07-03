#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

path = Path("src/app/controller/search_history.rs")
if not path.is_file():
    raise SystemExit("Run this script from the Nocky repository root after applying the search-history patch.")

text = path.read_text(encoding="utf-8")
old = '''        if reveal
            && self.search_entry.has_focus()
            && self.search_entry.text().trim().is_empty()
        {
            self.search_history_popover.popup();
        }
'''
new = '''        if reveal && self.search_entry.text().trim().is_empty() {
            // GtkSearchEntry delegates focus to its internal text widget, so
            // has_focus() is usually false here. Wait until the SearchBar has
            // finished mapping the entry, then reveal the anchored popover.
            let weak = Rc::downgrade(self);
            gtk::glib::idle_add_local_once(move || {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if controller.search_entry.is_mapped()
                    && controller.search_entry.text().trim().is_empty()
                    && controller.search_history_popover.child().is_some()
                {
                    controller.search_history_popover.popup();
                }
            });
        }
'''

if new in text:
    print("Recent-search popover reveal fix is already applied.")
elif text.count(old) != 1:
    raise SystemExit(
        f"Expected one recent-search popup guard; found {text.count(old)}. No files were written."
    )
else:
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
    print("Recent-search popover reveal fix applied successfully.")
