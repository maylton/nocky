#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
CONSTRUCTION = ROOT / "src/app/controller/construction.rs"
CONTROLLER_HISTORY = ROOT / "src/app/controller/search_history.rs"
STYLE = ROOT / "assets/themes/material-expressive/102-search-history.css"


class PatchError(RuntimeError):
    pass


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count == 0 and new in text:
        print(f"[already applied] {label}")
        return text
    if count != 1:
        raise PatchError(f"{label}: expected one match, found {count}")
    print(f"[changed] {label}")
    return text.replace(old, new, 1)


STYLE_SOURCE = '''window.theme-material-expressive .search-history-dropdown {
  margin: 0;
  background: transparent;
}

window.theme-material-expressive .search-history-content {
  margin: 6px 0 10px;
  padding: 10px 12px 12px;
  border-radius: 24px;
  color: @m3_on_surface;
  background-color: @m3_surface_container_high;
  border: 1px solid alpha(@m3_outline, 0.18);
  box-shadow: 0 8px 22px alpha(black, 0.16);
}

window.theme-material-expressive .search-history-header {
  min-height: 38px;
  margin: 0 2px 4px;
  padding: 0 4px 0 8px;
}

window.theme-material-expressive .search-history-title {
  color: @m3_on_surface;
  font-size: 0.86rem;
  font-weight: 760;
}

window.theme-material-expressive .search-history-clear {
  color: @m3_primary;
  background-color: transparent;
}

window.theme-material-expressive .search-history-clear:hover {
  background-color: alpha(@m3_primary, 0.10);
}

window.theme-material-expressive .search-history-list,
window.theme-material-expressive .search-history-list > row {
  background-color: transparent;
}

window.theme-material-expressive .search-history-row {
  min-height: 44px;
  margin: 2px 0;
  padding: 0 4px;
  border-radius: 18px;
  color: @m3_on_surface_variant;
  border: 1px solid transparent;
}

window.theme-material-expressive .search-history-row:hover,
window.theme-material-expressive .search-history-row:focus-within {
  color: @m3_on_surface;
  background-color: alpha(@m3_primary, 0.09);
  border-color: alpha(@m3_primary, 0.18);
}

window.theme-material-expressive .search-history-row > box {
  min-height: 44px;
}

window.theme-material-expressive .search-history-icon {
  margin-left: 8px;
  color: @m3_on_surface_variant;
}

window.theme-material-expressive .search-history-query {
  min-height: 40px;
  padding: 0 10px;
  color: @m3_on_surface;
  background-color: transparent;
  box-shadow: none;
}

window.theme-material-expressive .search-history-query:hover,
window.theme-material-expressive .search-history-query:active,
window.theme-material-expressive .search-history-query:focus {
  background-color: transparent;
  box-shadow: none;
}

window.theme-material-expressive .search-history-remove {
  min-width: 36px;
  min-height: 36px;
  color: @m3_on_surface_variant;
  background-color: transparent;
}

window.theme-material-expressive .search-history-remove:hover {
  color: @m3_on_surface;
  background-color: alpha(@m3_primary, 0.12);
}
'''


def patch_construction(text: str) -> str:
    return replace_once(
        text,
        '''        search_history_revealer.set_hexpand(true);
        search_history_revealer.add_css_class("search-history-dropdown");
''',
        '''        search_history_revealer.set_hexpand(true);
        search_history_revealer.set_halign(gtk::Align::Fill);
        // Match the SearchEntry's left edge and reserve the SearchBar close
        // button area on the right.
        search_history_revealer.set_margin_start(12);
        search_history_revealer.set_margin_end(56);
        search_history_revealer.add_css_class("search-history-dropdown");
''',
        "Align dropdown with global search entry",
    )


def patch_controller_history(text: str) -> str:
    text = replace_once(
        text,
        '''        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_halign(gtk::Align::Center);
        root.set_width_request(420);
        root.add_css_class("search-history-content");
''',
        '''        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.set_halign(gtk::Align::Fill);
        root.set_hexpand(true);
        root.add_css_class("search-history-content");
''',
        "Use responsive dropdown surface",
    )
    text = replace_once(
        text,
        '''        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.add_css_class("search-history-list");
''',
        '''        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("search-history-list");
''',
        "Remove nested boxed-list surface",
    )
    text = replace_once(
        text,
        '''            let icon = gtk::Image::from_icon_name("document-open-recent-symbolic");
''',
        '''            let icon = gtk::Image::from_icon_name("system-search-symbolic");
''',
        "Use search icon for recent queries",
    )
    text = replace_once(
        text,
        '''            let remove = gtk::Button::builder()
                .icon_name("edit-delete-symbolic")
''',
        '''            let remove = gtk::Button::builder()
                .icon_name("window-close-symbolic")
''',
        "Use lightweight remove action",
    )
    return replace_once(
        text,
        '''        root.append(&list);
        self.search_history_revealer.set_child(Some(&root));
''',
        '''        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(280);
        scroll.set_child(Some(&list));
        scroll.add_css_class("search-history-scroll");
        root.append(&scroll);
        self.search_history_revealer.set_child(Some(&root));
''',
        "Keep long recent-search lists compact",
    )


def main() -> int:
    required = [CONSTRUCTION, CONTROLLER_HISTORY, STYLE]
    missing = [path for path in required if not path.is_file()]
    if missing:
        for path in missing:
            print(f"missing: {path}")
        raise SystemExit(
            "Run this script from the Nocky repository root after converting recent searches to an inline dropdown."
        )

    original = {path: path.read_text(encoding="utf-8") for path in required}
    updated = dict(original)
    try:
        updated[CONSTRUCTION] = patch_construction(updated[CONSTRUCTION])
        updated[CONTROLLER_HISTORY] = patch_controller_history(updated[CONTROLLER_HISTORY])
        updated[STYLE] = STYLE_SOURCE
    except PatchError as error:
        print(f"ERROR: {error}")
        print("No files were written.")
        return 1

    changed = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Recent-search dropdown visual refinement applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
