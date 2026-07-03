#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
CONTROLLER_MOD = ROOT / "src/app/controller/mod.rs"
CONSTRUCTION = ROOT / "src/app/controller/construction.rs"
CONTROLLER_HISTORY = ROOT / "src/app/controller/search_history.rs"
STYLE = ROOT / "assets/themes/material-expressive/102-search-history.css"
DOC = ROOT / "docs/SEARCH_HISTORY.md"


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


def replace_final_reveal_block(text: str) -> str:
    marker = "        if reveal && self.search_entry.text().trim().is_empty() {"
    start = text.rfind(marker)
    if start < 0:
        direct = '''        self.search_history_revealer.set_reveal_child(
            reveal && self.search_entry.text().trim().is_empty(),
        );
'''
        if direct in text:
            print("[already applied] Inline dropdown reveal contract")
            return text
        raise PatchError("Inline dropdown reveal contract: start marker not found")

    opening = text.find("{", start)
    depth = 0
    end = None
    for index in range(opening, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    if end is None:
        raise PatchError("Inline dropdown reveal contract: end marker not found")

    replacement = '''        self.search_history_revealer.set_reveal_child(
            reveal && self.search_entry.text().trim().is_empty(),
        );'''
    print("[changed] Inline dropdown reveal contract")
    return text[:start] + replacement + text[end:]


STYLE_SOURCE = '''window.theme-material-expressive .search-history-dropdown {
  background: transparent;
}

window.theme-material-expressive .search-history-content {
  min-width: 420px;
  margin: 0 0 10px;
  padding: 12px;
  border-radius: 0 0 20px 20px;
  background: alpha(@m3_surface_container_high, 0.98);
  border: 1px solid alpha(@m3_outline_variant, 0.58);
  border-top-width: 0;
}

window.theme-material-expressive .search-history-header {
  margin: 0 2px 2px;
}

window.theme-material-expressive .search-history-title {
  font-weight: 650;
  color: @m3_on_surface;
}

window.theme-material-expressive .search-history-list {
  background: transparent;
}

window.theme-material-expressive .search-history-row {
  padding: 2px 4px;
}

window.theme-material-expressive .search-history-row > box {
  min-height: 42px;
}

window.theme-material-expressive .search-history-icon {
  margin-left: 6px;
  color: @m3_on_surface_variant;
}

window.theme-material-expressive .search-history-query {
  padding-left: 8px;
  padding-right: 8px;
}

window.theme-material-expressive .search-history-remove {
  min-width: 36px;
  min-height: 36px;
}
'''


def patch_controller_mod(text: str) -> str:
    return replace_once(
        text,
        "    pub(crate) search_history_popover: gtk::Popover,\n",
        "    pub(crate) search_history_revealer: gtk::Revealer,\n",
        "Replace popover field with revealer",
    )


def patch_construction(text: str) -> str:
    text = replace_once(
        text,
        '''        let search_history_popover = gtk::Popover::new();
        search_history_popover.set_parent(&search_entry);
        search_history_popover.set_position(gtk::PositionType::Bottom);
        search_history_popover.set_has_arrow(false);
        search_history_popover.set_autohide(true);
        search_history_popover.add_css_class("search-history-popover");
        search_bar.set_child(Some(&search_entry));
''',
        '''        let search_history_revealer = gtk::Revealer::new();
        search_history_revealer
            .set_transition_type(gtk::RevealerTransitionType::SlideDown);
        search_history_revealer.set_transition_duration(180);
        search_history_revealer.set_reveal_child(false);
        search_history_revealer.set_hexpand(true);
        search_history_revealer.add_css_class("search-history-dropdown");
        search_bar.set_child(Some(&search_entry));
''',
        "Build inline recent-search dropdown",
    )
    text = replace_once(
        text,
        "        shell.append(&search_bar);\n",
        "        shell.append(&search_bar);\n        shell.append(&search_history_revealer);\n",
        "Place dropdown below search bar",
    )
    text = replace_once(
        text,
        "            search_history_popover: search_history_popover.clone(),\n",
        "            search_history_revealer: search_history_revealer.clone(),\n",
        "Store dropdown revealer",
    )
    return text.replace(
        "controller.search_history_popover.popdown();",
        "controller.search_history_revealer.set_reveal_child(false);",
    )


def patch_controller_history(text: str) -> str:
    text = text.replace(
        "self.search_history_popover.popdown();",
        "self.search_history_revealer.set_reveal_child(false);",
    )
    text = text.replace(
        "controller.search_history_popover.popdown();",
        "controller.search_history_revealer.set_reveal_child(false);",
    )
    text = text.replace(
        "self.search_history_popover.set_child(None::<&gtk::Widget>);",
        "self.search_history_revealer.set_child(None::<&gtk::Widget>);",
    )
    text = text.replace(
        "self.search_history_popover.set_child(Some(&root));",
        "self.search_history_revealer.set_child(Some(&root));",
    )
    text = replace_once(
        text,
        '''        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.add_css_class("search-history-content");
''',
        '''        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_halign(gtk::Align::Center);
        root.set_width_request(420);
        root.add_css_class("search-history-content");
''',
        "Align inline dropdown with search field",
    )
    return replace_final_reveal_block(text)


def patch_doc(text: str) -> str:
    text = text.replace("recent-query popover", "recent-query dropdown")
    text = text.replace("opens the recent-query popover", "opens the recent-query dropdown")
    text = text.replace("closes the popover", "closes the dropdown")
    return text


def main() -> int:
    required = [CONTROLLER_MOD, CONSTRUCTION, CONTROLLER_HISTORY, STYLE, DOC]
    missing = [path for path in required if not path.is_file()]
    if missing:
        for path in missing:
            print(f"missing: {path}")
        raise SystemExit("Run this script from the Nocky repository root after applying the search-history patch.")

    original = {path: path.read_text(encoding="utf-8") for path in required}
    updated = dict(original)
    try:
        updated[CONTROLLER_MOD] = patch_controller_mod(updated[CONTROLLER_MOD])
        updated[CONSTRUCTION] = patch_construction(updated[CONSTRUCTION])
        updated[CONTROLLER_HISTORY] = patch_controller_history(updated[CONTROLLER_HISTORY])
        updated[STYLE] = STYLE_SOURCE
        updated[DOC] = patch_doc(updated[DOC])
    except PatchError as error:
        print(f"ERROR: {error}")
        print("No files were written.")
        return 1

    changed = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Recent searches converted to an inline dropdown successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
