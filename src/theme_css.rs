// nocky_css_architecture_phase1_v3
//
// Module order is identical to the former monolithic file.
// Phase 1 changes packaging only; cascade order is preserved.

const NOCTALIA_CSS: &str = include_str!("../assets/themes/noctalia.css");

const MATERIAL_EXPRESSIVE_PARTS: &[&str] = &[
    include_str!("../assets/themes/material-expressive/000-foundation.css"),
    include_str!("../assets/themes/material-expressive/010-footer.css"),
    include_str!("../assets/themes/material-expressive/020-navigation.css"),
    include_str!("../assets/themes/material-expressive/030-dialogs-settings.css"),
    include_str!("../assets/themes/material-expressive/040-dialogs-settings.css"),
    include_str!("../assets/themes/material-expressive/050-dialogs-settings.css"),
    include_str!("../assets/themes/material-expressive/060-dialogs-settings.css"),
    include_str!("../assets/themes/material-expressive/070-player.css"),
    include_str!("../assets/themes/material-expressive/080-home-browser.css"),
    include_str!("../assets/themes/material-expressive/090-footer.css"),
];

pub(crate) fn combined_theme_css() -> String {
    let material_len = MATERIAL_EXPRESSIVE_PARTS
        .iter()
        .map(|part| part.len())
        .sum::<usize>();

    let mut css = String::with_capacity(NOCTALIA_CSS.len() + 1 + material_len);
    css.push_str(NOCTALIA_CSS);
    css.push('\n');

    for part in MATERIAL_EXPRESSIVE_PARTS {
        css.push_str(part);
    }

    css
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_MATERIAL_EXPRESSIVE_BYTES: usize = 116833;

    #[test]
    fn material_modules_keep_original_size() {
        let actual = MATERIAL_EXPRESSIVE_PARTS
            .iter()
            .map(|part| part.len())
            .sum::<usize>();

        assert_eq!(actual, EXPECTED_MATERIAL_EXPRESSIVE_BYTES);
    }

    #[test]
    fn material_modules_are_not_empty() {
        assert!(MATERIAL_EXPRESSIVE_PARTS
            .iter()
            .all(|part| !part.trim().is_empty()));
    }
}
