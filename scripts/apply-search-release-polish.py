#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"
ROADMAP = ROOT / "ROADMAP.md"
DOC = ROOT / "docs/SEARCH_RELEASE_POLISH.md"


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


DOC_SOURCE = r'''# Search release polish

## Scope

This checkpoint closes the categorized search polish pass by making the visible
search controls and result rows expose clearer accessible labels.

## Behavior

- section headings expose their visible title and count/status subtitle as one
  accessible label;
- remote/local status banners expose warning and detail text as one accessible
  label;
- local and remote "load more" buttons expose the same action text to assistive
  technologies;
- collection result rows expose title, secondary text and source as a compact
  accessible label;
- the previous categorized result summary remains attached to the search results
  container.

## Non-goals

This does not change source separation, ranking, pagination behavior or the
visual layout of search results.
'''

HELPER = r'''fn search_result_row_accessible_label(title: &str, secondary: &str, source: &str) -> String {
    let mut parts = vec![title.trim().to_string()];
    if !secondary.trim().is_empty() {
        parts.push(secondary.trim().to_string());
    }
    if !source.trim().is_empty() {
        parts.push(source.trim().to_string());
    }
    parts.join(". ")
}

'''

TESTS = r'''
#[cfg(test)]
mod search_release_polish_tests {
    use super::search_result_row_accessible_label;

    #[test]
    fn row_accessible_label_keeps_title_secondary_and_source() {
        assert_eq!(
            search_result_row_accessible_label("Hysteria", "Muse", "YouTube"),
            "Hysteria. Muse. YouTube"
        );
    }

    #[test]
    fn row_accessible_label_skips_empty_secondary() {
        assert_eq!(
            search_result_row_accessible_label("Origin of Symmetry", "", "Local"),
            "Origin of Symmetry. Local"
        );
    }
}
'''


def patch_browser(text: str) -> str:
    text = replace_once(
        text,
        "fn search_section_heading(\n",
        HELPER + "fn search_section_heading(\n",
        "Add pure result-row accessible label helper",
    )

    text = replace_once(
        text,
        '''    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.add_css_class("search-section-heading");
    heading.append(&title_label);
    heading.append(&subtitle_label);
    heading
}
''',
        '''    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.add_css_class("search-section-heading");
    heading.append(&title_label);
    heading.append(&subtitle_label);
    let accessible_label = format!("{title}. {subtitle}");
    heading.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
    heading
}
''',
        "Expose section heading title and count to accessibility",
    )

    text = replace_once(
        text,
        '''    if is_error && !detail.trim().is_empty() {
        let detail_label = gtk::Label::new(Some(detail));
        detail_label.set_xalign(0.0);
        detail_label.set_wrap(true);
        detail_label.add_css_class("dim-label");
        banner.append(&detail_label);
    }

    banner
}
''',
        '''    if is_error && !detail.trim().is_empty() {
        let detail_label = gtk::Label::new(Some(detail));
        detail_label.set_xalign(0.0);
        detail_label.set_wrap(true);
        detail_label.add_css_class("dim-label");
        banner.append(&detail_label);
    }

    let accessible_label = if is_error && !detail.trim().is_empty() {
        format!("{message}. {detail}")
    } else {
        message.to_string()
    };
    banner.update_property(&[gtk::accessible::Property::Label(&accessible_label)]);
    banner
}
''',
        "Expose search status banners to accessibility",
    )

    text = replace_once(
        text,
        '''    let next = remaining.min(SEARCH_BATCH_SIZE);
    let button = gtk::Button::with_label(&format!("{} {next} {category}", copy.load_more));
    button.set_halign(gtk::Align::Start);
''',
        '''    let next = remaining.min(SEARCH_BATCH_SIZE);
    let label = format!("{} {next} {category}", copy.load_more);
    let button = gtk::Button::with_label(&label);
    button.set_halign(gtk::Align::Start);
    button.update_property(&[gtk::accessible::Property::Label(&label)]);
''',
        "Expose local search load-more label to accessibility",
    )

    text = replace_once(
        text,
        '''    let button = gtk::Button::with_label(&label);
    button.set_halign(gtk::Align::Start);
    button.set_sensitive(!loading);
    button.add_css_class("search-remote-more");
''',
        '''    let button = gtk::Button::with_label(&label);
    button.set_halign(gtk::Align::Start);
    button.set_sensitive(!loading);
    button.update_property(&[gtk::accessible::Property::Label(&label)]);
    button.add_css_class("search-remote-more");
''',
        "Expose remote search load-more label to accessibility",
    )

    text = replace_once(
        text,
        '''    let source = gtk::Label::new(Some(if online { "YouTube" } else { "Local" }));
    source.add_css_class("pill");
''',
        '''    let source_text = if online { "YouTube" } else { "Local" };
    let row_accessible_label = search_result_row_accessible_label(title, secondary, source_text);
    let source = gtk::Label::new(Some(source_text));
    source.add_css_class("pill");
''',
        "Build descriptive result-row accessible label",
    )

    text = replace_once(
        text,
        '''    row.add_css_class("search-result-keyboard-row");
    install_row_keyboard_activation(&row, card.open_event(), event_tx);
    row
}
''',
        '''    row.add_css_class("search-result-keyboard-row");
    row.update_property(&[gtk::accessible::Property::Label(&row_accessible_label)]);
    install_row_keyboard_activation(&row, card.open_event(), event_tx);
    row
}
''',
        "Expose result rows to accessibility",
    )

    if "mod search_release_polish_tests" not in text:
        text += TESTS
        print("[changed] Add search release polish tests")
    else:
        print("[already applied] Add search release polish tests")

    return text


def patch_roadmap(text: str) -> str:
    active_candidates = [
        "- 🟡 Final search release polish and keyboard/a11y audit.\n",
        "- 🟡 Search result update announcements and accessibility polish.\n",
    ]
    for candidate in active_candidates:
        if candidate in text:
            text = text.replace(
                candidate,
                "- 🟡 Expressive buttons and button-state motion.\n",
                1,
            )
            print("[changed] Advance active checkpoint beyond search polish")
            break
    else:
        print("[already applied] Advance active checkpoint beyond search polish")

    anchors = [
        "- ✅ Accessible search-result summaries update after each categorized result rebuild.\n",
        "- ✅ Route-aware cancellation for stale YouTube Music search responses.\n",
        "- ✅ Collection-result rows support arrow navigation and Enter/Space activation.\n",
    ]
    anchor = next((candidate for candidate in anchors if candidate in text), None)
    if anchor is None:
        raise PatchError("Document completed search release polish: anchor not found")
    completed = "- ✅ Search rows, result headings, status banners and pagination controls expose descriptive accessible labels.\n"
    if completed not in text:
        text = text.replace(anchor, anchor + completed, 1)
        print("[changed] Document completed search release polish")
    else:
        print("[already applied] Document completed search release polish")

    for remaining in [
        "- Final search release polish and keyboard/a11y audit.\n",
        "- Accessibility announcements when results update.\n",
    ]:
        if remaining in text:
            text = text.replace(remaining, "", 1)
            print(f"[changed] Remove completed item: {remaining.strip()[2:]}")

    for order in [
        "8. Complete final search release polish and accessibility audit.\n",
        "8. Add search result announcements and release polish.\n",
    ]:
        if order in text:
            text = text.replace(
                order,
                "8. Continue with expressive button-state motion.\n",
                1,
            )
            print("[changed] Advance recommended implementation order")
            break
    else:
        print("[already applied] Advance recommended implementation order")

    return text


def main() -> int:
    required = [BROWSER, ROADMAP]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    if "search_results_announcement" not in original[BROWSER]:
        print(
            "ERROR: apply and validate the accessible search result summaries checkpoint first.",
            file=sys.stderr,
        )
        return 1
    if DOC.exists() and DOC.read_text(encoding="utf-8") != DOC_SOURCE:
        print(f"ERROR: {DOC} already exists with different content. No files were written.", file=sys.stderr)
        return 1

    updated = dict(original)
    try:
        updated[BROWSER] = patch_browser(updated[BROWSER])
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

    if not DOC.exists():
        DOC.write_text(DOC_SOURCE, encoding="utf-8")
        changed.append(DOC.relative_to(ROOT))

    print("Search release polish patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
