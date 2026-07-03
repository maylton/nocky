#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"
CSS = ROOT / "assets/themes/material-expressive/080-home-browser.css"
THEME_CSS = ROOT / "src/theme_css.rs"
ROADMAP = ROOT / "ROADMAP.md"
AUDIT = ROOT / "docs/CARD_ACTIONS_LOADING_AUDIT.md"
MATERIAL_DOC = ROOT / "docs/MATERIAL_EXPRESSIVE_CARDS_CAROUSELS.md"


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


ACCESSIBILITY_HELPERS = r'''fn collection_action_title(spec: &CollectionActionSpec) -> &str {
    match &spec.open_event {
        BrowserEvent::Navigate(BrowserRoute::Album(title))
        | BrowserEvent::Navigate(BrowserRoute::Playlist(title)) => title,
        BrowserEvent::OpenYouTubeCollection(item)
        | BrowserEvent::OpenYouTubePlaylist(item) => &item.title,
        _ => "",
    }
}

fn collection_action_kind(spec: &CollectionActionSpec) -> &'static str {
    match &spec.open_event {
        BrowserEvent::Navigate(BrowserRoute::Album(_))
        | BrowserEvent::OpenYouTubeCollection(_) => "album",
        BrowserEvent::Navigate(BrowserRoute::Playlist(_))
        | BrowserEvent::OpenYouTubePlaylist(_) => "playlist",
        _ => "collection",
    }
}

fn localized_collection_kind(language: AppLanguage, kind: &str) -> &'static str {
    match (language, kind) {
        (AppLanguage::Portuguese, "album") => "álbum",
        (AppLanguage::Portuguese, "playlist") => "playlist",
        (AppLanguage::Portuguese, _) => "coleção",
        (AppLanguage::English, "album") => "album",
        (AppLanguage::English, "playlist") => "playlist",
        (AppLanguage::English, _) => "collection",
        (AppLanguage::Spanish, "album") => "álbum",
        (AppLanguage::Spanish, "playlist") => "playlist",
        (AppLanguage::Spanish, _) => "colección",
    }
}

fn collection_open_accessible_label(
    language: AppLanguage,
    kind: &str,
    title: &str,
) -> String {
    let kind = localized_collection_kind(language, kind);
    match language {
        AppLanguage::Portuguese => format!("Abrir {kind}: {title}"),
        AppLanguage::English => format!("Open {kind}: {title}"),
        AppLanguage::Spanish => format!("Abrir {kind}: {title}"),
    }
}

fn collection_play_accessible_label(
    language: AppLanguage,
    title: &str,
    is_active: bool,
    playing: bool,
) -> String {
    match (language, is_active, playing) {
        (AppLanguage::Portuguese, true, true) => format!("Pausar: {title}"),
        (AppLanguage::Portuguese, true, false) => format!("Continuar: {title}"),
        (AppLanguage::Portuguese, false, _) => format!("Reproduzir: {title}"),
        (AppLanguage::English, true, true) => format!("Pause: {title}"),
        (AppLanguage::English, true, false) => format!("Resume: {title}"),
        (AppLanguage::English, false, _) => format!("Play: {title}"),
        (AppLanguage::Spanish, true, true) => format!("Pausar: {title}"),
        (AppLanguage::Spanish, true, false) => format!("Continuar: {title}"),
        (AppLanguage::Spanish, false, _) => format!("Reproducir: {title}"),
    }
}

fn collection_loading_accessible_label(language: AppLanguage, title: &str) -> String {
    match language {
        AppLanguage::Portuguese => format!("Carregando coleção: {title}"),
        AppLanguage::English => format!("Loading collection: {title}"),
        AppLanguage::Spanish => format!("Cargando colección: {title}"),
    }
}

fn collection_more_options_accessible_label(
    language: AppLanguage,
    title: &str,
) -> String {
    match language {
        AppLanguage::Portuguese => format!("Mais opções para: {title}"),
        AppLanguage::English => format!("More options for: {title}"),
        AppLanguage::Spanish => format!("Más opciones para: {title}"),
    }
}

fn collection_menu_accessible_label(label: &str, title: &str) -> String {
    format!("{label}: {title}")
}

'''


ACCESSIBILITY_TESTS = r'''#[cfg(test)]
mod collection_action_accessibility_tests {
    use super::*;

    #[test]
    fn collection_action_labels_include_the_collection_title() {
        let title = "Discovery";
        assert_eq!(
            collection_open_accessible_label(AppLanguage::English, "album", title),
            "Open album: Discovery"
        );
        assert_eq!(
            collection_play_accessible_label(AppLanguage::Portuguese, title, false, false),
            "Reproduzir: Discovery"
        );
        assert_eq!(
            collection_more_options_accessible_label(AppLanguage::Spanish, title),
            "Más opciones para: Discovery"
        );
    }

    #[test]
    fn active_collection_labels_distinguish_pause_and_resume() {
        assert_eq!(
            collection_play_accessible_label(AppLanguage::English, "Mix", true, true),
            "Pause: Mix"
        );
        assert_eq!(
            collection_play_accessible_label(AppLanguage::English, "Mix", true, false),
            "Resume: Mix"
        );
    }
}

'''


def patch_browser(text: str) -> str:
    text = replace_once(
        text,
        "fn collection_loading_label(language: AppLanguage) -> &'static str {\n",
        ACCESSIBILITY_HELPERS + "fn collection_loading_label(language: AppLanguage) -> &'static str {\n",
        "Accessibility label helpers",
    )

    text = replace_once(
        text,
        '''    if spec.is_loading {
        let label = collection_loading_label(language);
        let loading = MaterialLoadingIndicator::compact();
        loading
            .widget()
            .update_property(&[gtk::accessible::Property::Label(label)]);
        control.set_child(Some(loading.widget()));
        control.set_sensitive(false);
        control.add_css_class("loading");
        control.set_tooltip_text(Some(label));
        control.update_property(&[gtk::accessible::Property::Label(label)]);
        return control;
    }

    let tooltip = collection_play_tooltip(language, spec.is_active, spec.playing);
''',
        '''    let title = collection_action_title(spec);
    control.set_focusable(true);
    control.add_css_class("collection-action-focusable");

    if spec.is_loading {
        let tooltip = collection_loading_label(language);
        let accessible_label = collection_loading_accessible_label(language, title);
        let loading = MaterialLoadingIndicator::compact();
        loading
            .widget()
            .update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
        control.set_child(Some(loading.widget()));
        control.set_sensitive(false);
        control.add_css_class("loading");
        control.set_tooltip_text(Some(tooltip));
        control.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
        return control;
    }

    let tooltip = collection_play_tooltip(language, spec.is_active, spec.playing);
    let accessible_label =
        collection_play_accessible_label(language, title, spec.is_active, spec.playing);
''',
        "Primary action accessible loading state",
    )

    text = replace_once(
        text,
        '''    control.set_tooltip_text(Some(tooltip));
    control.update_property(&[gtk::accessible::Property::Label(tooltip)]);
''',
        '''    control.set_tooltip_text(Some(tooltip));
    control.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
''',
        "Primary action contextual accessible name",
    )

    text = replace_once(
        text,
        '''fn collection_menu_action_button(
    label: &str,
    icon_name: &str,
    selected: bool,
    event: BrowserEvent,
''',
        '''fn collection_menu_action_button(
    label: &str,
    accessible_label: String,
    icon_name: &str,
    selected: bool,
    event: BrowserEvent,
''',
        "Menu action accessible label parameter",
    )

    text = replace_once(
        text,
        '''    button.add_css_class("material-card-menu-action");
    if selected {
''',
        '''    button.add_css_class("material-card-menu-action");
    button.set_focusable(true);
    button.add_css_class("collection-action-focusable");
    button.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
    if selected {
''',
        "Menu action focus and accessible name",
    )

    text = replace_once(
        text,
        '''    let more_options_label = match language {
        AppLanguage::Portuguese => "Mais opções",
        AppLanguage::English => "More options",
        AppLanguage::Spanish => "Más opciones",
    };
    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(more_options_label)
        .build();
    menu.update_property(&[gtk::accessible::Property::Label(more_options_label)]);
''',
        '''    let title = collection_action_title(spec);
    let more_options_label = collection_more_options_accessible_label(language, title);
    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(more_options_label.as_str())
        .build();
    menu.update_property(&[gtk::accessible::Property::Label(&more_options_label)]);
    menu.set_focusable(true);
    menu.add_css_class("collection-action-focusable");
''',
        "Overflow contextual accessible name",
    )

    replacements = [
        (
            '''        collection_menu_action_button(
            labels.0,
            "media-skip-forward-symbolic",''',
            '''        collection_menu_action_button(
            labels.0,
            collection_menu_accessible_label(labels.0, title),
            "media-skip-forward-symbolic",''',
            "Play-next menu accessible name",
        ),
        (
            '''        collection_menu_action_button(
            labels.1,
            "list-add-symbolic",''',
            '''        collection_menu_action_button(
            labels.1,
            collection_menu_accessible_label(labels.1, title),
            "list-add-symbolic",''',
            "Append menu accessible name",
        ),
        (
            '''        collection_menu_action_button(
            labels.2,
            "go-next-symbolic",''',
            '''        collection_menu_action_button(
            labels.2,
            collection_menu_accessible_label(labels.2, title),
            "go-next-symbolic",''',
            "Open menu accessible name",
        ),
        (
            '''        collection_menu_action_button(
            labels.3,
            if spec.favorite_selected {''',
            '''        collection_menu_action_button(
            labels.3,
            collection_menu_accessible_label(labels.3, title),
            if spec.favorite_selected {''',
            "Favorite menu accessible name",
        ),
        (
            '''        let button = collection_menu_action_button(
            label,
            "folder-download-symbolic",''',
            '''        let button = collection_menu_action_button(
            label,
            collection_menu_accessible_label(label, title),
            "folder-download-symbolic",''',
            "Offline menu accessible name",
        ),
    ]
    for old, new, label in replacements:
        text = replace_once(text, old, new, label)

    text = replace_once(
        text,
        '''    apply_collection_action_state(&card, &spec);
    let main_button = collection_event_button(card, spec.open_event.clone(), event_tx);

    let overlay = gtk::Overlay::new();
''',
        '''    apply_collection_action_state(&card, &spec);
    let main_button = collection_event_button(card, spec.open_event.clone(), event_tx);
    let open_label = collection_open_accessible_label(
        language,
        collection_action_kind(&spec),
        collection_action_title(&spec),
    );
    main_button.set_focusable(true);
    main_button.add_css_class("collection-action-focusable");
    main_button.set_tooltip_text(Some(&open_label));
    main_button.update_property(&[gtk::accessible::Property::Label(&open_label)]);

    let overlay = gtk::Overlay::new();
''',
        "Grid card navigation accessible name",
    )

    text = replace_once(
        text,
        '''    row.add_css_class("playlist-card-row-with-actions");
    row.set_widget_name(&format!("collection-play-row:{}", spec.widget_key));
''',
        '''    let open_label = collection_open_accessible_label(
        language,
        collection_action_kind(spec),
        collection_action_title(spec),
    );
    row.set_focusable(true);
    row.add_css_class("collection-action-focusable");
    row.update_property(&[gtk::accessible::Property::Label(&open_label)]);
    row.add_css_class("playlist-card-row-with-actions");
    row.set_widget_name(&format!("collection-play-row:{}", spec.widget_key));
''',
        "Playlist row navigation accessible name",
    )

    return replace_once(
        text,
        "fn collection_button(\n",
        ACCESSIBILITY_TESTS + "fn collection_button(\n",
        "Accessibility unit tests",
    )


def patch_css(text: str) -> str:
    marker = "/* Keyboard focus for collection actions */"
    if marker in text:
        print("[already applied] collection action focus CSS")
        return text

    addition = r'''

/* Keyboard focus for collection actions */
window.theme-material-expressive
  .collection-action-focusable:focus {
  box-shadow:
    0 0 0 3px alpha(@m3_primary, 0.28),
    0 8px 20px alpha(black, 0.16);
}

window.theme-material-expressive
  .playlist-card-row-with-actions.collection-action-focusable:focus {
  background-color: alpha(@m3_primary_container, 0.26);
  box-shadow:
    inset 3px 0 0 @m3_primary,
    0 0 0 2px alpha(@m3_primary, 0.22);
}

window.theme-material-expressive
  .collection-card-overflow-popover
  .collection-action-focusable:focus {
  background-color: alpha(@m3_primary_container, 0.38);
  box-shadow: inset 0 0 0 2px alpha(@m3_primary, 0.24);
}
'''
    print("[changed] collection action focus CSS")
    return text.rstrip() + addition.rstrip() + "\n"


def patch_theme_tests(text: str) -> str:
    return replace_once(
        text,
        '''            ".collection-grid-action-overlay",
            ".playlist-card-row-with-actions",
''',
        '''            ".collection-grid-action-overlay",
            ".playlist-card-row-with-actions",
            ".collection-action-focusable",
''',
        "Collection action focus CSS contract",
    )


def patch_roadmap(text: str) -> str:
    text = replace_once(
        text,
        '''- 🟡 Card actions and loading states: dedicated first-paint Home placeholder
  rails, reusable action overlays for collection grids and accessibility
  validation.
''',
        '''- 🟡 Compact search-result actions and keyboard-first result navigation.
''',
        "Active roadmap checkpoint",
    )
    return replace_once(
        text,
        '''- ✅ Reusable play/pause and overflow actions are applied to album grids and playlist rows.
- Keep artist cards navigation-first until deterministic artist queue resolution is available.
''',
        '''- ✅ Reusable play/pause and overflow actions are applied to album grids and playlist rows.
- ✅ Collection actions have contextual accessible names and visible keyboard focus.
- Keep artist cards navigation-first until deterministic artist queue resolution is available.
''',
        "Accessibility roadmap status",
    )


def patch_audit(text: str) -> str:
    text = replace_once(
        text,
        '''1. validate album and playlist action focus order with keyboard and screen readers;
2. keep artist cards navigation-only until artist queue resolution is explicit;
3. decide separately whether artist-album cards should gain actions after async artist-page refreshes can preserve playback context;
4. keep search rows compact and avoid copying the complete card action cluster there.
''',
        '''1. ✅ contextual accessible names identify both the action and collection title;
2. ✅ visible focus treatment covers navigation, play/pause, overflow and menu actions;
3. keep artist cards navigation-only until artist queue resolution is explicit;
4. decide separately whether artist-album cards should gain actions after async artist-page refreshes can preserve playback context;
5. keep search rows compact and add only keyboard-first trailing actions.
''',
        "Accessibility audit status",
    )
    return replace_once(
        text,
        '''5. audit keyboard and screen-reader behavior before extending search-row actions.
''',
        '''5. ✅ audit keyboard and screen-reader behavior before extending search-row actions;
6. add compact trailing actions to search results without copying the complete card cluster.
''',
        "Audit implementation order",
    )


def patch_material_doc(text: str) -> str:
    return replace_once(
        text,
        '''- state classes `material-card-menu-action-selected`,
  `material-card-menu-action-loading`, and `material-card-menu-action-success`
  for favorite/offline feedback.
''',
        '''- state classes `material-card-menu-action-selected`,
  `material-card-menu-action-loading`, and `material-card-menu-action-success`
  for favorite/offline feedback;
- contextual accessible names include the collection title for navigation,
  play/pause, overflow and menu actions;
- `.collection-action-focusable` provides a visible Material focus treatment
  without changing pointer-hover geometry.
''',
        "Material card accessibility contract",
    )


def main() -> int:
    paths = [BROWSER, CSS, THEME_CSS, ROADMAP, AUDIT, MATERIAL_DOC]
    missing = [path for path in paths if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in paths}
    if "struct CollectionActionSpec" not in original[BROWSER]:
        print(
            "ERROR: apply-collection-page-actions-v2.py must be applied first.",
            file=sys.stderr,
        )
        return 1

    updated = dict(original)
    try:
        updated[BROWSER] = patch_browser(updated[BROWSER])
        updated[CSS] = patch_css(updated[CSS])
        updated[THEME_CSS] = patch_theme_tests(updated[THEME_CSS])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
        updated[AUDIT] = patch_audit(updated[AUDIT])
        updated[MATERIAL_DOC] = patch_material_doc(updated[MATERIAL_DOC])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed = []
    for path in paths:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Collection action accessibility patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
