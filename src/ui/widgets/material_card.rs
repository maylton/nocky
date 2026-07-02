//! Material Expressive card and carousel class contract.
//!
//! Cards are content surfaces for a single subject. Carousels are horizontal
//! collections of visual cards. This module only applies semantic classes; the
//! owning layout code keeps geometry and scrolling behavior.

use gtk::prelude::*;

const CARD_VARIANT_CLASSES: &[&str] = &[
    "material-card-elevated",
    "material-card-filled",
    "material-card-outlined",
];

const CAROUSEL_VARIANT_CLASSES: &[&str] =
    &["material-carousel-multi-browse", "material-carousel-hero"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialCardVariant {
    Elevated,
    Filled,
    Outlined,
}

impl MaterialCardVariant {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Elevated => "material-card-elevated",
            Self::Filled => "material-card-filled",
            Self::Outlined => "material-card-outlined",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialCarouselVariant {
    MultiBrowse,
    Hero,
}

impl MaterialCarouselVariant {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::MultiBrowse => "material-carousel-multi-browse",
            Self::Hero => "material-carousel-hero",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialCardSpec {
    pub variant: MaterialCardVariant,
}

impl MaterialCardSpec {
    pub const fn new(variant: MaterialCardVariant) -> Self {
        Self { variant }
    }

    pub fn css_classes(self) -> Vec<&'static str> {
        vec![self.variant.css_class()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialCarouselSpec {
    pub variant: MaterialCarouselVariant,
}

impl MaterialCarouselSpec {
    pub const fn new(variant: MaterialCarouselVariant) -> Self {
        Self { variant }
    }

    pub fn css_classes(self) -> Vec<&'static str> {
        vec![self.variant.css_class()]
    }
}

pub fn apply_material_card(widget: &impl IsA<gtk::Widget>, spec: MaterialCardSpec) {
    let widget = widget.as_ref();
    widget.add_css_class("material-card");

    for class_name in CARD_VARIANT_CLASSES {
        widget.remove_css_class(class_name);
    }

    for class_name in spec.css_classes() {
        widget.add_css_class(class_name);
    }
}

pub fn apply_material_carousel(widget: &impl IsA<gtk::Widget>, spec: MaterialCarouselSpec) {
    let widget = widget.as_ref();
    widget.add_css_class("material-carousel");

    for class_name in CAROUSEL_VARIANT_CLASSES {
        widget.remove_css_class(class_name);
    }

    for class_name in spec.css_classes() {
        widget.add_css_class(class_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_variants_map_to_expected_classes() {
        let cases = [
            (MaterialCardVariant::Elevated, "material-card-elevated"),
            (MaterialCardVariant::Filled, "material-card-filled"),
            (MaterialCardVariant::Outlined, "material-card-outlined"),
        ];

        for (variant, expected) in cases {
            let spec = MaterialCardSpec::new(variant);
            let classes = spec.css_classes();
            assert_eq!(classes, vec![expected]);
        }
    }

    #[test]
    fn carousel_variants_map_to_expected_classes() {
        let cases = [
            (
                MaterialCarouselVariant::MultiBrowse,
                "material-carousel-multi-browse",
            ),
            (MaterialCarouselVariant::Hero, "material-carousel-hero"),
        ];

        for (variant, expected) in cases {
            let spec = MaterialCarouselSpec::new(variant);
            let classes = spec.css_classes();
            assert_eq!(classes, vec![expected]);
        }
    }
}
