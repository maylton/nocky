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
        ] {
            assert!(css.contains(required), "missing required CSS: {required}");
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
