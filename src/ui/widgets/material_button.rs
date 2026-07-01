//! Shared semantic contract for Material Expressive labeled buttons.
//!
//! The helper owns CSS class assignment only. Theme modules own colors,
//! elevation, state layers and shape. Loading content and expressive motion are
//! added in later checkpoints so this foundation remains allocation-stable.

use gtk::prelude::*;

const LEGACY_CLASSES: &[&str] = &["suggested-action", "destructive-action", "pill"];
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
    const fn css_class(self) -> &'static str {
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
    const fn css_class(self) -> &'static str {
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

    pub const fn destructive(mut self) -> Self {
        self.semantic = MaterialButtonSemantic::Destructive;
        self
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

    for class_name in spec_classes(spec) {
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

fn spec_classes(spec: MaterialButtonSpec) -> Vec<&'static str> {
    let mut classes = vec![spec.variant.css_class(), spec.size.css_class()];
    if spec.semantic == MaterialButtonSemantic::Destructive {
        classes.push("material-button-destructive");
    }
    classes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_a_distinct_semantic_class() {
        let classes = [
            MaterialButtonVariant::Filled,
            MaterialButtonVariant::FilledTonal,
            MaterialButtonVariant::Elevated,
            MaterialButtonVariant::Outlined,
            MaterialButtonVariant::Text,
        ]
        .map(MaterialButtonVariant::css_class);

        let mut unique = classes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), classes.len());
    }

    #[test]
    fn every_size_has_a_distinct_class() {
        let classes = [
            MaterialButtonSize::Compact,
            MaterialButtonSize::Standard,
            MaterialButtonSize::Large,
        ]
        .map(MaterialButtonSize::css_class);

        let mut unique = classes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), classes.len());
    }

    #[test]
    fn standard_spec_contains_variant_and_size_only() {
        let classes = spec_classes(MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Standard,
        ));

        assert_eq!(
            classes,
            vec!["material-button-filled-tonal", "material-button-standard"]
        );
    }

    #[test]
    fn destructive_is_a_semantic_modifier() {
        let classes = spec_classes(
            MaterialButtonSpec::new(
                MaterialButtonVariant::Outlined,
                MaterialButtonSize::Compact,
            )
            .destructive(),
        );

        assert_eq!(
            classes,
            vec![
                "material-button-outlined",
                "material-button-compact",
                "material-button-destructive",
            ]
        );
    }
}
