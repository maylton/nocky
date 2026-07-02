//! Material Expressive button foundation.
//!
//! The contract is introduced incrementally so each layer is validated before
//! existing widgets begin to consume it.

use gtk::prelude::*;

const LEGACY_CLASSES: &[&str] = &[
    "suggested-action",
    "destructive-action",
    "pill",
    "flat",
    "settings-primary-action",
    "settings-row-action",
];
const VARIANT_CLASSES: &[&str] = &[
    "material-button-filled",
    "material-button-filled-tonal",
    "material-button-elevated",
    "material-button-outlined",
    "material-button-text",
];
const SIZE_CLASSES: &[&str] = &[
    "material-button-compact",
    "material-button-standard",
    "material-button-large",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialButtonVariant {
    Filled,
    FilledTonal,
    Elevated,
    Outlined,
    Text,
}

impl MaterialButtonVariant {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Filled => "material-button-filled",
            Self::FilledTonal => "material-button-filled-tonal",
            Self::Elevated => "material-button-elevated",
            Self::Outlined => "material-button-outlined",
            Self::Text => "material-button-text",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialButtonSize {
    Compact,
    Standard,
    Large,
}

impl MaterialButtonSize {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Compact => "material-button-compact",
            Self::Standard => "material-button-standard",
            Self::Large => "material-button-large",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MaterialButtonSemantic {
    #[default]
    Standard,
    Destructive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialButtonSpec {
    pub variant: MaterialButtonVariant,
    pub size: MaterialButtonSize,
    pub semantic: MaterialButtonSemantic,
}

impl MaterialButtonSpec {
    pub const fn new(variant: MaterialButtonVariant, size: MaterialButtonSize) -> Self {
        Self {
            variant,
            size,
            semantic: MaterialButtonSemantic::Standard,
        }
    }

    pub const fn with_semantic(mut self, semantic: MaterialButtonSemantic) -> Self {
        self.semantic = semantic;
        self
    }

    pub fn css_classes(self) -> Vec<&'static str> {
        let mut classes = vec![self.variant.css_class(), self.size.css_class()];
        if self.semantic == MaterialButtonSemantic::Destructive {
            classes.push("material-button-destructive");
        }
        classes
    }
}

pub fn apply_material_button(button: &gtk::Button, spec: MaterialButtonSpec) {
    button.add_css_class("material-button");

    for class_name in LEGACY_CLASSES
        .iter()
        .chain(VARIANT_CLASSES)
        .chain(SIZE_CLASSES)
    {
        button.remove_css_class(class_name);
    }
    button.remove_css_class("material-button-destructive");

    for class_name in spec.css_classes() {
        button.add_css_class(class_name);
    }
}

pub fn set_material_button_selected(button: &gtk::Button, selected: bool) {
    set_state_class(button, "material-button-selected", selected);
}

pub fn set_material_button_loading(button: &gtk::Button, loading: bool) {
    set_state_class(button, "material-button-loading", loading);
}

fn set_state_class(button: &gtk::Button, class_name: &str, active: bool) {
    if active {
        button.add_css_class(class_name);
    } else {
        button.remove_css_class(class_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_map_to_expected_classes() {
        let cases = [
            (MaterialButtonVariant::Filled, "material-button-filled"),
            (
                MaterialButtonVariant::FilledTonal,
                "material-button-filled-tonal",
            ),
            (MaterialButtonVariant::Elevated, "material-button-elevated"),
            (MaterialButtonVariant::Outlined, "material-button-outlined"),
            (MaterialButtonVariant::Text, "material-button-text"),
        ];

        for (variant, expected) in cases {
            let spec = MaterialButtonSpec::new(variant, MaterialButtonSize::Standard);
            let classes = spec.css_classes();
            assert_eq!(classes[0], expected);
        }
    }

    #[test]
    fn sizes_map_to_expected_classes() {
        let cases = [
            (MaterialButtonSize::Compact, "material-button-compact"),
            (MaterialButtonSize::Standard, "material-button-standard"),
            (MaterialButtonSize::Large, "material-button-large"),
        ];

        for (size, expected) in cases {
            let spec = MaterialButtonSpec::new(MaterialButtonVariant::Filled, size);
            let classes = spec.css_classes();
            assert_eq!(classes[1], expected);
        }
    }

    #[test]
    fn standard_spec_contains_variant_and_size_only() {
        let spec = MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Standard,
        );
        let classes = spec.css_classes();
        let expected = vec!["material-button-filled-tonal", "material-button-standard"];

        assert_eq!(classes, expected);
    }

    #[test]
    fn destructive_is_a_semantic_modifier() {
        let spec =
            MaterialButtonSpec::new(MaterialButtonVariant::Outlined, MaterialButtonSize::Compact)
                .with_semantic(MaterialButtonSemantic::Destructive);
        let classes = spec.css_classes();
        let expected = vec![
            "material-button-outlined",
            "material-button-compact",
            "material-button-destructive",
        ];

        assert_eq!(classes, expected);
    }
}
