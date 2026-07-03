#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"
THEME_CSS = ROOT / "src/theme_css.rs"
KEYBOARD_CSS = ROOT / "assets/themes/material-expressive/101-keyboard-search.css"
ROADMAP = ROOT / "ROADMAP.md"
AUDIT = ROOT / "docs/CARD_ACTIONS_LOADING_AUDIT.md"


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


SEARCH_SECTION = r'''fn search_list_section(
    title: &str,
    empty_message: &str,
    cards: Vec<HomeCard>,
    limit: Rc<Cell<usize>>,
    event_tx: &Sender<BrowserEvent>,
    loading: bool,
    copy: SearchCopy,
    config: &AppConfig,
    playback: &BrowserPlaybackState,
) -> gtk::Box {
    let total = cards.len();
    let visible = total.min(limit.get());
    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    section.add_css_class("home-section");
    section.add_css_class("search-section-card");
    section.append(&search_section_heading(
        title, visible, total, loading, copy,
    ));

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(true);
    list.add_css_class("boxed-list");
    list.add_css_class("search-results-list");
    list.add_css_class("search-results-surface");
    list.add_css_class("search-keyboard-list");

    let visible_cards = Rc::new(RefCell::new(Vec::<HomeCard>::new()));
    {
        let sender = event_tx.clone();
        let cards = visible_cards.clone();
        list.connect_row_activated(move |_, row| {
            let Some(card) = cards.borrow().get(row.index() as usize).cloned() else {
                return;
            };
            let _ = sender.send(card.open_event());
        });
    }

    if total == 0 {
        list.append(&empty_row(if loading {
            copy.searching
        } else {
            empty_message
        }));
    } else {
        for card in cards.into_iter().take(visible) {
            let row = search_collection_row(&card, event_tx, config, playback);
            list.append(&row);
            visible_cards.borrow_mut().push(card);
        }
    }
    section.append(&list);

    if total > visible {
        section.append(&search_more_button(
            title,
            total - visible,
            limit,
            event_tx,
            copy,
        ));
    }
    section
}

fn search_collection_action_spec(
    card: &HomeCard,
    config: &AppConfig,
    playback: &BrowserPlaybackState,
) -> Option<CollectionActionSpec> {
    match card {
        HomeCard::LocalAlbum { title, .. } => {
            Some(local_album_action_spec(title, playback, config))
        }
        HomeCard::YouTubeAlbum { item, .. } => {
            Some(youtube_album_action_spec(item, &item.title, playback, config))
        }
        HomeCard::LocalPlaylist { title, .. } => {
            Some(local_playlist_action_spec(title, playback, config))
        }
        HomeCard::YouTubePlaylist(item) => {
            Some(youtube_playlist_action_spec(item, playback, config))
        }
        _ => None,
    }
}

fn is_keyboard_activation_key(key: gdk::Key) -> bool {
    key == gdk::Key::Return || key == gdk::Key::KP_Enter || key == gdk::Key::space
}

fn install_row_keyboard_activation(
    row: &gtk::ListBoxRow,
    event: BrowserEvent,
    event_tx: &Sender<BrowserEvent>,
) {
    let controller = gtk::EventControllerKey::new();
    let row_weak = row.downgrade();
    let sender = event_tx.clone();
    controller.connect_key_pressed(move |_, key, _, _| {
        let Some(row) = row_weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if !row.has_focus() || !is_keyboard_activation_key(key) {
            return glib::Propagation::Proceed;
        }

        let _ = sender.send(event.clone());
        glib::Propagation::Stop
    });
    row.add_controller(controller);
}

fn search_collection_row(
    card: &HomeCard,
    event_tx: &Sender<BrowserEvent>,
    config: &AppConfig,
    playback: &BrowserPlaybackState,
) -> gtk::ListBoxRow {
    let (cover_path, icon_name, title, subtitle, detail, online) = match card {
        HomeCard::LocalAlbum {
            title,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "media-optical-symbolic",
            title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            false,
        ),
        HomeCard::YouTubeAlbum {
            item,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "media-optical-symbolic",
            item.title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            true,
        ),
        HomeCard::LocalArtist {
            title,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "avatar-default-symbolic",
            title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            false,
        ),
        HomeCard::YouTubeArtist {
            item,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "avatar-default-symbolic",
            item.title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            true,
        ),
        HomeCard::LocalPlaylist { title, subtitle } => (
            None,
            "view-list-symbolic",
            title.as_str(),
            subtitle.as_str(),
            "Playlist local",
            false,
        ),
        HomeCard::LocalMix {
            title,
            subtitle,
            detail,
            cover_path,
            ..
        } => (
            cover_path.as_deref(),
            "media-playlist-shuffle-symbolic",
            title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            false,
        ),
        HomeCard::YouTubeTrack { item, .. } => (
            item.cached_cover(),
            "audio-x-generic-symbolic",
            item.title.as_str(),
            item.subtitle.as_str(),
            "YouTube Music",
            true,
        ),
        HomeCard::YouTubePlaylist(item) => (
            item.cached_cover(),
            "view-list-symbolic",
            item.title.as_str(),
            youtube_playlist_subtitle(item),
            youtube_playlist_detail(item),
            true,
        ),
    };

    let leading = search_result_artwork(cover_path, icon_name);

    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.set_hexpand(true);
    title_label.set_single_line_mode(true);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.add_css_class("heading");

    let secondary = if subtitle.is_empty() { detail } else { subtitle };
    let subtitle_label = gtk::Label::new(Some(secondary));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_single_line_mode(true);
    subtitle_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    let source = gtk::Label::new(Some(if online { "YouTube" } else { "Local" }));
    source.add_css_class("pill");
    source.add_css_class("search-source-badge");

    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.set_pixel_size(16);
    arrow.add_css_class("dim-label");
    arrow.add_css_class("search-result-arrow");

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.add_css_class("search-result-row");
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.append(&leading);
    content.append(&text);
    content.append(&source);

    if let Some(spec) = search_collection_action_spec(card, config, playback) {
        let action = collection_primary_action_button(&spec, event_tx, config.language);
        action.set_size_request(36, 36);
        action.set_halign(gtk::Align::End);
        action.set_valign(gtk::Align::Center);
        action.add_css_class("search-result-primary-action");
        content.append(&action);
    }

    content.append(&arrow);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    row.set_focusable(true);
    row.set_activatable(true);
    row.set_selectable(true);
    row.add_css_class("search-result-keyboard-row");
    install_row_keyboard_activation(&row, card.open_event(), event_tx);
    row
}

#[cfg(test)]
mod keyboard_search_action_tests {
    use super::*;

    #[test]
    fn enter_space_and_keypad_enter_activate_rows() {
        assert!(is_keyboard_activation_key(gdk::Key::Return));
        assert!(is_keyboard_activation_key(gdk::Key::KP_Enter));
        assert!(is_keyboard_activation_key(gdk::Key::space));
        assert!(!is_keyboard_activation_key(gdk::Key::Escape));
    }
}

'''


KEYBOARD_CSS_CONTENT = r'''/* Loaded after 100-buttons.css so keyboard focus remains visible. */
window.theme-material-expressive
  flowboxchild.collection-grid-wrapper:focus {
  box-shadow: none;
}

window.theme-material-expressive
  .collection-action-focusable:focus,
window.theme-material-expressive
  row.search-result-keyboard-row:focus {
  box-shadow:
    0 0 0 3px alpha(@m3_primary, 0.30),
    0 8px 20px alpha(black, 0.16);
}

window.theme-material-expressive
  row.search-result-keyboard-row:selected {
  background-color: alpha(@m3_primary_container, 0.30);
}

window.theme-material-expressive
  row.search-result-keyboard-row:hover {
  background-color: alpha(@m3_on_surface, 0.05);
}

window.theme-material-expressive
  .search-result-primary-action {
  min-width: 36px;
  min-height: 36px;
  padding: 0;
  border-radius: 999px;
}

window.theme-material-expressive
  .search-result-primary-action.loading {
  opacity: 0.78;
}
'''


def patch_browser(text: str) -> str:
    text = replace_once(
        text,
        """            self.rebuild_search(tracks, config, youtube, query);\n""",
        """            self.rebuild_search(tracks, config, youtube, query, context.playback);\n""",
        "Search receives playback state",
    )

    text = replace_once(
        text,
        """        youtube: &YouTubeLibraryCache,\n        raw_query: &str,\n    ) {\n""",
        """        youtube: &YouTubeLibraryCache,\n        raw_query: &str,\n        playback: &BrowserPlaybackState,\n    ) {\n""",
        "Search rebuild signature",
    )

    for old, new, label in [
        (
            """            loading,\n            copy,\n        ));\n        self.search_content.append(&search_list_section(\n            copy.artists,\n""",
            """            loading,\n            copy,\n            config,\n            playback,\n        ));\n        self.search_content.append(&search_list_section(\n            copy.artists,\n""",
            "Album search action context",
        ),
        (
            """            loading,\n            copy,\n        ));\n        self.search_content.append(&search_list_section(\n            copy.playlists,\n""",
            """            loading,\n            copy,\n            config,\n            playback,\n        ));\n        self.search_content.append(&search_list_section(\n            copy.playlists,\n""",
            "Artist search action context",
        ),
        (
            """            loading,\n            copy,\n        ));\n    }\n\n    fn rebuild_queue(\n""",
            """            loading,\n            copy,\n            config,\n            playback,\n        ));\n    }\n\n    fn rebuild_queue(\n""",
            "Playlist search action context",
        ),
    ]:
        text = replace_once(text, old, new, label)

    text = replace_between(
        text,
        "fn search_list_section(\n",
        "fn search_result_artwork(\n",
        SEARCH_SECTION,
        "Keyboard-first search collection rows",
    )

    focus_helper = r'''fn neutralize_generated_flow_box_focus(widget: &gtk::Widget) {
    let Some(parent) = widget.parent() else {
        return;
    };
    let Ok(child) = parent.downcast::<gtk::FlowBoxChild>() else {
        return;
    };

    child.set_focusable(false);
    child.add_css_class("collection-grid-wrapper");
}

'''
    text = replace_once(
        text,
        "fn append_collection_grid_card<W: IsA<gtk::Widget>>(\n",
        focus_helper + "fn append_collection_grid_card<W: IsA<gtk::Widget>>(\n",
        "FlowBox wrapper focus helper",
    )

    text = replace_once(
        text,
        """        grid.insert(&widget, -1);\n        return;\n""",
        """        grid.insert(&widget, -1);\n        neutralize_generated_flow_box_focus(&widget);\n        return;\n""",
        "Non-animated FlowBox focus fix",
    )
    text = replace_once(
        text,
        """    grid.insert(&widget, -1);\n\n    let widget_weak = widget.downgrade();\n""",
        """    grid.insert(&widget, -1);\n    neutralize_generated_flow_box_focus(&widget);\n\n    let widget_weak = widget.downgrade();\n""",
        "Animated FlowBox focus fix",
    )

    text = replace_once(
        text,
        """    row.set_widget_name(&format!(\"collection-play-row:{}\", spec.widget_key));\n    if spec.is_active {\n""",
        """    row.set_widget_name(&format!(\"collection-play-row:{}\", spec.widget_key));\n    install_row_keyboard_activation(row, spec.open_event.clone(), event_tx);\n    if spec.is_active {\n""",
        "Playlist Enter and Space activation",
    )
    return text


def patch_theme_css(text: str) -> str:
    text = replace_once(
        text,
        '''    (
        "100-buttons.css",
        include_str!("../assets/themes/material-expressive/100-buttons.css"),
    ),
];
''',
        '''    (
        "100-buttons.css",
        include_str!("../assets/themes/material-expressive/100-buttons.css"),
    ),
    (
        "101-keyboard-search.css",
        include_str!("../assets/themes/material-expressive/101-keyboard-search.css"),
    ),
];
''',
        "Register late keyboard/search CSS module",
    )

    text = replace_once(
        text,
        '''            ".collection-action-focusable",
''',
        '''            ".collection-action-focusable",
            ".collection-grid-wrapper",
            ".search-result-keyboard-row",
            ".search-result-primary-action",
''',
        "Keyboard/search CSS contract",
    )

    ordering_test = r'''    #[test]
    fn keyboard_search_css_loads_after_button_rules() {
        let names = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        let buttons = names
            .iter()
            .position(|name| *name == "100-buttons.css")
            .expect("button module should be registered");
        let keyboard = names
            .iter()
            .position(|name| *name == "101-keyboard-search.css")
            .expect("keyboard/search module should be registered");
        assert!(keyboard > buttons);
    }

'''
    return replace_once(
        text,
        "    #[test]\n    fn material_button_css_does_not_style_noctalia() {\n",
        ordering_test + "    #[test]\n    fn material_button_css_does_not_style_noctalia() {\n",
        "Keyboard CSS cascade-order test",
    )


def patch_roadmap(text: str) -> str:
    text = replace_once(
        text,
        "- 🟡 Compact search-result actions and keyboard-first result navigation.\n",
        "- 🟡 Remote search pagination and cache expiration.\n",
        "Advance active search checkpoint",
    )
    text = replace_once(
        text,
        """- Local and YouTube results remain source-aware.\n\n### Remaining\n""",
        """- Local and YouTube results remain source-aware.\n- ✅ Album and playlist results expose one compact play/pause action.\n- ✅ Collection-result rows support arrow navigation and Enter/Space activation.\n\n### Remaining\n""",
        "Search implementation status",
    )
    return replace_once(
        text,
        "- Keyboard-first result navigation.\n",
        "",
        "Remove completed search keyboard item",
    )


def patch_audit(text: str) -> str:
    text = replace_once(
        text,
        """- track rows already provide track-level actions through the queue/action menu;\n- album, artist and playlist result rows are navigation-only;\n- loading uses section headings, status banners and empty/searching rows.\n\nRecommended follow-up:\n\n- keep search rows compact;\n- add only the most useful trailing action rather than reproducing the complete\n  Home overlay;\n- prioritize keyboard-first navigation before adding several icon-only actions.\n""",
        """- track rows already provide track-level actions through the queue/action menu;\n- album and playlist results expose one compact play/pause action;\n- artist results remain navigation-only;\n- collection rows use arrow navigation and Enter/Space activation;\n- loading uses section headings, status banners and empty/searching rows.\n\nRecommended follow-up:\n\n- keep search rows compact and avoid reproducing the complete Home overlay;\n- implement true remote pagination and an expiring result cache;\n- announce result refreshes only if a future assistive-technology pass requires it.\n""",
        "Search action and keyboard audit",
    )
    return replace_once(
        text,
        "6. add compact trailing actions to search results without copying the complete card cluster.\n",
        "6. ✅ add compact trailing actions and keyboard navigation to search results.\n",
        "Search checkpoint implementation order",
    )


def main() -> int:
    required = [BROWSER, THEME_CSS, ROADMAP, AUDIT]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    if KEYBOARD_CSS.exists() and KEYBOARD_CSS.read_text(encoding="utf-8") != KEYBOARD_CSS_CONTENT:
        print(f"ERROR: {KEYBOARD_CSS} already exists with different content.", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    if "struct CollectionActionSpec" not in original[BROWSER]:
        print("ERROR: collection-page actions must be applied first.", file=sys.stderr)
        return 1

    updated = dict(original)
    try:
        updated[BROWSER] = patch_browser(updated[BROWSER])
        updated[THEME_CSS] = patch_theme_css(updated[THEME_CSS])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
        updated[AUDIT] = patch_audit(updated[AUDIT])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed: list[Path] = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    if not KEYBOARD_CSS.exists():
        KEYBOARD_CSS.write_text(KEYBOARD_CSS_CONTENT, encoding="utf-8")
        changed.append(KEYBOARD_CSS.relative_to(ROOT))

    print("Keyboard navigation and compact search actions patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
