const CAROUSEL_CSS: &str =
    include_str!("../assets/themes/material-expressive/081-carousel-motion.css");

#[test]
fn carousel_masks_do_not_fade_card_copy() {
    for forbidden_selector in [
        ".collection-card-title",
        ".expressive-card-title",
        ".expressive-card-subtitle",
        ".expressive-card-detail",
    ] {
        assert!(
            !CAROUSEL_CSS.contains(forbidden_selector),
            "carousel motion CSS must clip card copy naturally instead of fading {forbidden_selector}"
        );
    }
}

#[test]
fn narrow_carousel_previews_may_hide_only_floating_actions() {
    for required_selector in [
        ".collection-card-context-action",
        ".collection-card-overflow-button",
        ".material-card-primary-action",
        ".material-card-overflow-trigger",
    ] {
        assert!(
            CAROUSEL_CSS.contains(required_selector),
            "missing narrow-preview action selector: {required_selector}"
        );
    }
}
