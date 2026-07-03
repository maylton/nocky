#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"
SEARCH_CSS = ROOT / "assets/themes/material-expressive/101-keyboard-search.css"
ROADMAP = ROOT / "ROADMAP.md"
DOC = ROOT / "docs/SEARCH_ACCESSIBILITY.md"


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


ROW_HELPERS = r'''fn search_source_accessible_name(language: AppLanguage, online: bool) -> &'static str {
    match (language, online) {
        (AppLanguage::Portuguese, true) => "YouTube Music",
        (AppLanguage::Portuguese, false) => "biblioteca local",
        (AppLanguage::English, true) => "YouTube Music",
        (AppLanguage::English, false) => "local library",
        (AppLanguage::Spanish, true) => "YouTube Music",
        (AppLanguage::Spanish, false) => "biblioteca local",
    }
}

fn search_result_row_accessible_label(
    language: AppLanguage,
    title: &str,
    secondary: &str,
    detail: &str,
    online: bool,
    has_primary_action: bool,
) -> String {
    let title = title.trim();
    let secondary = secondary.trim();
    let detail = detail.trim();
    let source = search_source_accessible_name(language, online);
    let metadata = if !secondary.is_empty() && !detail.is_empty() && secondary != detail {
        format!("{secondary}. {detail}.")
    } else if !secondary.is_empty() {
        format!("{secondary}.")
    } else if !detail.is_empty() {
        format!("{detail}.")
    } else {
        String::new()
    };

    match language {
        AppLanguage::Portuguese => {
            let action = if has_primary_action {
                "Ação rápida disponível."
            } else {
                "Pressione Enter para abrir."
            };
            format!("{title}. {metadata} Fonte: {source}. {action}")
        }
        AppLanguage::English => {
            let action = if has_primary_action {
                "Quick action available."
            } else {
                "Press Enter to open."
            };
            format!("{title}. {metadata} Source: {source}. {action}")
        }
        AppLanguage::Spanish => {
            let action = if has_primary_action {
                "Acción rápida disponible."
            } else {
                "Presiona Enter para abrir."
            };
            format!("{title}. {metadata} Fuente: {source}. {action}")
        }
    }
}

'''

TESTS = r'''
#[cfg(test)]
mod search_keyboard_a11y_polish_tests {
    use super::*;

    #[test]
    fn row_accessible_label_mentions_source_and_quick_action_in_portuguese() {
        let label = search_result_row_accessible_label(
            AppLanguage::Portuguese,
            "Absolution",
            "Muse",
            "Álbum • YouTube Music",
            true,
            true,
        );
        assert!(label.contains("Absolution"));
        assert!(label.contains("Fonte: YouTube Music"));
        assert!(label.contains("Ação rápida disponível"));
    }

    #[test]
    fn row_accessible_label_keeps_local_source_in_english() {
        let label = search_result_row_accessible_label(
            AppLanguage::English,
            "The Bends",
            "Radiohead",
            "Local • 12 tracks",
            false,
            false,
        );
        assert!(label.contains("Source: local library"));
        assert!(label.contains("Press Enter to open"));
    }

    #[test]
    fn row_accessible_label_deduplicates_secondary_and_detail() {
        let label = search_result_row_accessible_label(
            AppLanguage::Spanish,
            "Playlist",
            "YouTube Music",
            "YouTube Music",
            true,
            false,
        );
        assert_eq!(label.matches("YouTube Music.").count(), 1);
        assert!(label.contains("Fuente: YouTube Music"));
    }
}
'''

DOC_APPEND = r'''

## Keyboard and row-label polish

The final search accessibility polish adds explicit accessible labels to
collection-result rows. The labels include title, secondary metadata, source and
whether a quick action is available, so keyboard and screen-reader users do not
have to infer the row purpose from visual badges alone.

The Material Expressive keyboard CSS also treats focus inside a row the same as
focus on the row itself. This keeps the visible ring stable when users tab from a
row to its compact play/pause action.
'''

CSS_APPEND = r'''

window.theme-material-expressive
  row.search-result-keyboard-row:focus-within {
  box-shadow:
    0 0 0 3px alpha(@m3_primary, 0.30),
    0 8px 20px alpha(black, 0.16);
}

window.theme-material-expressive
  .search-result-primary-action:focus {
  box-shadow:
    0 0 0 3px alpha(@m3_primary, 0.28),
    0 6px 16px alpha(black, 0.14);
}
'''


def patch_browser(text: str) -> str:
    if "fn search_result_row_accessible_label(" not in text:
        text = replace_once(
            text,
            "fn search_section_heading(\n",
            ROW_HELPERS + "fn search_section_heading(\n",
            "Add accessible row label helpers",
        )
    else:
        print("[already applied] Add accessible row label helpers")

    text = replace_once(
        text,
        '''    let leading = search_result_artwork(cover_path, icon_name);

    let title_label = gtk::Label::new(Some(title));
''',
        '''    let leading = search_result_artwork(cover_path, icon_name);

    let title_label = gtk::Label::new(Some(title));
''',
        "Keep artwork setup stable",
    )
    text = replace_once(
        text,
        '''    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
''',
        '''    subtitle_label.add_css_class("dim-label");

    let primary_action_spec = search_collection_action_spec(card, config, playback);
    let accessible_label = search_result_row_accessible_label(
        config.language,
        title,
        secondary,
        detail,
        online,
        primary_action_spec.is_some(),
    );

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
''',
        "Build accessible label for search result rows",
    )
    text = replace_once(
        text,
        '''    if let Some(spec) = search_collection_action_spec(card, config, playback) {
''',
        '''    if let Some(spec) = primary_action_spec {
''',
        "Reuse action spec for row labels and quick action",
    )
    text = replace_once(
        text,
        '''    row.add_css_class("search-result-keyboard-row");
    install_row_keyboard_activation(&row, card.open_event(), event_tx);
''',
        '''    row.add_css_class("search-result-keyboard-row");
    row.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
    install_row_keyboard_activation(&row, card.open_event(), event_tx);
''',
        "Expose search result row accessible labels",
    )

    if "mod search_keyboard_a11y_polish_tests" not in text:
        text += TESTS
        print("[changed] Add keyboard/a11y row label tests")
    else:
        print("[already applied] Add keyboard/a11y row label tests")
    return text


def patch_css(text: str) -> str:
    if "row.search-result-keyboard-row:focus-within" in text:
        print("[already applied] Add focus-within styling for search rows")
        return text
    print("[changed] Add focus-within styling for search rows")
    return text.rstrip() + CSS_APPEND + "\n"


def patch_doc(text: str) -> str:
    if "## Keyboard and row-label polish" in text:
        print("[already applied] Document keyboard and row-label polish")
        return text
    print("[changed] Document keyboard and row-label polish")
    return text.rstrip() + DOC_APPEND + "\n"


def patch_roadmap(text: str) -> str:
    active_candidates = [
        "- 🟡 Final search release polish and keyboard/a11y audit.\n",
        "- 🟡 Search result update announcements and accessibility polish.\n",
    ]
    active_matches = [candidate for candidate in active_candidates if candidate in text]
    if active_matches:
        text = text.replace(
            active_matches[0],
            "- 🟡 Menus and contextual surfaces.\n",
            1,
        )
        print("[changed] Advance active visual checkpoint to contextual surfaces")
    else:
        print("[already applied] Advance active visual checkpoint to contextual surfaces")

    anchors = [
        "- ✅ Accessible search-result summaries update after each categorized result rebuild.\n",
        "- ✅ Collection-result rows support arrow navigation and Enter/Space activation.\n",
        "- ✅ Route-aware cancellation for stale YouTube Music search responses.\n",
    ]
    anchor = next((candidate for candidate in anchors if candidate in text), None)
    if anchor is None:
        raise PatchError("Document completed search keyboard/a11y polish: anchor not found")
    completed = "- ✅ Search result rows expose source-aware accessible labels and stable focus-within rings.\n"
    if completed not in text:
        text = text.replace(anchor, anchor + completed, 1)
        print("[changed] Document completed search keyboard/a11y polish")
    else:
        print("[already applied] Document completed search keyboard/a11y polish")

    remaining = "- Accessibility announcements when results update.\n"
    if remaining in text:
        text = text.replace(remaining, "", 1)
        print("[changed] Remove completed accessibility item from search remaining")

    return text


def main() -> int:
    required = [BROWSER, SEARCH_CSS, ROADMAP, DOC]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root after applying the search result announcements checkpoint.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    if "fn search_results_announcement(" not in original[BROWSER]:
        print("ERROR: apply and validate apply-search-result-announcements.py first.", file=sys.stderr)
        return 1
    if "Search result accessibility announcements" not in original[DOC]:
        print("ERROR: docs/SEARCH_ACCESSIBILITY.md does not look like the expected accessibility doc.", file=sys.stderr)
        return 1

    updated = dict(original)
    try:
        updated[BROWSER] = patch_browser(updated[BROWSER])
        updated[SEARCH_CSS] = patch_css(updated[SEARCH_CSS])
        updated[DOC] = patch_doc(updated[DOC])
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

    print("Search keyboard/accessibility polish patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
