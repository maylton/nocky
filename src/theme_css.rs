// Material Expressive CSS is embedded as one explicitly named, ordered
// manifest. The numeric filename prefix is the cascade order contract.

const NOCTALIA_CSS: &str = include_str!("../assets/themes/noctalia.css");
const FROSTED_GLASS_CSS: &str = include_str!("../assets/themes/frosted-glass.css");

const MATERIAL_EXPRESSIVE_MODULES: &[(&str, &str)] = &[
    (
        "000-foundation.css",
        include_str!("../assets/themes/material-expressive/000-foundation.css"),
    ),
    (
        "010-footer.css",
        include_str!("../assets/themes/material-expressive/010-footer.css"),
    ),
    (
        "020-navigation.css",
        include_str!("../assets/themes/material-expressive/020-navigation.css"),
    ),
    (
        "030-dialogs-settings.css",
        include_str!("../assets/themes/material-expressive/030-dialogs-settings.css"),
    ),
    (
        "040-dialogs-settings.css",
        include_str!("../assets/themes/material-expressive/040-dialogs-settings.css"),
    ),
    (
        "050-dialogs-settings.css",
        include_str!("../assets/themes/material-expressive/050-dialogs-settings.css"),
    ),
    (
        "060-dialogs-settings.css",
        include_str!("../assets/themes/material-expressive/060-dialogs-settings.css"),
    ),
    (
        "070-player.css",
        include_str!("../assets/themes/material-expressive/070-player.css"),
    ),
    (
        "080-home-browser.css",
        include_str!("../assets/themes/material-expressive/080-home-browser.css"),
    ),
    (
        "081-carousel-motion.css",
        include_str!("../assets/themes/material-expressive/081-carousel-motion.css"),
    ),
    (
        "085-compact-volume.css",
        include_str!("../assets/themes/material-expressive/085-compact-volume.css"),
    ),
    (
        "095-controls.css",
        include_str!("../assets/themes/material-expressive/095-controls.css"),
    ),
    (
        "096-tonal-surfaces.css",
        include_str!("../assets/themes/material-expressive/096-tonal-surfaces.css"),
    ),
    (
        "097-queue.css",
        include_str!("../assets/themes/material-expressive/097-queue.css"),
    ),
    (
        "098-cache-indicators.css",
        include_str!("../assets/themes/material-expressive/098-cache-indicators.css"),
    ),
    (
        "099-loading-indicator.css",
        include_str!("../assets/themes/material-expressive/099-loading-indicator.css"),
    ),
    (
        "100-buttons.css",
        include_str!("../assets/themes/material-expressive/100-buttons.css"),
    ),
    (
        "101-keyboard-search.css",
        include_str!("../assets/themes/material-expressive/101-keyboard-search.css"),
    ),
    (
        "102-search-history.css",
        include_str!("../assets/themes/material-expressive/102-search-history.css"),
    ),
    (
        "103-home-player-polish.css",
        include_str!("../assets/themes/material-expressive/103-home-player-polish.css"),
    ),
];

pub(crate) fn frosted_glass_css() -> &'static str {
    FROSTED_GLASS_CSS
}

pub(crate) fn combined_theme_css() -> String {
    let material_len = MATERIAL_EXPRESSIVE_MODULES
        .iter()
        .map(|(_, css)| css.len())
        .sum::<usize>();

    let mut css = String::with_capacity(NOCTALIA_CSS.len() + 1 + material_len);
    css.push_str(NOCTALIA_CSS);
    css.push('\n');

    for (_, module_css) in MATERIAL_EXPRESSIVE_MODULES {
        css.push_str(module_css);
    }

    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_modules_keep_required_tokens_and_surfaces() {
        let css = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .map(|(_, css)| *css)
            .collect::<String>();

        for required in [
            "@define-color m3_primary",
            "@define-color m3_error",
            "@define-color m3_outline_variant",
            ".expressive-footer",
            ".expressive-player-card",
            ".header-view-switcher",
            ".queue2-page",
            ".material-loading-indicator.contained",
            ".material-button-filled",
            ".material-button-filled-tonal",
            ".material-button-elevated",
            ".material-button-outlined",
            ".material-button-text",
            ".material-button-loading",
            ".material-carousel-motion-installed",
            ".material-carousel-edge-spring",
            ".material-carousel-edge-spring-surface",
            ".youtube-home-loading-placeholders",
            ".home-card-loading-placeholder",
            ".collection-grid-action-overlay",
            ".playlist-card-row-with-actions",
            ".collection-action-focusable",
            ".collection-grid-wrapper",
            ".search-result-keyboard-row",
            ".search-result-primary-action",
        ] {
            assert!(css.contains(required), "missing required CSS: {required}");
        }
    }

    #[test]
    fn material_typography_prefers_google_sans_flex_without_leaking_to_other_themes() {
        let foundation_css = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .find_map(|(name, css)| (*name == "000-foundation.css").then_some(*css))
            .expect("000-foundation.css module should be registered");

        assert!(foundation_css.contains("font-family: \"Google Sans Flex\""));
        assert!(foundation_css.contains("window.theme-material-expressive"));
        assert!(!foundation_css.contains("theme-noctalia"));
        assert!(!foundation_css.contains("theme-frosted-glass"));
    }

    #[test]
    fn material_toast_overlay_stays_transparent() {
        let css = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .map(|(_, css)| *css)
            .collect::<String>();

        assert!(css.contains("window.theme-material-expressive > toastoverlay"));
        assert!(!css.contains(
            "window.theme-material-expressive,\nwindow.theme-material-expressive > toastoverlay"
        ));
    }

    #[test]
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

    #[test]
    fn material_button_css_does_not_style_noctalia() {
        let button_css = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .find_map(|(name, css)| (*name == "100-buttons.css").then_some(*css))
            .expect("100-buttons.css module should be registered");

        assert!(!button_css.contains("theme-noctalia"));
        let mut remaining = button_css;
        while let Some((selectors, rest)) = remaining.split_once('{') {
            remaining = match rest.split_once('}') {
                Some((_, tail)) => tail,
                None => "",
            };

            for selector in selectors.split(',') {
                let selector = selector.split_whitespace().collect::<Vec<_>>().join(" ");
                for global_prefix in [
                    "button.material-button",
                    "button.material-icon-button",
                    "button.material-chip",
                ] {
                    assert!(
                        !selector.starts_with(global_prefix),
                        "global Material button selector leaked: {selector}"
                    );
                }
            }
        }
    }

    #[test]
    fn material_controls_css_does_not_style_noctalia() {
        let controls_css = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .find_map(|(name, css)| (*name == "095-controls.css").then_some(*css))
            .expect("095-controls.css module should be registered");

        assert!(!controls_css.contains("theme-noctalia"));
    }

    #[test]
    fn material_loading_css_does_not_style_noctalia() {
        let loading_css = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .find_map(|(name, css)| (*name == "099-loading-indicator.css").then_some(*css))
            .expect("099-loading-indicator.css module should be registered");

        assert!(!loading_css.contains("theme-noctalia"));
    }

    #[test]
    fn material_carousel_motion_is_scoped_and_loaded_after_home_geometry() {
        let names = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        let home_index = names
            .iter()
            .position(|name| *name == "080-home-browser.css")
            .expect("Home module should be registered");
        let motion_index = names
            .iter()
            .position(|name| *name == "081-carousel-motion.css")
            .expect("Carousel motion module should be registered");
        assert!(motion_index > home_index);

        let motion_css = MATERIAL_EXPRESSIVE_MODULES[motion_index].1;
        for required in [
            "window.theme-material-expressive",
            ".material-carousel-edge-spring",
            ".material-carousel-edge-spring-surface",
            ".collection-card-context-action",
            "opacity: 1;",
        ] {
            assert!(
                motion_css.contains(required),
                "missing carousel spring CSS contract: {required}"
            );
        }
        assert!(!motion_css.contains(".material-carousel-item-large"));
        assert!(!motion_css.contains(".material-carousel-item-medium"));
        assert!(!motion_css.contains(".material-carousel-item-small"));
        assert!(!motion_css.contains("theme-noctalia"));
        assert!(!motion_css.contains("theme-frosted-glass"));
    }

    #[test]
    fn material_home_preserves_featured_card_hierarchy() {
        let home_css = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .find_map(|(name, css)| (*name == "080-home-browser.css").then_some(*css))
            .expect("080-home-browser.css module should be registered");

        for required in [
            ".home-section-featured button.home-card-button",
            ".collection-card.home-card-featured",
            ".home-card-featured .collection-artwork",
            "min-width: 220px;",
            "min-width: 196px;",
            "min-width: 176px;",
            "min-height: 288px;",
            ".home-section-compact .home-card-context-overlay",
            "min-width: 168px;",
        ] {
            assert!(
                home_css.contains(required),
                "missing Featured/Compact hierarchy CSS: {required}"
            );
        }
    }

    #[test]
    fn material_modules_are_not_empty() {
        assert!(MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .all(|(_, css)| !css.trim().is_empty()));
    }

    #[test]
    fn frosted_glass_keeps_its_overlay_contract() {
        for required in [
            ".theme-frosted-glass",
            ".expressive-header",
            ".expressive-player-card",
            ".expressive-footer",
            ".settings-group",
        ] {
            assert!(
                FROSTED_GLASS_CSS.contains(required),
                "missing Frosted Glass selector: {required}"
            );
        }
    }

    #[test]
    fn material_module_names_are_unique_and_ordered() {
        let names = MATERIAL_EXPRESSIVE_MODULES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();

        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);

        let original_len = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), original_len);
    }

    #[test]
    fn material_module_names_follow_the_prefix_contract() {
        assert!(MATERIAL_EXPRESSIVE_MODULES.iter().all(|(name, _)| {
            name.len() > 8
                && name.as_bytes()[..3].iter().all(u8::is_ascii_digit)
                && name.as_bytes()[3] == b'-'
                && name.ends_with(".css")
        }));
    }
}
