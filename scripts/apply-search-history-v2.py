#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-search-history.py")
text = SOURCE.read_text(encoding="utf-8")

old_remove_clear = r'''    pub fn remove(&mut self, raw_query: &str) -> bool {
        let key = normalize_search_text(raw_query);
        let Some(index) = self
            .queries
            .iter()
            .position(|query| normalize_search_text(query) == key)
        else {
            return false;
        };
        self.queries.remove(index);
        self.save();
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.queries.is_empty() {
            return false;
        }
        self.queries.clear();
        self.save();
        true
    }
'''
new_remove_clear = r'''    pub fn remove(&mut self, raw_query: &str) -> bool {
        let changed = self.remove_in_memory(raw_query);
        if changed {
            self.save();
        }
        changed
    }

    pub fn clear(&mut self) -> bool {
        let changed = self.clear_in_memory();
        if changed {
            self.save();
        }
        changed
    }

    fn remove_in_memory(&mut self, raw_query: &str) -> bool {
        let key = normalize_search_text(raw_query);
        let Some(index) = self
            .queries
            .iter()
            .position(|query| normalize_search_text(query) == key)
        else {
            return false;
        };
        self.queries.remove(index);
        true
    }

    fn clear_in_memory(&mut self) -> bool {
        if self.queries.is_empty() {
            return false;
        }
        self.queries.clear();
        true
    }
'''

old_tests = r'''        assert!(history.remove("massive attack"));
        assert_eq!(history.queries, vec!["Portishead"]);
        assert!(!history.remove("missing"));
        assert!(history.clear());
        assert!(history.queries.is_empty());
        assert!(!history.clear());
'''
new_tests = r'''        assert!(history.remove_in_memory("massive attack"));
        assert_eq!(history.queries, vec!["Portishead"]);
        assert!(!history.remove_in_memory("missing"));
        assert!(history.clear_in_memory());
        assert!(history.queries.is_empty());
        assert!(!history.clear_in_memory());
'''

old_query_button = r'''            let query_button = gtk::Button::with_label(&query);
            query_button.set_hexpand(true);
            query_button.set_halign(gtk::Align::Fill);
            query_button.add_css_class("search-history-query");
'''
new_query_button = r'''            let query_label = gtk::Label::new(Some(&query));
            query_label.set_xalign(0.0);
            query_label.set_hexpand(true);
            let query_button = gtk::Button::new();
            query_button.set_child(Some(&query_label));
            query_button.set_hexpand(true);
            query_button.set_halign(gtk::Align::Fill);
            query_button.add_css_class("search-history-query");
'''

old_css = r'''
window.theme-material-expressive .search-history-query label {
  text-align: left;
}
'''

for old, new, label in [
    (old_remove_clear, new_remove_clear, "side-effect-free history mutations"),
    (old_tests, new_tests, "side-effect-free history tests"),
    (old_query_button, new_query_button, "left-aligned query button content"),
    (old_css, "", "supported GTK CSS"),
]:
    if old not in text:
        raise SystemExit(f"Could not apply {label} correction to {SOURCE}.")
    text = text.replace(old, new, 1)

namespace = {
    "__name__": "__main__",
    "__file__": str(SOURCE),
}
exec(compile(text, str(SOURCE), "exec"), namespace)
