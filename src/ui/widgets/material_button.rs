//! Material Expressive button foundation.
//!
//! The contract is introduced incrementally so each layer is validated before
//! existing widgets begin to consume it.

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
            (
                MaterialButtonVariant::Elevated,
                "material-button-elevated",
            ),
            (
                MaterialButtonVariant::Outlined,
                "material-button-outlined",
            ),
            (MaterialButtonVariant::Text, "material-button-text"),
        ];

        for (variant, expected) in cases {
            let classes = MaterialButtonSpec::new(variant, MaterialButtonSize::Standard)
                .css_classes();
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
            let classes = MaterialButtonSpec::new(MaterialButtonVariant::Filled, size).css_classes();
            assert_eq!(classes[1], expected);
        }
    }

    #[test]
    fn standard_spec_contains_variant_and_size_only() {
        let classes = MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Standard,
        )
        .css_classes();

        assert_eq!(
            classes,
            vec!["material-button-filled-tonal", "material-button-standard"]
        );
    }

    #[test]
    fn destructive_is_a_semantic_modifier() {
        let classes = MaterialButtonSpec::new(
            MaterialButtonVariant::Outlined,
            MaterialButtonSize::Compact,
        )
        .with_semantic(MaterialButtonSemantic::Destructive)
        .css_classes();

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
