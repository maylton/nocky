#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"
CSS = ROOT / "assets/themes/material-expressive/080-home-browser.css"
THEME_CSS = ROOT / "src/theme_css.rs"
ROADMAP = ROOT / "ROADMAP.md"


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


def patch_browser(text: str) -> str:
    text = replace_once(
        text,
        """        let youtube_home = active_source == ListeningSource::YouTube;\n\n        let next_home = gtk::Box::new(gtk::Orientation::Vertical, 22);\n""",
        """        let youtube_home = active_source == ListeningSource::YouTube;\n        let show_youtube_loading_placeholders =\n            should_show_youtube_home_loading_placeholders(\n                youtube_home,\n                youtube_home_loading,\n                youtube_home_page.sections.len(),\n            );\n\n        let next_home = gtk::Box::new(gtk::Orientation::Vertical, 22);\n""",
        "placeholder condition",
    )
    text = replace_once(
        text,
        """        next_home.add_css_class(\"library-home\");\n        next_home.add_css_class(\"expressive-library-home\");\n\n        if youtube_home && !youtube_home_page.sections.is_empty() {\n""",
        """        next_home.add_css_class(\"library-home\");\n        next_home.add_css_class(\"expressive-library-home\");\n\n        if show_youtube_loading_placeholders {\n            next_home.add_css_class(\"youtube-home-loading-placeholders\");\n            next_home.append(&youtube_home_loading_banner(youtube_home_page, language));\n            next_home.append(&home_loading_placeholder_section(\n                copy.mixtapes_title,\n                HomeSectionPresentation::Featured,\n                language,\n            ));\n            next_home.append(&home_loading_placeholder_section(\n                copy.albums_title,\n                HomeSectionPresentation::Compact,\n                language,\n            ));\n            next_home.append(&home_loading_placeholder_section(\n                copy.playlists_title,\n                HomeSectionPresentation::Compact,\n                language,\n            ));\n        }\n\n        if !show_youtube_loading_placeholders\n            && youtube_home\n            && !youtube_home_page.sections.is_empty()\n        {\n""",
        "placeholder sections",
    )
    text = replace_once(
        text,
        """        if matches!(config.startup_source, Some(StartupSource::YouTube)) {\n""",
        """        if !show_youtube_loading_placeholders {\n            if matches!(config.startup_source, Some(StartupSource::YouTube)) {\n""",
        "legacy Home guard start",
    )
    text = replace_once(
        text,
        """        if youtube_home && youtube.syncing {\n            next_home.append(&home_syncing_hint(language));\n        }\n\n        let generation = self.home_generation.get().wrapping_add(1);\n""",
        """            if youtube_home && youtube.syncing {\n                next_home.append(&home_syncing_hint(language));\n            }\n        }\n\n        let generation = self.home_generation.get().wrapping_add(1);\n""",
        "legacy Home guard end",
    )
    helper = r'''fn should_show_youtube_home_loading_placeholders(
    youtube_home: bool,
    loading: bool,
    section_count: usize,
) -> bool {
    youtube_home && loading && section_count == 0
}

fn home_loading_placeholder_section(
    title: &str,
    presentation: HomeSectionPresentation,
    language: AppLanguage,
) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("home-section-title");

    let subtitle = match language {
        AppLanguage::Portuguese => "Preparando recomendações…",
        AppLanguage::English => "Preparing recommendations…",
        AppLanguage::Spanish => "Preparando recomendaciones…",
    };
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.add_css_class("dim-label");

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.set_vexpand(false);
    heading.set_valign(gtk::Align::Start);
    heading.add_css_class("home-section-heading");
    heading.append(&title_label);
    heading.append(&subtitle_label);

    let count = match presentation {
        HomeSectionPresentation::Featured => 5,
        HomeSectionPresentation::Compact => 7,
        HomeSectionPresentation::TrackRows => 4,
    };
    let cards = (0..count)
        .map(|_| home_loading_placeholder_card(presentation))
        .collect::<Vec<_>>();
    let content = metrolist_home_section_content(cards, presentation, language, "", false);
    content.set_vexpand(false);
    content.set_valign(gtk::Align::Start);

    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    section.set_vexpand(false);
    section.set_valign(gtk::Align::Start);
    section.add_css_class("home-section");
    section.add_css_class(presentation.css_class());
    section.add_css_class("home-loading-placeholder-section");
    section.append(&heading);
    section.append(&content);
    section
}

fn home_loading_placeholder_card(presentation: HomeSectionPresentation) -> gtk::Widget {
    let card = home_collection_card(
        None,
        "\u{00a0}",
        "\u{00a0}",
        "\u{00a0}",
        false,
        presentation,
    );
    card.add_css_class("home-card");
    card.add_css_class("home-card-loading-placeholder");
    card.add_css_class("collection-card-skeleton");

    let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    slot.set_size_request(presentation.outer_width(), presentation.outer_height());
    slot.set_hexpand(false);
    slot.set_vexpand(false);
    slot.set_halign(gtk::Align::Start);
    slot.set_valign(gtk::Align::Start);
    slot.add_css_class("home-card-loading-slot");
    slot.append(&card);
    slot.upcast::<gtk::Widget>()
}

#[cfg(test)]
mod youtube_home_loading_placeholder_tests {
    use super::should_show_youtube_home_loading_placeholders;

    #[test]
    fn placeholders_require_youtube_loading_with_no_sections() {
        assert!(should_show_youtube_home_loading_placeholders(true, true, 0));
        assert!(!should_show_youtube_home_loading_placeholders(false, true, 0));
        assert!(!should_show_youtube_home_loading_placeholders(true, false, 0));
        assert!(!should_show_youtube_home_loading_placeholders(true, true, 1));
    }
}

'''
    return replace_once(
        text,
        "fn youtube_feed_section_cards(\n",
        helper + "fn youtube_feed_section_cards(\n",
        "placeholder helpers and tests",
    )


def patch_css(text: str) -> str:
    marker = "/* Dedicated first-paint rails for an empty remote YouTube Home. */"
    if marker in text:
        print("[already applied] placeholder CSS")
        return text
    addition = r'''

/* Dedicated first-paint rails for an empty remote YouTube Home. */
window.theme-material-expressive
  .youtube-home-loading-placeholders
  .home-card-loading-slot {
  opacity: 0.84;
}

window.theme-material-expressive
  .youtube-home-loading-placeholders
  .home-card-loading-placeholder {
  background-color: @m3_surface_container;
  border-color: alpha(@m3_outline_variant, 0.32);
  box-shadow: none;
}

window.theme-material-expressive
  .youtube-home-loading-placeholders
  .home-card-loading-placeholder
  .collection-artwork {
  background-color: @m3_surface_container_highest;
  box-shadow: inset 0 0 0 1px alpha(@m3_outline_variant, 0.18);
}

window.theme-material-expressive
  .youtube-home-loading-placeholders
  .home-card-loading-placeholder
  .cover-icon {
  opacity: 0;
}

window.theme-material-expressive
  .youtube-home-loading-placeholders
  .home-card-loading-placeholder
  .collection-card-title,
window.theme-material-expressive
  .youtube-home-loading-placeholders
  .home-card-loading-placeholder
  .collection-card-subtitle,
window.theme-material-expressive
  .youtube-home-loading-placeholders
  .home-card-loading-placeholder
  .collection-card-detail {
  min-height: 10px;
  color: transparent;
  background-color: alpha(@m3_on_surface_variant, 0.14);
  border-radius: 999px;
}
'''
    print("[changed] placeholder CSS")
    return text.rstrip() + addition + "\n"


def patch_theme_tests(text: str) -> str:
    return replace_once(
        text,
        """            \".material-carousel-edge-spring\",\n            \".material-carousel-edge-spring-surface\",\n""",
        """            \".material-carousel-edge-spring\",\n            \".material-carousel-edge-spring-surface\",\n            \".youtube-home-loading-placeholders\",\n            \".home-card-loading-placeholder\",\n""",
        "placeholder CSS contract",
    )


def patch_roadmap(text: str) -> str:
    text = replace_once(text, "> Last updated: 2026-07-01", "> Last updated: 2026-07-03", "roadmap date")
    text = replace_once(
        text,
        """### Active checkpoint\n\n- 🟡 Reusable native Material Loading Indicator for page, inline/action and\n  load-more loading states.\n""",
        """### Active checkpoint\n\n- 🟡 Card actions and loading states: dedicated first-paint Home placeholder\n  rails, reusable action overlays for collection grids and accessibility\n  validation.\n""",
        "active roadmap checkpoint",
    )
    text = replace_once(
        text,
        """- Current playback information on media rows.\n\n### Remaining\n\n- Contextual play button on collection cards.\n- ✅ Favorite action inside each collection card overflow menu.\n- Overflow menu.\n- 🟡 Skeleton treatment during remote loading is implemented on active collection cards; dedicated placeholder rails remain planned.\n""",
        """- Current playback information on media rows.\n- ✅ Contextual play/pause and overflow actions on supported Home collection cards.\n- ✅ Dedicated first-paint placeholder rails for an empty remote YouTube Home.\n\n### Remaining\n\n- Reuse Home play/pause and overflow action overlays on album and playlist collection grids.\n- Keep artist cards navigation-first until deterministic artist queue resolution is available.\n- ✅ Favorite action inside each collection card overflow menu.\n- ✅ Skeleton treatment during remote loading is implemented on active collection cards and initial Home placeholder rails.\n""",
        "card checkpoint roadmap status",
    )
    return replace_once(
        text,
        """6. 🟡 Consolidate Material Expressive loading indicators and visual-system primitives.\n7. Finish card actions and loading placeholders.\n""",
        """6. ✅ Consolidate Material Expressive loading indicators and visual-system primitives.\n7. 🟡 Finish card actions and loading placeholders.\n""",
        "recommended roadmap order",
    )


def main() -> int:
    paths = [BROWSER, CSS, THEME_CSS, ROADMAP]
    missing = [path for path in paths if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in paths}
    updated = dict(original)
    try:
        updated[BROWSER] = patch_browser(updated[BROWSER])
        updated[CSS] = patch_css(updated[CSS])
        updated[THEME_CSS] = patch_theme_tests(updated[THEME_CSS])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed = []
    for path in paths:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Checkpoint applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
