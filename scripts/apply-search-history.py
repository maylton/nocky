#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
MAIN = ROOT / "src/main.rs"
SEARCH_HISTORY = ROOT / "src/search_history.rs"
CONTROLLER_MOD = ROOT / "src/app/controller/mod.rs"
CONTROLLER_HISTORY = ROOT / "src/app/controller/search_history.rs"
CONSTRUCTION = ROOT / "src/app/controller/construction.rs"
THEME_CSS = ROOT / "src/theme_css.rs"
STYLE = ROOT / "assets/themes/material-expressive/102-search-history.css"
ROADMAP = ROOT / "ROADMAP.md"
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


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if start_index < 0:
        if replacement in text:
            print(f"[already applied] {label}")
            return text
        raise PatchError(f"{label}: start marker not found")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise PatchError(f"{label}: end marker not found")
    print(f"[changed] {label}")
    return text[:start_index] + replacement + text[end_index:]


SEARCH_HISTORY_SOURCE = r'''use crate::search_text::normalize_search_text;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const SEARCH_HISTORY_VERSION: u32 = 1;
const SEARCH_HISTORY_LIMIT: usize = 20;
const MIN_QUERY_CHARACTERS: usize = 2;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct StoredSearchHistory {
    version: u32,
    queries: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchHistory {
    queries: Vec<String>,
}

impl SearchHistory {
    pub fn load() -> Self {
        let Ok(raw) = fs::read_to_string(search_history_path()) else {
            return Self::default();
        };
        let Ok(stored) = serde_json::from_str::<StoredSearchHistory>(&raw) else {
            return Self::default();
        };
        if stored.version != SEARCH_HISTORY_VERSION {
            return Self::default();
        }

        let mut history = Self::default();
        for query in stored.queries.into_iter().rev() {
            history.record_in_memory(&query);
        }
        history
    }

    pub fn queries(&self) -> &[String] {
        &self.queries
    }

    pub fn record(&mut self, raw_query: &str) -> bool {
        let changed = self.record_in_memory(raw_query);
        if changed {
            self.save();
        }
        changed
    }

    pub fn remove(&mut self, raw_query: &str) -> bool {
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

    fn record_in_memory(&mut self, raw_query: &str) -> bool {
        let query = normalized_display_query(raw_query);
        if query.chars().count() < MIN_QUERY_CHARACTERS {
            return false;
        }
        let key = normalize_search_text(&query);
        let previous = self
            .queries
            .iter()
            .position(|candidate| normalize_search_text(candidate) == key);
        let unchanged_at_front = previous == Some(0)
            && self
                .queries
                .first()
                .is_some_and(|candidate| candidate == &query);
        if unchanged_at_front {
            return false;
        }
        if let Some(index) = previous {
            self.queries.remove(index);
        }
        self.queries.insert(0, query);
        self.queries.truncate(SEARCH_HISTORY_LIMIT);
        true
    }

    fn save(&self) {
        if let Err(error) = save_search_history(&self.queries) {
            eprintln!("Could not save recent searches: {error}");
        }
    }
}

fn normalized_display_query(raw_query: &str) -> String {
    raw_query.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn save_search_history(queries: &[String]) -> Result<(), String> {
    let path = search_history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create search history folder: {error}"))?;
    }
    let payload = StoredSearchHistory {
        version: SEARCH_HISTORY_VERSION,
        queries: queries.to_vec(),
    };
    let serialized = serde_json::to_vec(&payload)
        .map_err(|error| format!("could not serialize search history: {error}"))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serialized)
        .map_err(|error| format!("could not write search history: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("could not replace search history: {error}"))?;
    Ok(())
}

fn search_history_path() -> PathBuf {
    gtk::glib::user_data_dir()
        .join("nocky")
        .join("search-history.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_normalizes_deduplicates_and_moves_queries_to_the_front() {
        let mut history = SearchHistory::default();
        assert!(history.record_in_memory("  Daft   Punk  "));
        assert!(history.record_in_memory("Muse"));
        assert!(history.record_in_memory("DAFT PUNK"));

        assert_eq!(history.queries, vec!["DAFT PUNK", "Muse"]);
        assert!(!history.record_in_memory("DAFT PUNK"));
    }

    #[test]
    fn history_ignores_single_character_queries_and_keeps_a_bounded_mru_list() {
        let mut history = SearchHistory::default();
        assert!(!history.record_in_memory("x"));
        for index in 0..25 {
            assert!(history.record_in_memory(&format!("query {index}")));
        }

        assert_eq!(history.queries.len(), SEARCH_HISTORY_LIMIT);
        assert_eq!(history.queries.first().map(String::as_str), Some("query 24"));
        assert_eq!(history.queries.last().map(String::as_str), Some("query 5"));
    }

    #[test]
    fn remove_and_clear_update_the_in_memory_history() {
        let mut history = SearchHistory::default();
        history.record_in_memory("Massive Attack");
        history.record_in_memory("Portishead");

        assert!(history.remove("massive attack"));
        assert_eq!(history.queries, vec!["Portishead"]);
        assert!(!history.remove("missing"));
        assert!(history.clear());
        assert!(history.queries.is_empty());
        assert!(!history.clear());
    }
}
'''

CONTROLLER_HISTORY_SOURCE = r'''//! Recent-search popover and local history actions.

use super::AppController;
use crate::{
    config::AppLanguage,
    ui::widgets::material_button::{
        apply_material_button, apply_material_icon_button, MaterialButtonSize,
        MaterialButtonSpec, MaterialButtonVariant, MaterialIconButtonSpec,
        MaterialIconButtonVariant,
    },
};
use gtk::prelude::*;
use std::rc::Rc;

#[derive(Clone, Copy)]
struct SearchHistoryCopy {
    title: &'static str,
    clear_all: &'static str,
    remove: &'static str,
}

fn search_history_copy(language: AppLanguage) -> SearchHistoryCopy {
    match language {
        AppLanguage::Portuguese => SearchHistoryCopy {
            title: "Buscas recentes",
            clear_all: "Limpar tudo",
            remove: "Remover busca recente",
        },
        AppLanguage::English => SearchHistoryCopy {
            title: "Recent searches",
            clear_all: "Clear all",
            remove: "Remove recent search",
        },
        AppLanguage::Spanish => SearchHistoryCopy {
            title: "Búsquedas recientes",
            clear_all: "Limpiar todo",
            remove: "Eliminar búsqueda reciente",
        },
    }
}

impl AppController {
    pub(crate) fn record_recent_search(self: &Rc<Self>, query: &str) {
        if self.search_history.borrow_mut().record(query) {
            self.refresh_recent_searches(false);
        }
    }

    pub(crate) fn refresh_recent_searches(self: &Rc<Self>, reveal: bool) {
        let queries = self.search_history.borrow().queries().to_vec();
        if queries.is_empty() {
            self.search_history_popover.popdown();
            self.search_history_popover.set_child(None::<&gtk::Widget>);
            return;
        }

        let copy = search_history_copy(self.config.borrow().language);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.add_css_class("search-history-content");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("search-history-header");
        let title = gtk::Label::new(Some(copy.title));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("search-history-title");
        let clear = gtk::Button::with_label(copy.clear_all);
        apply_material_button(
            &clear,
            MaterialButtonSpec::new(MaterialButtonVariant::Text, MaterialButtonSize::Compact),
        );
        clear.add_css_class("search-history-clear");
        {
            let weak = Rc::downgrade(self);
            clear.connect_clicked(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.search_history.borrow_mut().clear();
                controller.refresh_recent_searches(false);
            });
        }
        header.append(&title);
        header.append(&clear);
        root.append(&header);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.add_css_class("search-history-list");

        for query in queries {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_selectable(false);
            row.add_css_class("search-history-row");

            let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let icon = gtk::Image::from_icon_name("document-open-recent-symbolic");
            icon.set_pixel_size(16);
            icon.add_css_class("search-history-icon");

            let query_button = gtk::Button::with_label(&query);
            query_button.set_hexpand(true);
            query_button.set_halign(gtk::Align::Fill);
            query_button.add_css_class("search-history-query");
            apply_material_button(
                &query_button,
                MaterialButtonSpec::new(MaterialButtonVariant::Text, MaterialButtonSize::Compact),
            );
            {
                let weak = Rc::downgrade(self);
                let selected_query = query.clone();
                query_button.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    controller.record_recent_search(&selected_query);
                    controller.search_history_popover.popdown();
                    controller.search_entry.set_text(&selected_query);
                    controller.search_entry.set_position(-1);
                    controller.search_entry.grab_focus();
                });
            }

            let remove = gtk::Button::builder()
                .icon_name("edit-delete-symbolic")
                .tooltip_text(copy.remove)
                .build();
            remove.add_css_class("search-history-remove");
            apply_material_icon_button(
                &remove,
                MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
            );
            {
                let weak = Rc::downgrade(self);
                let removed_query = query;
                remove.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    controller.search_history.borrow_mut().remove(&removed_query);
                    controller.refresh_recent_searches(true);
                });
            }

            content.append(&icon);
            content.append(&query_button);
            content.append(&remove);
            row.set_child(Some(&content));
            list.append(&row);
        }

        root.append(&list);
        self.search_history_popover.set_child(Some(&root));
        if reveal
            && self.search_entry.has_focus()
            && self.search_entry.text().trim().is_empty()
        {
            self.search_history_popover.popup();
        }
    }
}
'''

STYLE_SOURCE = r'''window.theme-material-expressive popover.search-history-popover > contents {
  min-width: 360px;
  border-radius: 20px;
  padding: 0;
  background: alpha(@m3_surface_container_high, 0.98);
  border: 1px solid alpha(@m3_outline_variant, 0.58);
}

window.theme-material-expressive .search-history-content {
  min-width: 360px;
  padding: 12px;
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

window.theme-material-expressive .search-history-query label {
  text-align: left;
}

window.theme-material-expressive .search-history-remove {
  min-width: 36px;
  min-height: 36px;
}
'''

DOC_SOURCE = r'''# Recent search history

## Scope

Nocky keeps a small local MRU list of completed search text. It is shared by
local-library and YouTube Music modes because it stores only the text entered by
the user, not account identifiers, remote result metadata or continuation
tokens.

## Behavior

- a stable query is recorded after 800 ms;
- pressing Enter records it immediately;
- selecting a recent query moves it to the front;
- matching is case-insensitive and whitespace-normalized;
- single-character fragments are ignored;
- at most 20 entries are retained;
- entries can be removed individually or cleared together;
- focusing the empty global search field opens the recent-query popover;
- typing non-empty text closes the popover and keeps the existing live search
  behavior.

## Storage

The list is stored atomically in the user data directory as
`nocky/search-history.json`. Corrupt or incompatible files are ignored. Search
cache entries remain session-scoped and separate from this file.

## Deferred

Mixed local/remote ranking, route-aware cancellation and result-update
announcements remain later search checkpoints.
'''

SEARCH_CALLBACK_BLOCK = r'''        {
            let weak = Rc::downgrade(&controller);
            let pending_search = Rc::new(RefCell::new(None::<glib::SourceId>));
            let pending_history = Rc::new(RefCell::new(None::<glib::SourceId>));
            search_entry.connect_search_changed(move |entry| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };

                if let Some(source) = pending_search.borrow_mut().take() {
                    source.remove();
                }
                if let Some(source) = pending_history.borrow_mut().take() {
                    source.remove();
                }

                let query = entry.text().trim().to_string();
                controller.search_query.replace(query.clone());
                let youtube_only =
                    controller.config.borrow().startup_source == Some(StartupSource::YouTube);

                if query.is_empty() {
                    controller.refresh_recent_searches(true);
                    controller
                        .youtube_search_request_id
                        .set(controller.youtube_search_request_id.get().wrapping_add(1));
                    controller.youtube_library.borrow_mut().search =
                        YouTubeSearchResults::default();
                    controller.navigate_browser(BrowserRoute::All);
                    return;
                }

                controller.search_history_popover.popdown();
                if youtube_only {
                    let mut cached = controller
                        .youtube_library
                        .borrow()
                        .cached_search_results(&query);
                    cached.loading = true;
                    controller.youtube_library.borrow_mut().search = cached;
                }
                controller.navigate_browser(BrowserRoute::All);

                let history_controller = Rc::downgrade(&controller);
                let history_pending = pending_history.clone();
                let history_query = query.clone();
                let history_source =
                    glib::timeout_add_local_once(Duration::from_millis(800), move || {
                        history_pending.borrow_mut().take();
                        if let Some(controller) = history_controller.upgrade() {
                            controller.record_recent_search(&history_query);
                        }
                    });
                pending_history.borrow_mut().replace(history_source);

                if !youtube_only {
                    return;
                }

                let delayed_controller = Rc::downgrade(&controller);
                let delayed_pending = pending_search.clone();
                let source = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                    delayed_pending.borrow_mut().take();
                    if let Some(controller) = delayed_controller.upgrade() {
                        controller.request_global_youtube_search(query);
                    }
                });
                pending_search.borrow_mut().replace(source);
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            search_entry.connect_activate(move |entry| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let query = entry.text().trim().to_string();
                if !query.is_empty() {
                    controller.record_recent_search(&query);
                    controller.search_history_popover.popdown();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            let focus = gtk::EventControllerFocus::new();
            focus.connect_enter(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if controller.search_entry.text().trim().is_empty() {
                    controller.refresh_recent_searches(true);
                }
            });
            search_entry.add_controller(focus);
        }

'''


def patch_main(text: str) -> str:
    return replace_once(
        text,
        "mod search_text;\n",
        "mod search_history;\nmod search_text;\n",
        "Register search history module",
    )


def patch_controller_mod(text: str) -> str:
    text = replace_once(
        text,
        "mod settings;\n",
        "mod search_history;\nmod settings;\n",
        "Register search history controller",
    )
    text = replace_once(
        text,
        "    reveal_bounce::RevealBounce,\n",
        "    reveal_bounce::RevealBounce,\n    search_history::SearchHistory,\n",
        "Import search history state",
    )
    text = replace_once(
        text,
        "    pub(crate) search_query: RefCell<String>,\n",
        "    pub(crate) search_query: RefCell<String>,\n    pub(crate) search_history: RefCell<SearchHistory>,\n",
        "Add search history runtime state",
    )
    return replace_once(
        text,
        "    pub(crate) search_entry: gtk::SearchEntry,\n",
        "    pub(crate) search_entry: gtk::SearchEntry,\n    pub(crate) search_history_popover: gtk::Popover,\n",
        "Add recent-search popover field",
    )


def patch_construction(text: str) -> str:
    text = replace_once(
        text,
        "    reveal_bounce::RevealBounce,\n",
        "    reveal_bounce::RevealBounce,\n    search_history::SearchHistory,\n",
        "Import search history during construction",
    )
    text = replace_once(
        text,
        '''        search_entry.add_css_class("expressive-search-entry");
        search_bar.set_child(Some(&search_entry));
''',
        '''        search_entry.add_css_class("expressive-search-entry");
        let search_history_popover = gtk::Popover::new();
        search_history_popover.set_parent(&search_entry);
        search_history_popover.set_position(gtk::PositionType::Bottom);
        search_history_popover.set_has_arrow(false);
        search_history_popover.set_autohide(true);
        search_history_popover.add_css_class("search-history-popover");
        search_bar.set_child(Some(&search_entry));
''',
        "Build recent-search popover",
    )
    text = replace_once(
        text,
        "                search_query: RefCell::new(String::new()),\n",
        "                search_query: RefCell::new(String::new()),\n                search_history: RefCell::new(SearchHistory::load()),\n",
        "Load recent-search history",
    )
    text = replace_once(
        text,
        "            search_entry: search_entry.clone(),\n",
        "            search_entry: search_entry.clone(),\n            search_history_popover: search_history_popover.clone(),\n",
        "Store recent-search popover",
    )

    old_toggle = '''        {
            let search_bar = search_bar.clone();
            search_button.connect_toggled(move |button| {
                set_material_icon_button_selected(button, button.is_active());
                search_bar.set_search_mode(button.is_active());
            });
        }
'''
    new_toggle = '''        {
            let search_bar = search_bar.clone();
            let weak = Rc::downgrade(&controller);
            search_button.connect_toggled(move |button| {
                set_material_icon_button_selected(button, button.is_active());
                search_bar.set_search_mode(button.is_active());
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if button.is_active() {
                    controller.search_entry.grab_focus();
                    if controller.search_entry.text().trim().is_empty() {
                        controller.refresh_recent_searches(true);
                    }
                } else {
                    controller.search_history_popover.popdown();
                }
            });
        }
'''
    text = replace_once(text, old_toggle, new_toggle, "Open recent searches with global search")

    return replace_between(
        text,
        '''        {
            let weak = Rc::downgrade(&controller);
            let pending_search = Rc::new(RefCell::new(None::<glib::SourceId>));
            search_entry.connect_search_changed(move |entry| {
''',
        '''        {
            let weak = Rc::downgrade(&controller);
            settings_button.connect_toggled(move |button| {
''',
        SEARCH_CALLBACK_BLOCK,
        "Record and recall recent searches",
    )


def patch_theme_css(text: str) -> str:
    return replace_once(
        text,
        '''    (
        "101-keyboard-search.css",
        include_str!("../assets/themes/material-expressive/101-keyboard-search.css"),
    ),
];
''',
        '''    (
        "101-keyboard-search.css",
        include_str!("../assets/themes/material-expressive/101-keyboard-search.css"),
    ),
    (
        "102-search-history.css",
        include_str!("../assets/themes/material-expressive/102-search-history.css"),
    ),
];
''',
        "Register recent-search CSS module",
    )


def patch_roadmap(text: str) -> str:
    text = replace_once(
        text,
        "- 🟡 Search history and recent queries.\n",
        "- 🟡 Better ranking across mixed local and remote results.\n",
        "Advance active search checkpoint",
    )
    anchor = "- ✅ Real per-category remote pagination backed by opaque YouTube Music continuations.\n"
    text = replace_once(
        text,
        anchor,
        anchor + "- ✅ Local recent-query history with MRU ordering, individual removal and clear-all controls.\n",
        "Document completed recent-search history",
    )
    text = replace_once(
        text,
        "- Search history and recent queries.\n",
        "",
        "Remove completed recent-search item",
    )
    return replace_once(
        text,
        "8. Add search history, mixed-source ranking and route-aware cancellation.\n",
        "8. Improve mixed-source ranking and route-aware cancellation.\n",
        "Advance recommended search order",
    )


def main() -> int:
    required = [MAIN, CONTROLLER_MOD, CONSTRUCTION, THEME_CSS, ROADMAP]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    if "YouTubeSearchCategory" not in (ROOT / "src/youtube/mod.rs").read_text(encoding="utf-8"):
        print("ERROR: apply and validate real remote search pagination first.", file=sys.stderr)
        return 1
    if "fn search_collection_row(" not in (ROOT / "src/browser.rs").read_text(encoding="utf-8"):
        print("ERROR: apply and validate keyboard-first search actions first.", file=sys.stderr)
        return 1

    creations = [
        (SEARCH_HISTORY, SEARCH_HISTORY_SOURCE),
        (CONTROLLER_HISTORY, CONTROLLER_HISTORY_SOURCE),
        (STYLE, STYLE_SOURCE),
        (DOC, DOC_SOURCE),
    ]
    for path, expected in creations:
        if path.exists() and path.read_text(encoding="utf-8") != expected:
            print(f"ERROR: {path} already exists with different content.", file=sys.stderr)
            print("No files were written.", file=sys.stderr)
            return 1

    updated = dict(original)
    try:
        updated[MAIN] = patch_main(updated[MAIN])
        updated[CONTROLLER_MOD] = patch_controller_mod(updated[CONTROLLER_MOD])
        updated[CONSTRUCTION] = patch_construction(updated[CONSTRUCTION])
        updated[THEME_CSS] = patch_theme_css(updated[THEME_CSS])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed: list[Path] = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    for path, content in creations:
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Recent search history patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
