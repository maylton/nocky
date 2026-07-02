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
        assert_eq!(MaterialButtonVariant::Filled.css_class(), "material-button-filled");
        assert_eq!(
            MaterialButtonVariant::FilledTonal.css_class(),
            "material-button-filled-tonal"
        );
        assert_eq!(
            MaterialButtonVariant::Elevated.css_class(),
            "material-button-elevated"
        );
        assert_eq!(
            MaterialButtonVariant::Outlined.css_class(),
            "material-button-outlined"
        );
        assert_eq!(MaterialButtonVariant::Text.css_class(), "material-button-text");
    }

    #[test]
    fn sizes_map_to_expected_classes() {
        assert_eq!(
            MaterialButtonSize::Compact.css_class(),
            "material-button-compact"
        );
        assert_eq!(
            MaterialButtonSize::Standard.css_class(),
            "material-button-standard"
        );
        assert_eq!(MaterialButtonSize::Large.css_class(), "material-button-large");
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
