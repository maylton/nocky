//! Shared semantic contract for Material Expressive labeled buttons.
//!
//! This first layer is intentionally independent from GTK widgets. It defines
//! the stable mapping between semantic variants, sizes and CSS classes. Widget
//! mutation, loading content and expressive motion are introduced in later
//! checkpoints after this contract is validated.

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

    pub const fn destructive(mut self) -> Self {
        self.semantic = MaterialButtonSemantic::Destructive;
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
        .destructive()
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
